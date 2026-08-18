//! FTR-006: catalog-backed built-in model lifecycle.
//!
//! The catalog is a checked-in trust root. Callers select only a catalog ID; download URLs,
//! expected sizes, digests, licenses, filenames, runtime compatibility, and redirect hosts never
//! cross IPC from the webview. Downloads land in a catalog-owned `.partial` file, resume only
//! from a valid byte range, and become visible at the final GGUF path only after exact-size,
//! SHA-256, and GGUF validation succeeds.

use crate::errors::AppError;
use crate::AppState;
use futures_util::StreamExt;
use reqwest::header::{CONTENT_LENGTH, CONTENT_RANGE, LOCATION, RANGE};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use sysinfo::{Disks, System};
use tauri::{AppHandle, Emitter, Manager};
use tokio::io::AsyncWriteExt;
use tokio::sync::Notify;

const DOWNLOAD_EVENT: &str = "managed-model:download-progress";
const MAX_REDIRECTS: usize = 5;
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(60 * 60);
const DISK_RESERVE_BYTES: u64 = 512 * 1024 * 1024;

#[derive(Debug, Default)]
pub(crate) struct ManagedDownloadCancellation {
    cancelled: AtomicBool,
    notify: Notify,
}

impl ManagedDownloadCancellation {
    fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
        self.notify.notify_one();
    }

    async fn wait(&self) {
        if self.cancelled.load(Ordering::SeqCst) {
            return;
        }
        self.notify.notified().await;
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ModelCatalog {
    schema_version: u32,
    reviewed_at: String,
    models: Vec<ManagedModelCatalogEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ManagedModelCompatibility {
    pub runtime: String,
    pub runtime_version: String,
    pub format: String,
    pub platforms: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ManagedModelCatalogEntry {
    pub id: String,
    pub display_name: String,
    pub publisher: String,
    pub description: String,
    pub source_repository: String,
    pub source_commit: String,
    pub download_url: String,
    /// Security policy for redirects from the immutable publisher URL. A host must be exactly
    /// one suffix or a dot-delimited subdomain of it; arbitrary `hf.co` hosts are not accepted.
    pub allowed_download_host_suffixes: Vec<String>,
    pub file_name: String,
    pub size_bytes: u64,
    pub sha256: String,
    pub license: String,
    pub license_url: String,
    pub quantization: String,
    pub context_window: u64,
    pub architecture: String,
    pub parameter_count: String,
    pub minimum_available_memory_bytes: u64,
    pub recommended_available_memory_bytes: u64,
    pub compatibility: ManagedModelCompatibility,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ManagedModelStatus {
    pub model: ManagedModelCatalogEntry,
    pub storage_directory: String,
    pub model_path: String,
    pub installed: bool,
    pub verified: bool,
    pub partial_bytes: u64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ManagedModelOperation {
    Download,
    Load,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HardwareFitRisk {
    Safe,
    Warning,
    Blocked,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ManagedModelPreflight {
    pub model_id: String,
    pub operation: ManagedModelOperation,
    pub risk: HardwareFitRisk,
    pub available_memory_bytes: u64,
    pub minimum_available_memory_bytes: u64,
    pub recommended_available_memory_bytes: u64,
    pub available_disk_bytes: u64,
    pub required_disk_bytes: u64,
    pub advisories: Vec<String>,
    pub advanced_override_allowed: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct HardwareFitEvidence {
    pub total_memory_bytes: u64,
    pub available_memory_bytes: u64,
    pub execution_device: String,
    pub accelerator_memory_bytes: Option<u64>,
    pub method_version: String,
}

pub fn local_hardware_fit_evidence() -> HardwareFitEvidence {
    let mut system = System::new_all();
    system.refresh_memory();
    HardwareFitEvidence {
        total_memory_bytes: system.total_memory(),
        available_memory_bytes: system.available_memory(),
        execution_device: "local_device".to_string(),
        // Ark deliberately reports unknown rather than inferring shared/dedicated GPU memory
        // from platform-specific APIs that have not yet been qualified on the support matrix.
        accelerator_memory_bytes: None,
        method_version: "ark-fit-v1".to_string(),
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManagedModelDownloadRequest {
    pub model_id: String,
    #[serde(default)]
    pub acknowledge_warning: bool,
    #[serde(default)]
    pub advanced_override: bool,
    pub override_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartManagedModelRequest {
    pub model_id: String,
    #[serde(default)]
    pub acknowledge_warning: bool,
    #[serde(default)]
    pub advanced_override: bool,
    pub override_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManagedModelDownloadProgress {
    pub schema_version: u32,
    pub model_id: String,
    pub status: String,
    pub completed_bytes: u64,
    pub total_bytes: u64,
    pub resumed: bool,
}

fn catalog() -> Result<ModelCatalog, AppError> {
    let catalog: ModelCatalog =
        serde_json::from_str(include_str!("../../config/model-catalog.json"))
            .map_err(|error| AppError::new("model_catalog_invalid", error.to_string()))?;
    validate_catalog(&catalog)?;
    Ok(catalog)
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn safe_file_name(value: &str) -> bool {
    !value.is_empty()
        && value.ends_with(".gguf")
        && Path::new(value).file_name().and_then(|name| name.to_str()) == Some(value)
        && !value.chars().any(char::is_control)
}

fn validate_catalog(catalog: &ModelCatalog) -> Result<(), AppError> {
    if catalog.schema_version != 1
        || catalog.reviewed_at.trim().is_empty()
        || catalog.models.is_empty()
    {
        return Err(AppError::new(
            "model_catalog_invalid",
            "The managed model catalog schema or review metadata is invalid.",
        ));
    }

    let reviewed_runtime: serde_json::Value =
        serde_json::from_str(include_str!("../../config/native-artifacts.json"))
            .map_err(|error| AppError::new("model_catalog_invalid", error.to_string()))?;
    let runtime_version = reviewed_runtime["runtime"]["version"]
        .as_str()
        .ok_or_else(|| AppError::new("model_catalog_invalid", "Runtime version is missing."))?;
    let reviewed_platforms: HashSet<String> = reviewed_runtime["artifacts"]
        .as_array()
        .ok_or_else(|| AppError::new("model_catalog_invalid", "Runtime artifacts are missing."))?
        .iter()
        .filter_map(|artifact| {
            Some(format!(
                "{}-{}",
                artifact["platform"].as_str()?,
                artifact["arch"].as_str()?
            ))
        })
        .collect();
    let release_capabilities: serde_json::Value =
        serde_json::from_str(include_str!("../../config/release-capabilities.json"))
            .map_err(|error| AppError::new("model_catalog_invalid", error.to_string()))?;
    let qualified_platforms: HashSet<String> = release_capabilities["artifactPlatforms"]
        .as_array()
        .ok_or_else(|| {
            AppError::new(
                "model_catalog_invalid",
                "Qualified artifact platforms are missing.",
            )
        })?
        .iter()
        .filter_map(|platform| platform["runtimeTarget"].as_str().map(str::to_string))
        .collect();
    if qualified_platforms.is_empty()
        || qualified_platforms
            .iter()
            .any(|platform| !reviewed_platforms.contains(platform))
    {
        return Err(AppError::new(
            "model_catalog_invalid",
            "Qualified packaged targets drift from reviewed runtime artifacts.",
        ));
    }
    let mut ids = HashSet::new();
    for model in &catalog.models {
        let source = reqwest::Url::parse(&model.source_repository)
            .map_err(|_| AppError::new("model_catalog_invalid", "A source URL is invalid."))?;
        let license = reqwest::Url::parse(&model.license_url)
            .map_err(|_| AppError::new("model_catalog_invalid", "A license URL is invalid."))?;
        let download = reqwest::Url::parse(&model.download_url)
            .map_err(|_| AppError::new("model_catalog_invalid", "A download URL is invalid."))?;
        let model_platforms: HashSet<String> =
            model.compatibility.platforms.iter().cloned().collect();
        if !ids.insert(model.id.as_str())
            || crate::validation::validate_entity_id(&model.id, "Model ID").is_err()
            || !safe_file_name(&model.file_name)
            || model.size_bytes < 4
            || !valid_sha256(&model.sha256)
            || model.source_commit.len() != 40
            || !model
                .source_commit
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            || !model.source_repository.contains(&model.source_commit)
            || !model.download_url.contains(&model.source_commit)
            || !model.license_url.contains(&model.source_commit)
            || model.context_window == 0
            || model.minimum_available_memory_bytes == 0
            || model.recommended_available_memory_bytes < model.minimum_available_memory_bytes
            || model.compatibility.runtime != "llama.cpp"
            || model.compatibility.runtime_version != runtime_version
            || model_platforms != qualified_platforms
            || model.allowed_download_host_suffixes.is_empty()
            || [source, license, download]
                .iter()
                .any(|url| url.scheme() != "https" || url.host_str().is_none())
        {
            return Err(AppError::new(
                "model_catalog_invalid",
                format!("Managed model catalog entry '{}' is invalid.", model.id),
            ));
        }
        validate_download_url(
            &reqwest::Url::parse(&model.download_url).expect("validated above"),
            &model.allowed_download_host_suffixes,
        )?;
    }
    Ok(())
}

fn find_model(model_id: &str) -> Result<ManagedModelCatalogEntry, AppError> {
    let model_id = crate::validation::validate_entity_id(model_id, "Model ID")?;
    catalog()?
        .models
        .into_iter()
        .find(|model| model.id == model_id)
        .ok_or_else(|| AppError::not_found("Managed model"))
}

pub fn storage_directory(app: &AppHandle) -> Result<PathBuf, AppError> {
    let settings = crate::device_settings::load_device_settings(app, None);
    let raw = if let Some(custom) = settings.managed_model_directory {
        PathBuf::from(custom)
    } else {
        app.path()
            .app_local_data_dir()
            .map_err(|error| {
                AppError::new(
                    "model_storage_unavailable",
                    format!("Could not resolve managed model storage: {error}"),
                )
            })?
            .join("models")
    };
    let validated = crate::validation::validate_directory_target_path(
        &raw.display().to_string(),
        "Managed model directory",
    )?;
    std::fs::create_dir_all(&validated).map_err(AppError::from)?;
    Ok(validated)
}

fn model_paths(storage: &Path, model: &ManagedModelCatalogEntry) -> (PathBuf, PathBuf) {
    let destination = storage.join(&model.file_name);
    let partial = storage.join(format!("{}.partial", model.file_name));
    (destination, partial)
}

fn model_is_in_use(running_path: Option<&str>, destination: &Path) -> bool {
    running_path.is_some_and(|path| {
        crate::validation::paths_refer_to_same_location(Path::new(path), destination)
    })
}

fn regular_file_size(path: &Path) -> Result<Option<u64>, AppError> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
            Err(AppError::new(
                "managed_model_path_invalid",
                "A managed model path was replaced by a non-regular file or symbolic link.",
            ))
        }
        Ok(metadata) => Ok(Some(metadata.len())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(AppError::from(error)),
    }
}

pub fn list_managed_models(app: &AppHandle) -> Result<Vec<ManagedModelStatus>, AppError> {
    let storage = storage_directory(app)?;
    let provenance = crate::supply_chain::load_model_provenance(app)?;
    catalog()?
        .models
        .into_iter()
        .map(|model| {
            let (destination, partial) = model_paths(&storage, &model);
            let installed = regular_file_size(&destination)? == Some(model.size_bytes);
            let verified = installed
                && provenance.as_ref().is_some_and(|record| {
                    crate::validation::paths_refer_to_same_location(
                        Path::new(&record.path),
                        &destination,
                    ) && record.sha256 == model.sha256
                        && record.size_bytes == model.size_bytes
                });
            let partial_bytes = regular_file_size(&partial)?.unwrap_or(0);
            Ok(ManagedModelStatus {
                model,
                storage_directory: storage.display().to_string(),
                model_path: destination.display().to_string(),
                installed,
                verified,
                partial_bytes,
            })
        })
        .collect()
}

fn disk_available_for(path: &Path) -> u64 {
    let disks = Disks::new_with_refreshed_list();
    disks
        .iter()
        .filter(|disk| path.starts_with(disk.mount_point()))
        .max_by_key(|disk| disk.mount_point().as_os_str().len())
        .map(|disk| disk.available_space())
        .unwrap_or(0)
}

fn classify_preflight(
    model: &ManagedModelCatalogEntry,
    operation: ManagedModelOperation,
    available_memory_bytes: u64,
    available_disk_bytes: u64,
) -> ManagedModelPreflight {
    let required_disk_bytes = model.size_bytes.saturating_add(DISK_RESERVE_BYTES);
    let mut risk = HardwareFitRisk::Safe;
    let mut advisories = Vec::new();
    match operation {
        ManagedModelOperation::Download => {
            if available_disk_bytes == 0 {
                risk = HardwareFitRisk::Warning;
                advisories.push(
                    "Ark could not determine free space on the selected model-storage volume."
                        .to_string(),
                );
            } else if available_disk_bytes < model.size_bytes {
                risk = HardwareFitRisk::Blocked;
                advisories.push("The selected model-storage volume does not have enough free space for the model file.".to_string());
            } else if available_disk_bytes < required_disk_bytes {
                risk = HardwareFitRisk::Warning;
                advisories.push("The model fits, but the storage volume would have less than Ark's 512 MiB safety reserve.".to_string());
            }
        }
        ManagedModelOperation::Load => {
            if available_memory_bytes == 0 {
                risk = HardwareFitRisk::Warning;
                advisories
                    .push("Ark could not determine currently available system memory.".to_string());
            } else if available_memory_bytes < model.minimum_available_memory_bytes {
                risk = HardwareFitRisk::Blocked;
                advisories.push(
                    "Available memory is below the catalog's conservative minimum for this model."
                        .to_string(),
                );
            } else if available_memory_bytes < model.recommended_available_memory_bytes {
                risk = HardwareFitRisk::Warning;
                advisories.push("Available memory is below the catalog's recommended headroom; other applications may be affected.".to_string());
            }
        }
    }
    ManagedModelPreflight {
        model_id: model.id.clone(),
        operation,
        risk,
        available_memory_bytes,
        minimum_available_memory_bytes: model.minimum_available_memory_bytes,
        recommended_available_memory_bytes: model.recommended_available_memory_bytes,
        available_disk_bytes,
        required_disk_bytes,
        advisories,
        advanced_override_allowed: risk == HardwareFitRisk::Blocked,
    }
}

pub fn preflight_managed_model(
    app: &AppHandle,
    model_id: &str,
    operation: ManagedModelOperation,
) -> Result<ManagedModelPreflight, AppError> {
    let model = find_model(model_id)?;
    let storage = storage_directory(app)?;
    let mut system = System::new_all();
    system.refresh_memory();
    Ok(classify_preflight(
        &model,
        operation,
        system.available_memory(),
        disk_available_for(&storage),
    ))
}

fn validate_override(
    preflight: &ManagedModelPreflight,
    acknowledge_warning: bool,
    advanced_override: bool,
    override_reason: Option<&str>,
) -> Result<(), AppError> {
    match preflight.risk {
        HardwareFitRisk::Safe => Ok(()),
        HardwareFitRisk::Warning if acknowledge_warning => Ok(()),
        HardwareFitRisk::Warning => Err(AppError::new(
            "hardware_fit_acknowledgement_required",
            "Review and acknowledge the hardware-fit warning before continuing.",
        )),
        HardwareFitRisk::Blocked => {
            let reason = override_reason.unwrap_or("").trim();
            if !advanced_override
                || reason.len() < 12
                || reason.len() > 512
                || reason.chars().any(char::is_control)
            {
                return Err(AppError::new(
                    "hardware_fit_blocked",
                    "This operation is clearly unsafe on the detected hardware. An advanced override requires a specific reason of 12–512 characters.",
                ));
            }
            Ok(())
        }
    }
}

fn host_matches_suffix(host: &str, suffix: &str) -> bool {
    host == suffix
        || host
            .strip_suffix(suffix)
            .is_some_and(|prefix| prefix.ends_with('.') && prefix.len() > 1)
}

fn validate_download_url(url: &reqwest::Url, allowed: &[String]) -> Result<(), AppError> {
    let host = url.host_str().unwrap_or_default();
    if url.scheme() != "https"
        || !url.username().is_empty()
        || url.password().is_some()
        || !allowed
            .iter()
            .any(|suffix| host_matches_suffix(host, suffix))
    {
        return Err(AppError::new(
            "model_download_destination_blocked",
            "The model download or redirect destination is outside the reviewed HTTPS host policy.",
        ));
    }
    Ok(())
}

async fn send_download_request(
    client: &reqwest::Client,
    model: &ManagedModelCatalogEntry,
    offset: u64,
) -> Result<reqwest::Response, AppError> {
    let mut url = reqwest::Url::parse(&model.download_url)
        .map_err(|_| AppError::new("model_catalog_invalid", "Model download URL is invalid."))?;
    for redirects in 0..=MAX_REDIRECTS {
        validate_download_url(&url, &model.allowed_download_host_suffixes)?;
        let mut request = client.get(url.clone());
        if offset > 0 {
            request = request.header(RANGE, format!("bytes={offset}-"));
        }
        let response = request.send().await.map_err(|error| {
            AppError::new(
                "model_download_failed",
                format!("Model download failed: {error}"),
            )
        })?;
        if !response.status().is_redirection() {
            return Ok(response);
        }
        if redirects == MAX_REDIRECTS {
            return Err(AppError::new(
                "model_download_redirect_limit",
                "The model download exceeded Ark's redirect limit.",
            ));
        }
        let location = response
            .headers()
            .get(LOCATION)
            .and_then(|value| value.to_str().ok())
            .ok_or_else(|| {
                AppError::new(
                    "model_download_failed",
                    "The model publisher returned a redirect without a valid location.",
                )
            })?;
        url = url.join(location).map_err(|_| {
            AppError::new(
                "model_download_failed",
                "The model publisher returned an invalid redirect location.",
            )
        })?;
    }
    unreachable!("redirect loop returns or errors")
}

fn content_range_starts_at(response: &reqwest::Response, offset: u64) -> bool {
    response
        .headers()
        .get(CONTENT_RANGE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.starts_with(&format!("bytes {offset}-")))
}

fn hash_file_exact(path: &Path, expected_size: u64) -> Result<String, AppError> {
    let metadata = std::fs::symlink_metadata(path).map_err(AppError::from)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() != expected_size {
        return Err(AppError::new(
            "managed_model_integrity_failed",
            "The downloaded model size does not match the reviewed catalog.",
        ));
    }
    let mut file = std::fs::File::open(path).map_err(AppError::from)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; 1024 * 1024];
    loop {
        let count = file.read(&mut buffer).map_err(AppError::from)?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn finalize_partial(
    partial: &Path,
    destination: &Path,
    model: &ManagedModelCatalogEntry,
) -> Result<(), AppError> {
    let digest = hash_file_exact(partial, model.size_bytes)?;
    if digest != model.sha256 {
        let _ = std::fs::remove_file(partial);
        return Err(AppError::new(
            "managed_model_integrity_failed",
            "The downloaded model SHA-256 does not match the reviewed catalog. The untrusted partial file was removed.",
        ));
    }
    if let Err(error) = crate::validation::validate_gguf_file(partial) {
        let _ = std::fs::remove_file(partial);
        return Err(error);
    }
    if destination.exists() {
        return Err(AppError::new(
            "managed_model_already_exists",
            "A model file already exists at the managed destination. Ark will not overwrite it.",
        ));
    }
    std::fs::rename(partial, destination).map_err(AppError::from)
}

fn emit_progress(
    app: &AppHandle,
    model: &ManagedModelCatalogEntry,
    status: &str,
    completed_bytes: u64,
    resumed: bool,
) {
    let _ = app.emit(
        DOWNLOAD_EVENT,
        ManagedModelDownloadProgress {
            schema_version: 1,
            model_id: model.id.clone(),
            status: status.to_string(),
            completed_bytes,
            total_bytes: model.size_bytes,
            resumed,
        },
    );
}

async fn download_inner(
    app: &AppHandle,
    model: &ManagedModelCatalogEntry,
    storage: &Path,
    cancellation: &ManagedDownloadCancellation,
) -> Result<ManagedModelStatus, AppError> {
    let (destination, partial) = model_paths(storage, model);
    if let Some(size) = regular_file_size(&destination)? {
        if size != model.size_bytes {
            return Err(AppError::new(
                "managed_model_integrity_failed",
                "The existing managed model has an unexpected size. Delete it before retrying.",
            ));
        }
        let destination_for_hash = destination.clone();
        let expected_size = model.size_bytes;
        let digest = tokio::task::spawn_blocking(move || {
            hash_file_exact(&destination_for_hash, expected_size)
        })
        .await
        .map_err(|_| {
            AppError::new(
                "managed_model_integrity_failed",
                "Model verification worker did not complete.",
            )
        })??;
        if digest != model.sha256 {
            return Err(AppError::new(
                "managed_model_integrity_failed",
                "The existing managed model does not match the reviewed catalog. Delete it before retrying.",
            ));
        }
        let record = crate::supply_chain::verify_and_record_model(
            app,
            &destination,
            &model.source_repository,
            &model.license,
        )?;
        return Ok(ManagedModelStatus {
            model: model.clone(),
            storage_directory: storage.display().to_string(),
            model_path: destination.display().to_string(),
            installed: true,
            verified: record.sha256 == model.sha256,
            partial_bytes: regular_file_size(&partial)?.unwrap_or(0),
        });
    }

    let mut offset = regular_file_size(&partial)?.unwrap_or(0);
    if offset > model.size_bytes {
        std::fs::remove_file(&partial).map_err(AppError::from)?;
        offset = 0;
    }
    let resumed = offset > 0;
    emit_progress(app, model, "starting", offset, resumed);

    if offset < model.size_bytes {
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(30))
            .timeout(DOWNLOAD_TIMEOUT)
            .user_agent("Ark/0.1 managed-model-downloader")
            .build()
            .map_err(|error| AppError::new("model_download_failed", error.to_string()))?;
        let mut response = send_download_request(&client, model, offset).await?;
        let appending = offset > 0
            && response.status() == reqwest::StatusCode::PARTIAL_CONTENT
            && content_range_starts_at(&response, offset);
        if offset > 0 && !appending {
            // A server that ignores Range must be restarted from zero; appending a 200 response
            // would create a corrupt file that only fails after hundreds of megabytes.
            offset = 0;
            response = send_download_request(&client, model, 0).await?;
        }
        if !response.status().is_success() {
            return Err(AppError::new(
                "model_download_failed",
                format!("The model publisher returned HTTP {}.", response.status()),
            ));
        }
        if let Some(length) = response
            .headers()
            .get(CONTENT_LENGTH)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse::<u64>().ok())
        {
            let expected_remaining = model.size_bytes.saturating_sub(offset);
            if length != expected_remaining {
                let _ = tokio::fs::remove_file(&partial).await;
                return Err(AppError::new(
                    "managed_model_integrity_failed",
                    "The publisher response size does not match the reviewed catalog.",
                ));
            }
        }

        let mut options = tokio::fs::OpenOptions::new();
        options.write(true).create(true);
        if offset == 0 {
            options.truncate(true);
        } else {
            options.append(true);
        }
        let mut file = options.open(&partial).await.map_err(AppError::from)?;
        let mut completed = offset;
        let mut last_event = Instant::now();
        let mut stream = response.bytes_stream();
        loop {
            let next_chunk = tokio::select! {
                _ = cancellation.wait() => {
                    None
                }
                next = stream.next() => {
                    match next {
                        Some(chunk) => Some(chunk),
                        None => break,
                    }
                }
            };
            let Some(chunk) = next_chunk else {
                file.flush().await.map_err(AppError::from)?;
                file.sync_all().await.map_err(AppError::from)?;
                emit_progress(app, model, "cancelled", completed, resumed);
                return Err(AppError::new(
                    "model_download_cancelled",
                    "Model download was cancelled. The downloaded prefix was retained for resume and will be verified before installation.",
                ));
            };
            let chunk = chunk.map_err(|error| {
                AppError::new(
                    "model_download_failed",
                    format!("Model download failed: {error}"),
                )
            })?;
            completed = completed.saturating_add(chunk.len() as u64);
            if completed > model.size_bytes {
                drop(file);
                let _ = tokio::fs::remove_file(&partial).await;
                return Err(AppError::new(
                    "managed_model_integrity_failed",
                    "The publisher sent more bytes than the reviewed model size.",
                ));
            }
            file.write_all(&chunk).await.map_err(AppError::from)?;
            if last_event.elapsed() >= Duration::from_millis(200) {
                emit_progress(app, model, "downloading", completed, resumed);
                last_event = Instant::now();
            }
        }
        file.flush().await.map_err(AppError::from)?;
        file.sync_all().await.map_err(AppError::from)?;
        drop(file);
        if completed != model.size_bytes {
            return Err(AppError::new(
                "model_download_incomplete",
                "The model download ended early. Retry to resume from the retained partial file.",
            ));
        }
    }

    emit_progress(app, model, "verifying", model.size_bytes, resumed);
    let partial_for_finalize = partial.clone();
    let destination_for_finalize = destination.clone();
    let model_for_finalize = model.clone();
    tokio::task::spawn_blocking(move || {
        finalize_partial(
            &partial_for_finalize,
            &destination_for_finalize,
            &model_for_finalize,
        )
    })
    .await
    .map_err(|_| {
        AppError::new(
            "managed_model_integrity_failed",
            "Model verification worker did not complete.",
        )
    })??;
    let record = crate::supply_chain::verify_and_record_model(
        app,
        &destination,
        &model.source_repository,
        &model.license,
    )?;
    if record.sha256 != model.sha256 || record.size_bytes != model.size_bytes {
        return Err(AppError::new(
            "managed_model_integrity_failed",
            "The installed model changed during final provenance verification.",
        ));
    }
    emit_progress(app, model, "complete", model.size_bytes, resumed);
    Ok(ManagedModelStatus {
        model: model.clone(),
        storage_directory: storage.display().to_string(),
        model_path: destination.display().to_string(),
        installed: true,
        verified: true,
        partial_bytes: 0,
    })
}

pub async fn download_managed_model(
    app: &AppHandle,
    state: &AppState,
    request: ManagedModelDownloadRequest,
) -> Result<ManagedModelStatus, AppError> {
    let model = find_model(&request.model_id)?;
    let preflight = preflight_managed_model(app, &model.id, ManagedModelOperation::Download)?;
    validate_override(
        &preflight,
        request.acknowledge_warning,
        request.advanced_override,
        request.override_reason.as_deref(),
    )?;
    let cancellation = Arc::new(ManagedDownloadCancellation::default());
    {
        let mut active = state.active_managed_model_downloads.lock().map_err(|_| {
            AppError::new(
                "state_error",
                "Could not access managed model download state.",
            )
        })?;
        if active.contains_key(&model.id) {
            return Err(AppError::new(
                "model_download_in_progress",
                "This managed model is already downloading.",
            ));
        }
        active.insert(model.id.clone(), Arc::clone(&cancellation));
    }
    let storage = storage_directory(app)?;
    let result = download_inner(app, &model, &storage, &cancellation).await;
    if let Ok(mut active) = state.active_managed_model_downloads.lock() {
        if active
            .get(&model.id)
            .is_some_and(|stored| Arc::ptr_eq(stored, &cancellation))
        {
            active.remove(&model.id);
        }
    }
    result
}

pub fn cancel_managed_model_download(state: &AppState, model_id: &str) -> Result<(), AppError> {
    let model_id = crate::validation::validate_entity_id(model_id, "Model ID")?;
    let active = state.active_managed_model_downloads.lock().map_err(|_| {
        AppError::new(
            "state_error",
            "Could not access managed model download state.",
        )
    })?;
    let cancellation = active.get(model_id).ok_or_else(|| {
        AppError::new(
            "model_download_not_active",
            "No download is active for this model.",
        )
    })?;
    cancellation.cancel();
    Ok(())
}

pub fn delete_managed_model(
    app: &AppHandle,
    state: &AppState,
    model_id: &str,
) -> Result<(), AppError> {
    let model = find_model(model_id)?;
    if state
        .active_managed_model_downloads
        .lock()
        .map_err(|_| {
            AppError::new(
                "state_error",
                "Could not access managed model download state.",
            )
        })?
        .contains_key(&model.id)
    {
        return Err(AppError::new(
            "model_download_in_progress",
            "Cancel the active download before deleting this model.",
        ));
    }
    let storage = storage_directory(app)?;
    let (destination, partial) = model_paths(&storage, &model);
    let running_path = crate::commands::lock_sidecar(state)?.model_path();
    if model_is_in_use(running_path.as_deref(), &destination) {
        return Err(AppError::new(
            "model_in_use",
            "Stop the built-in runtime before deleting the model it is using.",
        ));
    }
    for path in [&destination, &partial] {
        if regular_file_size(path)?.is_some() {
            std::fs::remove_file(path).map_err(AppError::from)?;
        }
    }
    crate::supply_chain::clear_model_provenance_for_path(app, &destination)?;
    Ok(())
}

pub fn installed_model(
    app: &AppHandle,
    model_id: &str,
) -> Result<(ManagedModelCatalogEntry, PathBuf), AppError> {
    let model = find_model(model_id)?;
    let storage = storage_directory(app)?;
    let (destination, _) = model_paths(&storage, &model);
    if regular_file_size(&destination)? != Some(model.size_bytes) {
        return Err(AppError::new(
            "managed_model_not_installed",
            "Download and verify this managed model before loading it.",
        ));
    }
    let digest = hash_file_exact(&destination, model.size_bytes)?;
    if digest != model.sha256 {
        return Err(AppError::new(
            "managed_model_integrity_failed",
            "The installed model no longer matches the reviewed catalog.",
        ));
    }
    Ok((model, destination))
}

pub fn authorize_start(
    app: &AppHandle,
    request: &StartManagedModelRequest,
) -> Result<(ManagedModelCatalogEntry, PathBuf), AppError> {
    let preflight = preflight_managed_model(app, &request.model_id, ManagedModelOperation::Load)?;
    validate_override(
        &preflight,
        request.acknowledge_warning,
        request.advanced_override,
        request.override_reason.as_deref(),
    )?;
    installed_model(app, &request.model_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_model(bytes: &[u8]) -> ManagedModelCatalogEntry {
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        ManagedModelCatalogEntry {
            id: "fixture".to_string(),
            display_name: "Fixture".to_string(),
            publisher: "Ark tests".to_string(),
            description: "fixture".to_string(),
            source_repository: "https://example.com/repo".to_string(),
            source_commit: "abc".to_string(),
            download_url: "https://example.com/model.gguf".to_string(),
            allowed_download_host_suffixes: vec!["example.com".to_string()],
            file_name: "fixture.gguf".to_string(),
            size_bytes: bytes.len() as u64,
            sha256: format!("{:x}", hasher.finalize()),
            license: "MIT".to_string(),
            license_url: "https://example.com/license".to_string(),
            quantization: "Q4_0".to_string(),
            context_window: 1024,
            architecture: "fixture".to_string(),
            parameter_count: "1".to_string(),
            minimum_available_memory_bytes: 100,
            recommended_available_memory_bytes: 200,
            compatibility: ManagedModelCompatibility {
                runtime: "llama.cpp".to_string(),
                runtime_version: "b9859".to_string(),
                format: "GGUF".to_string(),
                platforms: vec!["win32-x64".to_string()],
            },
        }
    }

    #[test]
    fn checked_in_catalog_is_valid_and_matches_reviewed_runtime_targets() {
        let parsed = catalog().expect("catalog validates");
        assert_eq!(parsed.schema_version, 1);
        assert!(!parsed.models.is_empty());
        assert_eq!(parsed.models[0].sha256.len(), 64);
    }

    #[test]
    fn download_destination_policy_accepts_only_dot_delimited_reviewed_hosts() {
        let allowed = vec!["cdn.hf.co".to_string()];
        assert!(validate_download_url(
            &reqwest::Url::parse("https://us.aws.cdn.hf.co/file").unwrap(),
            &allowed
        )
        .is_ok());
        assert!(validate_download_url(
            &reqwest::Url::parse("https://evilcdn.hf.co/file").unwrap(),
            &allowed
        )
        .is_err());
        assert!(validate_download_url(
            &reqwest::Url::parse("http://us.aws.cdn.hf.co/file").unwrap(),
            &allowed
        )
        .is_err());
    }

    #[test]
    fn hardware_fit_blocks_clear_shortfalls_and_requires_justified_override() {
        let model = fixture_model(b"GGUFfixture");
        let fit = classify_preflight(&model, ManagedModelOperation::Load, 99, u64::MAX);
        assert_eq!(fit.risk, HardwareFitRisk::Blocked);
        assert_eq!(
            validate_override(&fit, false, true, Some("too short"))
                .expect_err("short reason rejected")
                .code,
            "hardware_fit_blocked"
        );
        validate_override(
            &fit,
            false,
            true,
            Some("I have closed other workloads and accept possible memory pressure."),
        )
        .expect("specific advanced override accepted");
    }

    #[test]
    fn hardware_fit_warning_requires_acknowledgement() {
        let model = fixture_model(b"GGUFfixture");
        let fit = classify_preflight(&model, ManagedModelOperation::Load, 150, u64::MAX);
        assert_eq!(fit.risk, HardwareFitRisk::Warning);
        assert_eq!(
            validate_override(&fit, false, false, None)
                .expect_err("warning must be acknowledged")
                .code,
            "hardware_fit_acknowledgement_required"
        );
        validate_override(&fit, true, false, None).expect("warning acknowledged");
    }

    #[test]
    fn verified_partial_is_atomically_promoted() {
        let root = std::env::temp_dir().join(format!("ark-managed-model-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&root).unwrap();
        let mut bytes = vec![0_u8; 64];
        bytes[..4].copy_from_slice(b"GGUF");
        let model = fixture_model(&bytes);
        let partial = root.join("fixture.gguf.partial");
        let destination = root.join("fixture.gguf");
        std::fs::write(&partial, &bytes).unwrap();
        finalize_partial(&partial, &destination, &model).expect("verified file promoted");
        assert!(!partial.exists());
        assert_eq!(std::fs::read(&destination).unwrap(), bytes);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn digest_mismatch_fails_closed_and_removes_untrusted_partial() {
        let root = std::env::temp_dir().join(format!("ark-managed-model-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&root).unwrap();
        let mut model = fixture_model(b"GGUFtrusted");
        model.size_bytes = b"GGUFaltered".len() as u64;
        let partial = root.join("fixture.gguf.partial");
        let destination = root.join("fixture.gguf");
        std::fs::write(&partial, b"GGUFaltered").unwrap();
        assert_eq!(
            finalize_partial(&partial, &destination, &model)
                .expect_err("digest mismatch rejected")
                .code,
            "managed_model_integrity_failed"
        );
        assert!(!partial.exists());
        assert!(!destination.exists());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn deletion_in_use_check_matches_only_the_exact_running_model() {
        let destination = Path::new("C:\\Models\\managed.gguf");
        assert!(model_is_in_use(
            Some("C:\\Models\\managed.gguf"),
            destination
        ));
        assert!(!model_is_in_use(
            Some("C:\\Models\\other.gguf"),
            destination
        ));
        assert!(!model_is_in_use(None, destination));
    }
}
