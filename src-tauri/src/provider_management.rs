//! ARC-001: the provider-management application workflow (provider config updates, model list
//! refresh, Ollama model pull/delete, and the bundled `llama-server` sidecar lifecycle),
//! extracted from `commands::mod`. Like `generation` and `diagnostics`, these functions take
//! `&AppState`/`&AppHandle` directly rather than being reduced to plain `&Database` functions —
//! they genuinely need database access, network calls to a provider, and (for the built-in
//! runtime) child-process and port management, none of which is meaningful to strip out for
//! testability's sake. `&AppState` (rather than Tauri's `State<AppState>` wrapper) is the actual
//! port: a plain struct a test can construct directly, no running Tauri app required. This is a
//! pure code-motion extraction: no behavior changed, and the full existing test suite continues
//! to pass unchanged, which is itself the acceptance evidence that no regression was introduced
//! (the same standard already established by `generation.rs`).

use crate::db::now;
use crate::errors::AppError;
use crate::providers::{
    ModelInfo, OllamaPullProgress, ProviderConfig, ProviderHealth, ProviderRegistry,
};
use crate::AppState;
use serde::{Deserialize, Serialize};
use std::path::Path;
use sysinfo::Disks;
use tauri::{AppHandle, Emitter};

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RemoteProviderKind {
    OpenAi,
    OpenAiCompatible,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateRemoteProviderRequest {
    pub name: String,
    pub kind: RemoteProviderKind,
    /// Ignored for the curated OpenAI adapter, whose official endpoint is fixed by Ark.
    pub base_url: Option<String>,
    /// FTR-007/SEC-001: remote disclosure must be explicitly accepted before creation.
    #[serde(default)]
    pub acknowledge_remote_risk: bool,
    /// Development-only escape hatch for advanced OpenAI-compatible endpoints.
    #[serde(default)]
    pub allow_insecure_remote: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProviderRequest {
    pub provider_id: String,
    pub base_url: String,
    pub default_model_id: Option<String>,
    pub temperature: Option<f64>,
    pub max_tokens: Option<i64>,
    /// SEC-001: must be `true` to save a base URL that classifies as a public/remote
    /// destination. Defaults to `false` when omitted so older frontend builds fail closed.
    #[serde(default)]
    pub acknowledge_remote_risk: bool,
    /// Explicitly reclassifies a local-only provider as remote. Defaults false for fail-closed
    /// compatibility with older clients.
    #[serde(default)]
    pub convert_to_remote_provider: bool,
    /// Explicit development-mode exception for HTTP outside loopback.
    #[serde(default)]
    pub allow_insecure_remote: bool,
}

pub fn create_remote_provider(
    state: &AppState,
    request: CreateRemoteProviderRequest,
) -> Result<ProviderConfig, AppError> {
    let (provider_type, base_url) = match request.kind {
        RemoteProviderKind::OpenAi => {
            if request.base_url.as_deref().is_some_and(|value| {
                !value.trim().is_empty() && value.trim() != crate::config::OPENAI_PROVIDER_BASE_URL
            }) {
                return Err(AppError::invalid_input(
                    "The curated OpenAI provider uses Ark's fixed official API endpoint.",
                ));
            }
            if request.allow_insecure_remote {
                return Err(AppError::invalid_input(
                    "The curated OpenAI provider cannot enable insecure HTTP.",
                ));
            }
            (
                crate::config::OPENAI_PROVIDER_TYPE,
                crate::config::OPENAI_PROVIDER_BASE_URL,
            )
        }
        RemoteProviderKind::OpenAiCompatible => (
            crate::config::LOCAL_INFERENCE_HOST_PROVIDER_TYPE,
            request.base_url.as_deref().ok_or_else(|| {
                AppError::invalid_input("An OpenAI-compatible provider requires a base URL.")
            })?,
        ),
    };

    crate::commands::lock_db(state)?.create_remote_provider(
        crate::db::CreateRemoteProviderChanges {
            name: &request.name,
            provider_type,
            base_url,
            acknowledge_remote_risk: request.acknowledge_remote_risk,
            allow_insecure_remote: request.allow_insecure_remote,
        },
    )
}

pub async fn delete_provider(
    state: &AppState,
    provider_id: String,
    confirmed: bool,
) -> Result<(), AppError> {
    if !confirmed {
        return Err(AppError::new(
            "provider_delete_confirmation_required",
            "Confirm provider deletion before continuing.",
        ));
    }
    let provider_id =
        crate::validation::validate_entity_id(&provider_id, "Provider ID")?.to_string();
    let provider = crate::commands::lock_db(state)?.get_provider(&provider_id)?;
    if !provider.is_user_managed {
        return Err(AppError::new(
            "provider_delete_forbidden",
            "Built-in provider entries cannot be deleted.",
        ));
    }
    crate::secret_store::delete_user_provider_and_secret(state, provider).await
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RefreshModelsResult {
    pub health: ProviderHealth,
    pub models: Vec<ModelInfo>,
    pub provider: ProviderConfig,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PullOllamaModelRequest {
    pub provider_id: String,
    pub model_name: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteOllamaModelRequest {
    pub provider_id: String,
    pub model_name: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuiltInRuntimeStatus {
    pub running: bool,
    pub port: Option<u16>,
    pub model_path: Option<String>,
    /// COR-012/FTR-006: whether `llama-server` is actually present. Development builds obtain it
    /// through the verified setup script; qualified packages bundle the same verified resource.
    pub binary_installed: bool,
    pub binary_verified: bool,
    pub runtime_provenance: Option<crate::supply_chain::RuntimeProvenance>,
    pub model_provenance: Option<crate::supply_chain::ModelProvenance>,
    pub state: crate::sidecar::RuntimeLifecycleState,
    pub failure: Option<crate::sidecar::RuntimeFailure>,
}

pub fn update_provider(
    state: &AppState,
    mut request: UpdateProviderRequest,
) -> Result<ProviderConfig, AppError> {
    request.provider_id =
        crate::validation::validate_entity_id(&request.provider_id, "Provider ID")?.to_string();
    let temperature = crate::validation::validate_temperature(request.temperature)?;
    let max_tokens = crate::validation::validate_max_tokens(request.max_tokens)?;

    let existing = crate::commands::lock_db(state)?.get_provider(&request.provider_id)?;
    if existing.provider_type == crate::config::OPENAI_PROVIDER_TYPE {
        request.base_url = crate::config::OPENAI_PROVIDER_BASE_URL.to_string();
        request.convert_to_remote_provider = true;
        request.allow_insecure_remote = false;
    }

    crate::commands::lock_db(state)?.update_provider(
        &request.provider_id,
        crate::db::UpdateProviderChanges {
            base_url: &request.base_url,
            default_model_id: request.default_model_id.as_deref(),
            temperature,
            max_tokens,
            acknowledge_remote_risk: request.acknowledge_remote_risk,
            convert_to_remote_provider: request.convert_to_remote_provider,
            allow_insecure_remote: request.allow_insecure_remote,
        },
    )
}

pub async fn refresh_models(
    app: &AppHandle,
    state: &AppState,
    provider_id: String,
) -> Result<RefreshModelsResult, AppError> {
    let started = std::time::Instant::now();
    let provider_id =
        crate::validation::validate_entity_id(&provider_id, "Provider ID")?.to_string();
    let provider = {
        let db = crate::commands::lock_db(state)?;
        db.get_provider(&provider_id)?
    };

    let bearer_token = crate::secret_store::resolve_bearer_token(state, &provider)?;
    let runtime = ProviderRegistry::create_with_bearer_token(provider.clone(), bearer_token)?;
    let request_id = uuid::Uuid::new_v4().to_string();
    let (abort_handle, abort_registration) = futures_util::future::AbortHandle::new_pair();
    {
        let mut active = state.active_provider_refreshes.lock().map_err(|_| {
            AppError::new("state_error", "Could not access active provider refreshes.")
        })?;
        if let Some((_previous_id, previous)) =
            active.insert(provider_id.clone(), (request_id.clone(), abort_handle))
        {
            previous.abort();
        }
    }

    let operation = async {
        let health = runtime.health().await;
        if !health.is_reachable {
            return Ok(RefreshModelsResult {
                health,
                models: Vec::new(),
                provider: provider.clone(),
            });
        }

        let models = runtime.list_models(&now()).await?;
        let refreshed_provider = {
            let db = crate::commands::lock_db(state)?;
            db.upsert_models(&provider_id, &models)?;
            db.get_provider(&provider_id)?
        };
        Ok(RefreshModelsResult {
            health,
            models,
            provider: refreshed_provider,
        })
    };
    let result = match futures_util::future::Abortable::new(operation, abort_registration).await {
        Ok(result) => result,
        Err(_) => Err(AppError::new(
            "provider_refresh_cancelled",
            "Provider refresh was cancelled.",
        )),
    };

    {
        let mut active = state.active_provider_refreshes.lock().map_err(|_| {
            AppError::new("state_error", "Could not access active provider refreshes.")
        })?;
        if active
            .get(&provider_id)
            .is_some_and(|(active_request_id, _)| active_request_id == &request_id)
        {
            active.remove(&provider_id);
        }
    }

    crate::perf_metrics::record_if_enabled(
        app,
        state,
        "perf.provider_refresh",
        Some(&provider_id),
        &[("duration_ms", started.elapsed().as_millis().to_string())],
    );

    result
}

pub fn cancel_provider_refresh(state: &AppState, provider_id: String) -> Result<(), AppError> {
    let provider_id =
        crate::validation::validate_entity_id(&provider_id, "Provider ID")?.to_string();
    let active = state
        .active_provider_refreshes
        .lock()
        .map_err(|_| AppError::new("state_error", "Could not access active provider refreshes."))?;
    if let Some((_request_id, handle)) = active.get(&provider_id) {
        handle.abort();
    }
    Ok(())
}

/// FTR-006: only one pull per provider can be tracked for cancellation at a time, matching the
/// UI's own reality (a single pull-by-name input per provider) — a second pull request for the
/// same provider while one is already in flight replaces the tracked cancellation flag for it,
/// which is harmless since Ollama itself would already reject/queue a concurrent pull of a
/// different model on its own.
pub async fn pull_ollama_model(
    app: &AppHandle,
    state: &AppState,
    mut request: PullOllamaModelRequest,
) -> Result<(), AppError> {
    request.provider_id =
        crate::validation::validate_entity_id(&request.provider_id, "Provider ID")?.to_string();
    request.model_name =
        crate::validation::validate_entity_id(&request.model_name, "Model name")?.to_string();
    let provider = {
        let db = crate::commands::lock_db(state)?;
        db.get_provider(&request.provider_id)?
    };

    let runtime = ProviderRegistry::create(provider)?;

    let cancellation = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    {
        let mut active_pulls = state
            .active_ollama_pulls
            .lock()
            .map_err(|_| AppError::new("state_error", "Could not access active pulls."))?;
        active_pulls.insert(request.provider_id.clone(), cancellation.clone());
    }

    // ARC-003: no destructuring by concrete provider type — a provider that doesn't support
    // pulling models (i.e. everything except Ollama today) fails here with the trait's clear
    // "not supported" error instead of this function needing to know which provider types do.
    let pull_result = runtime
        .pull_model(
            &request.model_name,
            &mut |progress: OllamaPullProgress| {
                app.emit("ollama:pull-progress", &progress).ok();
            },
            &|| cancellation.load(std::sync::atomic::Ordering::Acquire),
        )
        .await;

    state
        .active_ollama_pulls
        .lock()
        .map_err(|_| AppError::new("state_error", "Could not access active pulls."))?
        .remove(&request.provider_id);
    pull_result?;

    // Re-fetch the provider config and refresh the model list after a successful pull.
    // Lock scopes are explicit here to avoid holding MutexGuard across await points.
    let provider_for_refresh = {
        let db = crate::commands::lock_db(state)?;
        db.get_provider(&request.provider_id)?
    };
    let runtime_for_refresh = ProviderRegistry::create(provider_for_refresh)?;
    let models = runtime_for_refresh.list_models(&now()).await?;
    {
        let db = crate::commands::lock_db(state)?;
        db.upsert_models(&request.provider_id, &models)?;
    }

    Ok(())
}

/// FTR-006: signals cancellation to an in-flight `pull_ollama_model` call for this provider, if
/// any — a no-op if none is running (the pull may have already finished, or never started under
/// this provider ID), matching `cancel_import`'s established convention.
pub fn cancel_ollama_pull(state: &AppState, provider_id: &str) -> Result<(), AppError> {
    if let Some(cancellation) = state
        .active_ollama_pulls
        .lock()
        .map_err(|_| AppError::new("state_error", "Could not access active pulls."))?
        .get(provider_id)
    {
        cancellation.store(true, std::sync::atomic::Ordering::Release);
    }
    Ok(())
}

pub async fn delete_ollama_model(
    state: &AppState,
    mut request: DeleteOllamaModelRequest,
) -> Result<(), AppError> {
    request.provider_id =
        crate::validation::validate_entity_id(&request.provider_id, "Provider ID")?.to_string();
    request.model_name =
        crate::validation::validate_entity_id(&request.model_name, "Model name")?.to_string();
    let provider = {
        let db = crate::commands::lock_db(state)?;
        db.get_provider(&request.provider_id)?
    };

    let runtime = ProviderRegistry::create(provider)?;
    runtime.delete_model(&request.model_name).await?;

    let db = crate::commands::lock_db(state)?;
    db.mark_model_unavailable(&request.provider_id, &request.model_name)?;

    Ok(())
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiskSpaceInfo {
    pub total_bytes: u64,
    pub available_bytes: u64,
}

/// UX-011: best-effort disk-space signal for the model picker's "will this fit" hint before an
/// Ollama pull starts. Checks the workspace's own volume (the same `Disks` lookup
/// `diagnostics::run_diagnostics` already uses), since Ark cannot query Ollama's actual
/// model-storage path cross-platform — a documented approximation, not the pull target's real
/// free space. Never blocks a pull on its own; callers surface it as a warning with a way past.
pub fn check_disk_space(state: &AppState) -> Result<DiskSpaceInfo, AppError> {
    let workspace_root = {
        let workspace_info = state
            .workspace
            .lock()
            .map_err(|_| AppError::new("state_error", "Could not access workspace state."))?
            .clone();
        std::path::PathBuf::from(workspace_info.root_path)
    };
    let disks = Disks::new_with_refreshed_list();
    let workspace_disk = disks
        .iter()
        .filter(|disk| workspace_root.starts_with(disk.mount_point()))
        .max_by_key(|disk| disk.mount_point().as_os_str().len());
    Ok(DiskSpaceInfo {
        total_bytes: workspace_disk.map(|disk| disk.total_space()).unwrap_or(0),
        available_bytes: workspace_disk
            .map(|disk| disk.available_space())
            .unwrap_or(0),
    })
}

pub async fn get_built_in_runtime_status(
    app: &AppHandle,
    state: &AppState,
) -> Result<BuiltInRuntimeStatus, AppError> {
    let binary = crate::sidecar::llama_server_binary(app);
    let binary_installed = binary.exists();
    // PERF-002: `verify_runtime` hashes every installed runtime file (including the
    // llama-server binary itself) with SHA-256 — real, potentially slow, CPU/IO-bound work that
    // must not run inline on the async executor, since this function is awaited directly in
    // `bootstrap()`'s critical path before the composer becomes interactive. `spawn_blocking`
    // moves it onto Tokio's blocking thread pool, matching `secret_store.rs`'s established
    // pattern for the same class of work.
    let runtime_verification = if binary_installed {
        let binary_for_verification = binary.clone();
        Some(
            tokio::task::spawn_blocking(move || {
                crate::supply_chain::verify_runtime(&binary_for_verification)
            })
            .await
            .unwrap_or_else(|_| {
                Err(AppError::new(
                    "runtime_verification_failed",
                    "Runtime verification worker did not complete. Retry.",
                ))
            }),
        )
    } else {
        None
    };
    let runtime_provenance = runtime_verification
        .as_ref()
        .and_then(|result| result.as_ref().ok())
        .cloned();
    let binary_verified = runtime_provenance.is_some();
    let health_probe = {
        let mut sidecar = crate::commands::lock_sidecar(state)?;
        let running = sidecar.reconcile_process();
        if !binary_installed && !running {
            sidecar.mark_unavailable_binary(&binary);
        } else if let Some(Err(error)) = &runtime_verification {
            sidecar.mark_failure(
                crate::sidecar::RuntimeFailureCategory::SupplyChainVerificationFailed,
                error.message.clone(),
            );
        } else if !running {
            if let Some(model_path) = sidecar.model_path() {
                if !Path::new(&model_path).is_file() {
                    sidecar.mark_unavailable_model(
                        &model_path,
                        "The configured GGUF model file is no longer available.",
                    );
                }
            }
        }
        if running
            && matches!(
                sidecar.state(),
                crate::sidecar::RuntimeLifecycleState::Healthy
                    | crate::sidecar::RuntimeLifecycleState::Degraded
            )
        {
            sidecar.port().zip(sidecar.api_key())
        } else {
            None
        }
    };

    if let Some((port, api_key)) = health_probe {
        let health = crate::sidecar::check_health(port, &api_key).await;
        let mut sidecar = crate::commands::lock_sidecar(state)?;
        match health {
            Ok(()) => sidecar.mark_healthy(),
            Err(failure) => sidecar.mark_failure(failure.category, failure.message),
        }
    }

    let mut sidecar = crate::commands::lock_sidecar(state)?;
    let model_path = sidecar.model_path();
    let diagnostics = sidecar.diagnostics(false);
    let model_provenance = crate::supply_chain::load_model_provenance(app)?;
    Ok(BuiltInRuntimeStatus {
        running: diagnostics.pid.is_some(),
        port: diagnostics.port,
        model_path,
        binary_installed,
        binary_verified,
        runtime_provenance,
        model_provenance,
        state: diagnostics.state,
        failure: diagnostics.failure,
    })
}

pub async fn stop_built_in_runtime(state: &AppState) -> Result<(), AppError> {
    let mut sidecar = crate::commands::lock_sidecar(state)?;
    sidecar
        .stop()
        .map_err(|failure| AppError::provider(failure.message))
}

pub async fn start_built_in_runtime(
    model_path: String,
    model_source: String,
    model_license: String,
    app: &AppHandle,
    state: &AppState,
) -> Result<BuiltInRuntimeStatus, AppError> {
    start_built_in_runtime_with_expected_digest(
        model_path,
        model_source,
        model_license,
        None,
        app,
        state,
    )
    .await
}

/// FTR-006: the managed catalog path adds an immutable expected digest. Manual imports keep the
/// existing provenance-only flow; both paths share one process/proxy lifecycle implementation.
pub async fn start_built_in_runtime_with_expected_digest(
    model_path: String,
    model_source: String,
    model_license: String,
    expected_sha256: Option<&str>,
    app: &AppHandle,
    state: &AppState,
) -> Result<BuiltInRuntimeStatus, AppError> {
    use crate::config::BUILT_IN_PROVIDER_ID;
    use crate::sidecar::{
        generate_runtime_api_key, llama_server_binary, spawn_llama_server, wait_for_assigned_port,
        wait_for_ready, RuntimeFailureCategory,
    };

    // COR-008: fail fast with a specific, actionable error rather than letting spawn_llama_server
    // launch and then time out 30 seconds later against a path that was never going to work.
    let validated_model_path = match crate::validation::validate_model_path(&model_path) {
        Ok(path) => path,
        Err(error) => {
            crate::commands::lock_sidecar(state)?
                .mark_unavailable_model(&model_path, &error.message);
            return Err(error);
        }
    };
    // SEC-007: the cheap check above only validates the path's shape (extension, existence,
    // not-a-directory). This reads the file itself — rejects a symlinked, truncated, or
    // non-GGUF-signed file before it reaches the launch path.
    if let Err(error) = crate::validation::validate_gguf_file(&validated_model_path) {
        crate::commands::lock_sidecar(state)?.mark_unavailable_model(&model_path, &error.message);
        return Err(error);
    }

    let binary = llama_server_binary(app);
    if !binary.exists() {
        crate::commands::lock_sidecar(state)?.mark_unavailable_binary(&binary);
        return Err(AppError::provider(if cfg!(debug_assertions) {
            "Built-in runtime not installed. Run the platform setup script from the repo root."
        } else {
            "The packaged built-in runtime is missing. Reinstall this Ark package."
        }));
    }
    let runtime_provenance = crate::supply_chain::verify_runtime(&binary).inspect_err(|error| {
        if let Ok(mut sidecar) = crate::commands::lock_sidecar(state) {
            sidecar.mark_failure(
                RuntimeFailureCategory::SupplyChainVerificationFailed,
                error.message.clone(),
            );
        }
    })?;
    let model_provenance = crate::supply_chain::verify_and_record_model(
        app,
        &validated_model_path,
        &model_source,
        &model_license,
    )?;
    if expected_sha256.is_some_and(|expected| expected != model_provenance.sha256) {
        crate::commands::lock_sidecar(state)?.mark_unavailable_model(
            &model_path,
            "The managed model no longer matches its reviewed catalog digest.",
        );
        return Err(AppError::new(
            "managed_model_integrity_failed",
            "The managed model no longer matches its reviewed catalog digest.",
        ));
    }
    let model_path = model_provenance.path.clone();
    {
        let mut sidecar = crate::commands::lock_sidecar(state)?;
        sidecar
            .stop()
            .map_err(|failure| AppError::provider(failure.message))?;
        sidecar.begin_start(&model_path, &binary);
    }

    // SEC-002: a fresh, high-entropy secret for this launch only — never logged, never
    // persisted to the database or any file, held only in AppState for the process lifetime.
    let api_key = generate_runtime_api_key();

    let child = match spawn_llama_server(&binary, &model_path, &api_key) {
        Ok(child) => child,
        Err(error) => {
            crate::commands::lock_sidecar(state)?
                .mark_failure(RuntimeFailureCategory::SpawnFailed, &error.message);
            return Err(error);
        }
    };

    {
        let mut sidecar = crate::commands::lock_sidecar(state)?;
        sidecar.attach_process(child, None, model_path.clone(), api_key.clone());
    }

    let port = match wait_for_assigned_port(std::sync::Arc::clone(&state.sidecar)).await {
        Ok(port) => port,
        Err(failure) => {
            let mut sidecar = crate::commands::lock_sidecar(state)?;
            sidecar.mark_failure(failure.category, failure.message.clone());
            let safe_failure = sidecar.safe_failure_with_excerpt().unwrap_or(failure);
            let _ = sidecar.stop();
            sidecar.mark_failure(safe_failure.category, safe_failure.message.clone());
            return Err(AppError::provider(safe_failure.message));
        }
    };

    if let Err(failure) =
        wait_for_ready(std::sync::Arc::clone(&state.sidecar), port, &api_key).await
    {
        let mut sidecar = crate::commands::lock_sidecar(state)?;
        sidecar.mark_failure(failure.category, failure.message.clone());
        let safe_failure = sidecar.safe_failure_with_excerpt().unwrap_or(failure);
        let _ = sidecar.stop();
        sidecar.mark_failure(safe_failure.category, safe_failure.message.clone());
        return Err(AppError::provider(safe_failure.message));
    }

    // SEC-002: llama-server's own HTTP server exempts several routes from its `--api-key`
    // check and reflects `Origin` into permissive CORS headers (see `proxy.rs`'s module doc).
    // Ark never points its own traffic, and never persists `base_url`, at that raw port
    // directly — every request, including from Ark itself, goes through this authenticating,
    // CORS-sanitizing proxy instead.
    let (proxy_port, proxy_task) = match crate::proxy::spawn_auth_proxy(port, api_key.clone()).await
    {
        Ok(spawned) => spawned,
        Err(error) => {
            let mut sidecar = crate::commands::lock_sidecar(state)?;
            let _ = sidecar.stop();
            sidecar.mark_failure(
                RuntimeFailureCategory::PortUnavailable,
                format!("Ark could not start the runtime's authenticating proxy: {error}"),
            );
            return Err(AppError::provider(
                "Ark could not start the managed runtime's authenticating proxy.",
            ));
        }
    };
    {
        let mut sidecar = crate::commands::lock_sidecar(state)?;
        sidecar.attach_proxy(proxy_port, proxy_task);
    }

    let base_url = format!("http://127.0.0.1:{proxy_port}");
    if let Err(error) =
        crate::commands::lock_db(state)?.update_provider_base_url(BUILT_IN_PROVIDER_ID, &base_url)
    {
        let mut sidecar = crate::commands::lock_sidecar(state)?;
        let _ = sidecar.stop();
        sidecar.mark_failure(
            RuntimeFailureCategory::StateUnavailable,
            "Ark could not persist the managed runtime endpoint.",
        );
        return Err(error);
    }

    // OPS-001: a lifecycle anchor for support conversations — deliberately no model path or
    // any other filesystem detail, just confirmation of when a launch reached Healthy.
    if let Ok(mut log) = state.observability_log.lock() {
        log.record(
            crate::observability::LogLevel::Info,
            "runtime",
            None,
            "managed runtime became healthy",
        );
    }

    Ok(BuiltInRuntimeStatus {
        running: true,
        port: Some(proxy_port),
        model_path: Some(model_path),
        binary_installed: true,
        binary_verified: true,
        runtime_provenance: Some(runtime_provenance),
        model_provenance: Some(model_provenance),
        state: crate::sidecar::RuntimeLifecycleState::Healthy,
        failure: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::DEFAULT_PROVIDER_ID;
    use crate::db::Database;
    use crate::sidecar::SidecarState;
    use std::collections::HashMap;
    use std::env;
    use std::sync::atomic::AtomicBool;
    use std::sync::Mutex;
    use uuid::Uuid;

    /// ARC-001 acceptance evidence: `AppState` is a plain struct, so it can be constructed
    /// directly here with no running Tauri app, no `tauri::App`/`AppHandle`, and no IPC —
    /// proving the "use cases depend on explicit ports and can run with in-memory/test
    /// adapters" acceptance criterion for the functions in this module that don't also need
    /// `&AppHandle` (event emission, path resolution).
    fn test_app_state() -> (AppState, std::path::PathBuf) {
        let path = env::temp_dir().join(format!(
            "ark-provider-management-test-{}.sqlite3",
            Uuid::new_v4()
        ));
        let db = Database::open(&path).expect("database opens");
        let read_db = Database::open_read_replica(&path).expect("read replica opens");
        (
            AppState {
                db: Mutex::new(db),
                workspace: Mutex::new(crate::workspace::WorkspaceInfo {
                    root_path: path
                        .parent()
                        .expect("test path parent")
                        .display()
                        .to_string(),
                    database_path: path.display().to_string(),
                    default_root_path: path
                        .parent()
                        .expect("test path parent")
                        .display()
                        .to_string(),
                    config_path: path.with_extension("json").display().to_string(),
                    is_portable: false,
                    requires_restart: false,
                }),
                read_db: Mutex::new(read_db),
                workspace_open_error: Mutex::new(None),
                active_streams: Mutex::new(HashMap::new()),
                pending_streams: Mutex::new(HashMap::new()),
                active_imports: Mutex::new(HashMap::new()),
                active_ollama_pulls: Mutex::new(HashMap::new()),
                active_provider_refreshes: Mutex::new(HashMap::new()),
                active_managed_model_downloads: Mutex::new(HashMap::new()),
                active_code_runs: Mutex::new(HashMap::new()),
                storage_maintenance: AtomicBool::new(false),
                sidecar: std::sync::Arc::new(Mutex::new(SidecarState::new())),
                observability_log: std::sync::Arc::new(Mutex::new(
                    crate::observability::DiagnosticsLog::new(),
                )),
                companion_api: Mutex::new(None),
            },
            path,
        )
    }

    #[test]
    fn update_provider_saves_a_local_destination_through_the_appstate_port() {
        let (state, path) = test_app_state();

        let updated = update_provider(
            &state,
            UpdateProviderRequest {
                provider_id: DEFAULT_PROVIDER_ID.to_string(),
                base_url: "http://localhost:11434".to_string(),
                default_model_id: Some("llama3.2:latest".to_string()),
                temperature: Some(0.5),
                max_tokens: Some(1024),
                acknowledge_remote_risk: false,
                convert_to_remote_provider: false,
                allow_insecure_remote: false,
            },
        )
        .expect("local destination updates without acknowledgment");

        assert_eq!(updated.default_model_id.as_deref(), Some("llama3.2:latest"));
        assert!(updated.is_local);

        drop(state);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn update_provider_rejects_an_unacknowledged_remote_destination_through_the_appstate_port() {
        let (state, path) = test_app_state();

        let error = update_provider(
            &state,
            UpdateProviderRequest {
                provider_id: DEFAULT_PROVIDER_ID.to_string(),
                base_url: "https://api.example.com".to_string(),
                default_model_id: None,
                temperature: None,
                max_tokens: None,
                acknowledge_remote_risk: false,
                convert_to_remote_provider: false,
                allow_insecure_remote: false,
            },
        )
        .expect_err("a public destination requires explicit acknowledgment");

        assert_eq!(error.code, "destination_requires_remote_provider_class");

        drop(state);
        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn remote_provider_lifecycle_requires_both_risk_and_delete_confirmation() {
        let (state, path) = test_app_state();
        let created = create_remote_provider(
            &state,
            CreateRemoteProviderRequest {
                name: "OpenAI".to_string(),
                kind: RemoteProviderKind::OpenAi,
                base_url: None,
                acknowledge_remote_risk: true,
                allow_insecure_remote: false,
            },
        )
        .expect("explicitly acknowledged provider created");
        assert!(created.is_user_managed);
        assert_eq!(created.provider_type, crate::config::OPENAI_PROVIDER_TYPE);

        let error = delete_provider(&state, created.id.clone(), false)
            .await
            .expect_err("backend confirmation is mandatory");
        assert_eq!(error.code, "provider_delete_confirmation_required");
        assert!(crate::commands::lock_db(&state)
            .expect("database lock")
            .get_provider(&created.id)
            .is_ok());

        delete_provider(&state, created.id.clone(), true)
            .await
            .expect("confirmed provider deletion succeeds");
        assert_eq!(
            crate::commands::lock_db(&state)
                .expect("database lock")
                .get_provider(&created.id)
                .expect_err("provider removed")
                .code,
            "not_found"
        );

        drop(state);
        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn cancel_provider_refresh_aborts_the_registered_in_flight_future() {
        let (state, path) = test_app_state();
        let (handle, registration) = futures_util::future::AbortHandle::new_pair();
        state
            .active_provider_refreshes
            .lock()
            .expect("refresh lock")
            .insert(
                DEFAULT_PROVIDER_ID.to_string(),
                ("request-1".to_string(), handle),
            );

        cancel_provider_refresh(&state, DEFAULT_PROVIDER_ID.to_string())
            .expect("cancellation request succeeds");
        let result =
            futures_util::future::Abortable::new(std::future::pending::<()>(), registration).await;
        assert!(
            result.is_err(),
            "registered provider future must be aborted"
        );

        // Repeated/late cancellation is intentionally harmless.
        cancel_provider_refresh(&state, "no-active-refresh".to_string())
            .expect("missing refresh is a no-op");
        drop(state);
        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn stop_built_in_runtime_is_a_harmless_no_op_when_nothing_is_running() {
        let (state, path) = test_app_state();

        stop_built_in_runtime(&state)
            .await
            .expect("stopping an idle sidecar is not an error");
        assert!(!crate::commands::lock_sidecar(&state)
            .expect("sidecar lock")
            .reconcile_process());

        drop(state);
        let _ = std::fs::remove_file(path);
    }

    /// UX-011: `test_app_state`'s workspace root is a real temp-directory path, so this exercises
    /// the actual `Disks` lookup rather than mocking it — platform-portable by asserting only the
    /// invariant every real filesystem satisfies (available never exceeds total), not exact byte
    /// counts, which vary by machine and CI runner.
    #[test]
    fn check_disk_space_reports_consistent_non_negative_values_for_a_real_workspace_path() {
        let (state, path) = test_app_state();

        let info = check_disk_space(&state).expect("disk space check succeeds");
        assert!(info.available_bytes <= info.total_bytes);

        drop(state);
        let _ = std::fs::remove_file(path);
    }
}
