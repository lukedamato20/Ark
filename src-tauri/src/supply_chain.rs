use crate::errors::AppError;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};

const MAX_PROVENANCE_BYTES: u64 = 256 * 1024;
const MAX_MODEL_BYTES: u64 = 1024 * 1024 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct InstalledFileProvenance {
    pub name: String,
    pub size_bytes: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeProvenance {
    pub schema_version: u32,
    pub runtime: String,
    pub version: String,
    pub source_repository: String,
    pub source_commit: String,
    pub license: String,
    pub license_url: String,
    pub artifact_file_name: String,
    pub artifact_url: String,
    pub artifact_sha256: String,
    pub runtime_sha256: String,
    pub platform: String,
    pub arch: String,
    pub verified_at: String,
    pub installed_files: Vec<InstalledFileProvenance>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ModelProvenance {
    pub path: String,
    pub source: String,
    pub license: String,
    pub sha256: String,
    pub size_bytes: u64,
    pub verified_at: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReviewedManifest {
    schema_version: u32,
    runtime: ReviewedRuntime,
    artifacts: Vec<ReviewedArtifact>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReviewedRuntime {
    name: String,
    version: String,
    source_repository: String,
    source_commit: String,
    license: String,
    license_url: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReviewedArtifact {
    platform: String,
    arch: String,
    file_name: String,
    url: String,
    size_bytes: u64,
    sha256: String,
}

fn current_target() -> (&'static str, &'static str) {
    let platform = if cfg!(target_os = "windows") {
        "win32"
    } else if cfg!(target_os = "macos") {
        "darwin"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else {
        "unsupported"
    };
    let arch = if cfg!(target_arch = "x86_64") {
        "x64"
    } else if cfg!(target_arch = "aarch64") {
        "arm64"
    } else {
        "unsupported"
    };
    (platform, arch)
}

fn sha256_file(path: &Path, maximum_bytes: u64) -> Result<(u64, String), AppError> {
    let metadata = path.metadata().map_err(|error| {
        AppError::new(
            "provenance_verification_failed",
            format!("Could not inspect {}: {error}", path.display()),
        )
    })?;
    if !metadata.is_file() || metadata.len() == 0 || metadata.len() > maximum_bytes {
        return Err(AppError::new(
            "provenance_verification_failed",
            format!(
                "{} is not a non-empty regular file within the verification size limit.",
                path.display()
            ),
        ));
    }
    let mut file = File::open(path).map_err(|error| {
        AppError::new(
            "provenance_verification_failed",
            format!("Could not read {}: {error}", path.display()),
        )
    })?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; 1024 * 1024];
    let mut read = 0_u64;
    loop {
        let count = file.read(&mut buffer).map_err(|error| {
            AppError::new(
                "provenance_verification_failed",
                format!("Could not hash {}: {error}", path.display()),
            )
        })?;
        if count == 0 {
            break;
        }
        read = read.saturating_add(count as u64);
        if read > maximum_bytes {
            return Err(AppError::new(
                "provenance_verification_failed",
                "File grew beyond the verification size limit while being hashed.",
            ));
        }
        hasher.update(&buffer[..count]);
    }
    Ok((read, format!("{:x}", hasher.finalize())))
}

fn read_bounded(path: &Path) -> Result<String, AppError> {
    let metadata = path.metadata().map_err(|error| {
        AppError::new(
            "runtime_provenance_missing",
            format!("Runtime provenance is unavailable: {error}"),
        )
    })?;
    if !metadata.is_file() || metadata.len() > MAX_PROVENANCE_BYTES {
        return Err(AppError::new(
            "runtime_provenance_invalid",
            "Runtime provenance is not a bounded regular file.",
        ));
    }
    std::fs::read_to_string(path).map_err(|error| {
        AppError::new(
            "runtime_provenance_invalid",
            format!("Runtime provenance could not be read: {error}"),
        )
    })
}

pub fn verify_runtime(binary: &Path) -> Result<RuntimeProvenance, AppError> {
    let directory = binary.parent().ok_or_else(|| {
        AppError::new(
            "runtime_provenance_invalid",
            "Runtime binary has no containing directory.",
        )
    })?;
    let provenance: RuntimeProvenance = serde_json::from_str(&read_bounded(
        &directory.join("runtime-provenance.json"),
    )?)
    .map_err(|error| {
        AppError::new(
            "runtime_provenance_invalid",
            format!("Runtime provenance JSON is invalid: {error}"),
        )
    })?;
    let reviewed: ReviewedManifest =
        serde_json::from_str(include_str!("../../config/native-artifacts.json"))
            .map_err(|error| AppError::new("runtime_provenance_invalid", error.to_string()))?;
    let (platform, arch) = current_target();
    let artifact = reviewed
        .artifacts
        .iter()
        .find(|item| item.platform == platform && item.arch == arch)
        .ok_or_else(|| {
            AppError::new(
                "runtime_provenance_invalid",
                "No reviewed runtime artifact exists for this target.",
            )
        })?;
    let metadata_matches = reviewed.schema_version == 1
        && provenance.schema_version == 1
        && provenance.runtime == reviewed.runtime.name
        && provenance.version == reviewed.runtime.version
        && provenance.source_repository == reviewed.runtime.source_repository
        && provenance.source_commit == reviewed.runtime.source_commit
        && provenance.license == reviewed.runtime.license
        && provenance.license_url == reviewed.runtime.license_url
        && provenance.platform == platform
        && provenance.arch == arch
        && provenance.artifact_file_name == artifact.file_name
        && provenance.artifact_url == artifact.url
        && provenance.artifact_sha256 == artifact.sha256
        && artifact.size_bytes > 0;
    if !metadata_matches {
        return Err(AppError::new(
            "runtime_provenance_invalid",
            "Installed runtime provenance does not match Ark's reviewed artifact manifest.",
        ));
    }

    if provenance.installed_files.is_empty() {
        return Err(AppError::new(
            "runtime_provenance_invalid",
            "Runtime provenance contains no installed-file hashes.",
        ));
    }
    let expected: HashMap<_, _> = provenance
        .installed_files
        .iter()
        .map(|item| (item.name.as_str(), item))
        .collect();
    if expected.len() != provenance.installed_files.len() {
        return Err(AppError::new(
            "runtime_provenance_invalid",
            "Runtime provenance contains duplicate installed-file names.",
        ));
    }
    let allowed_metadata: HashSet<&str> = [".gitkeep", "runtime-provenance.json"]
        .into_iter()
        .collect();
    for entry in std::fs::read_dir(directory).map_err(AppError::from)? {
        let entry = entry.map_err(AppError::from)?;
        let name = entry.file_name().to_string_lossy().to_string();
        let file_type = entry.file_type().map_err(AppError::from)?;
        if allowed_metadata.contains(name.as_str()) {
            continue;
        }
        if !file_type.is_file() || !expected.contains_key(name.as_str()) {
            return Err(AppError::new(
                "runtime_provenance_invalid",
                format!("Runtime directory contains an unreviewed entry: {name}"),
            ));
        }
    }
    for item in &provenance.installed_files {
        if Path::new(&item.name)
            .file_name()
            .and_then(|name| name.to_str())
            != Some(&item.name)
            || !valid_sha256(&item.sha256)
        {
            return Err(AppError::new(
                "runtime_provenance_invalid",
                "Runtime installed-file provenance contains an invalid name or digest.",
            ));
        }
        let (size, digest) = sha256_file(&directory.join(&item.name), MAX_MODEL_BYTES)?;
        if size != item.size_bytes || digest != item.sha256 {
            return Err(AppError::new(
                "runtime_provenance_mismatch",
                format!(
                    "Installed runtime file '{}' failed hash verification.",
                    item.name
                ),
            ));
        }
    }
    let server_name = binary
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    let server = expected.get(server_name).ok_or_else(|| {
        AppError::new(
            "runtime_provenance_invalid",
            "Runtime server is absent from installed-file provenance.",
        )
    })?;
    if server.sha256 != provenance.runtime_sha256 {
        return Err(AppError::new(
            "runtime_provenance_invalid",
            "Runtime executable digest disagrees with installed-file provenance.",
        ));
    }
    Ok(provenance)
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn model_provenance_path(app: &AppHandle) -> Result<PathBuf, AppError> {
    app.path()
        .app_config_dir()
        .map(|path| path.join("model-provenance.json"))
        .map_err(|error| {
            AppError::new(
                "model_provenance_path_unavailable",
                format!("Could not resolve model provenance storage: {error}"),
            )
        })
}

pub fn verify_and_record_model(
    app: &AppHandle,
    path: &Path,
    source: &str,
    license: &str,
) -> Result<ModelProvenance, AppError> {
    let source = validate_metadata_text(source, "Model source", 2_048)?;
    let license = validate_metadata_text(license, "Model license", 256)?;
    let canonical = path.canonicalize().map_err(AppError::from)?;
    let (size_bytes, sha256) = sha256_file(&canonical, MAX_MODEL_BYTES)?;
    let record = ModelProvenance {
        path: canonical.display().to_string(),
        source: source.to_string(),
        license: license.to_string(),
        sha256,
        size_bytes,
        verified_at: chrono::Utc::now().to_rfc3339(),
    };
    save_model_provenance(app, &record)?;
    Ok(record)
}

pub fn load_model_provenance(app: &AppHandle) -> Result<Option<ModelProvenance>, AppError> {
    let path = model_provenance_path(app)?;
    if !path.exists() {
        return Ok(None);
    }
    serde_json::from_str(&read_bounded(&path)?)
        .map(Some)
        .map_err(|error| {
            AppError::new(
                "model_provenance_invalid",
                format!("Stored model provenance is invalid: {error}"),
            )
        })
}

fn save_model_provenance(app: &AppHandle, record: &ModelProvenance) -> Result<(), AppError> {
    let destination = model_provenance_path(app)?;
    let parent = destination.parent().ok_or_else(|| {
        AppError::new(
            "model_provenance_write_failed",
            "Model provenance path has no parent directory.",
        )
    })?;
    std::fs::create_dir_all(parent).map_err(AppError::from)?;
    let suffix = uuid::Uuid::new_v4();
    let next = parent.join(format!("model-provenance.{suffix}.next"));
    let previous = parent.join(format!("model-provenance.{suffix}.previous"));
    let bytes = serde_json::to_vec_pretty(record)
        .map_err(|error| AppError::new("model_provenance_write_failed", error.to_string()))?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&next)
        .map_err(AppError::from)?;
    file.write_all(&bytes).map_err(AppError::from)?;
    file.sync_all().map_err(AppError::from)?;
    drop(file);
    let had_destination = destination.exists();
    if had_destination {
        std::fs::rename(&destination, &previous).map_err(AppError::from)?;
    }
    if let Err(error) = std::fs::rename(&next, &destination) {
        if had_destination {
            let _ = std::fs::rename(&previous, &destination);
        }
        let _ = std::fs::remove_file(&next);
        return Err(AppError::from(error));
    }
    if had_destination {
        std::fs::remove_file(previous).map_err(AppError::from)?;
    }
    Ok(())
}

fn validate_metadata_text<'a>(
    value: &'a str,
    label: &str,
    maximum: usize,
) -> Result<&'a str, AppError> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.len() > maximum || trimmed.chars().any(char::is_control) {
        return Err(AppError::invalid_input(format!(
            "{label} must be non-empty, at most {maximum} bytes, and contain no control characters."
        )));
    }
    Ok(trimmed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashes_files_without_loading_them_whole_and_enforces_bounds() {
        let path = std::env::temp_dir().join(format!("ark-hash-{}", uuid::Uuid::new_v4()));
        std::fs::write(&path, b"abc").expect("write fixture");
        let (size, digest) = sha256_file(&path, 3).expect("hash fixture");
        assert_eq!(size, 3);
        assert_eq!(
            digest,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            sha256_file(&path, 2).expect_err("bound rejects file").code,
            "provenance_verification_failed"
        );
        std::fs::remove_file(path).expect("remove fixture");
    }

    #[test]
    fn metadata_fields_reject_empty_oversized_and_control_text() {
        assert!(validate_metadata_text("publisher", "source", 16).is_ok());
        assert!(validate_metadata_text("", "source", 16).is_err());
        assert!(validate_metadata_text("line\nbreak", "source", 16).is_err());
        assert!(validate_metadata_text("too long", "source", 3).is_err());
    }

    #[test]
    fn runtime_verification_rejects_tampering_and_unreviewed_files() {
        let directory = std::env::temp_dir().join(format!("ark-runtime-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&directory).expect("create fixture directory");
        let server_name = if cfg!(windows) {
            "llama-server.exe"
        } else {
            "llama-server"
        };
        let binary = directory.join(server_name);
        std::fs::write(&binary, b"reviewed runtime bytes").expect("write runtime");
        let (size_bytes, runtime_sha256) = sha256_file(&binary, 1024).expect("hash runtime");
        let reviewed: ReviewedManifest =
            serde_json::from_str(include_str!("../../config/native-artifacts.json"))
                .expect("parse reviewed manifest");
        let (platform, arch) = current_target();
        let artifact = reviewed
            .artifacts
            .iter()
            .find(|item| item.platform == platform && item.arch == arch)
            .expect("target artifact");
        let provenance = RuntimeProvenance {
            schema_version: 1,
            runtime: reviewed.runtime.name,
            version: reviewed.runtime.version,
            source_repository: reviewed.runtime.source_repository,
            source_commit: reviewed.runtime.source_commit,
            license: reviewed.runtime.license,
            license_url: reviewed.runtime.license_url,
            artifact_file_name: artifact.file_name.clone(),
            artifact_url: artifact.url.clone(),
            artifact_sha256: artifact.sha256.clone(),
            runtime_sha256: runtime_sha256.clone(),
            platform: platform.to_string(),
            arch: arch.to_string(),
            verified_at: "2026-08-14T00:00:00Z".to_string(),
            installed_files: vec![InstalledFileProvenance {
                name: server_name.to_string(),
                size_bytes,
                sha256: runtime_sha256,
            }],
        };
        std::fs::write(
            directory.join("runtime-provenance.json"),
            serde_json::to_vec(&provenance).expect("serialize provenance"),
        )
        .expect("write provenance");

        assert_eq!(
            verify_runtime(&binary).expect("verified runtime"),
            provenance
        );
        std::fs::write(&binary, b"tampered runtime bytes").expect("tamper runtime");
        assert_eq!(
            verify_runtime(&binary)
                .expect_err("tampering rejected")
                .code,
            "runtime_provenance_mismatch"
        );
        std::fs::write(&binary, b"reviewed runtime bytes").expect("restore runtime");
        std::fs::write(directory.join("unreviewed.dll"), b"unknown").expect("write unknown file");
        assert_eq!(
            verify_runtime(&binary)
                .expect_err("unknown file rejected")
                .code,
            "runtime_provenance_invalid"
        );
        std::fs::remove_dir_all(directory).expect("remove fixture");
    }
}
