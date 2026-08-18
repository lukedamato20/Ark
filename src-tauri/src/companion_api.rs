//! FTR-010: a disabled-by-default local HTTP API for external integrations and the future
//! mobile companion (MOB-009). Reuses SEC-002's proven proxy pattern (`proxy.rs`): a
//! loopback-only listener, custom-header bearer auth with zero exempt routes (not even
//! `/v1/health`), and no response this server sends ever carries an `Access-Control-*` header —
//! so a cross-origin browser page can never read a response even if it somehow knew the token,
//! and a real CORS preflight (which never carries the caller's intended `Authorization` header)
//! always fails the auth check first regardless.
//!
//! Every route is served by the exact same `Database`/application-service functions the Tauri
//! command surface uses. Generation and cancellation delegate to `generation.rs`; reads use the
//! cached database inventory. There is no second generation system, raw SQL, provider-secret
//! read, provider refresh/network discovery, or filesystem access reachable from the wire.
//!
//! **Scope of this pass, matching SEC-010's threat model and stated honestly rather than
//! silently narrowed:** SEC-010 calls for loopback and paired-LAN modes to have *distinct*
//! controls; paired-LAN mode depends on MOB-009's per-device pairing lifecycle, which does not
//! exist yet, so this implements the loopback control only and binds `127.0.0.1` exclusively —
//! there is no LAN-reachable mode to accidentally enable. Mutations require persisted,
//! transactionally checked idempotency keys; provider/model selection responses deliberately
//! omit endpoints, keychain references, and raw adapter metadata.

use crate::errors::AppError;
use crate::observability::LogLevel;
use crate::{chat::ConversationListRequest, AppState};
use bytes::Bytes;
use http_body_util::combinators::BoxBody;
use http_body_util::{BodyExt, Empty, Full};
use hyper::body::Incoming;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Method, Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::VecDeque;
use std::convert::Infallible;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager};
use tokio::net::TcpListener;
use tokio::task::JoinHandle;

pub const COMPANION_API_VERSION: &str = "v1";
const OPENAPI_DOCUMENT: &str = include_str!("../../docs/companion-api.openapi.json");
const OPENAPI_PATH: &str = "/v1/openapi.json";
const HEALTH_PATH: &str = "/v1/health";
const CONVERSATIONS_PATH: &str = "/v1/conversations";
const CONVERSATION_PATH_PREFIX: &str = "/v1/conversations/";
const MESSAGES_PATH_SUFFIX: &str = "/messages";
const PROVIDERS_PATH: &str = "/v1/providers";
const PROVIDER_PATH_PREFIX: &str = "/v1/providers/";
const MESSAGE_PATH_PREFIX: &str = "/v1/messages/";
const CANCEL_PATH_SUFFIX: &str = "/cancel";
const MAX_REQUEST_BODY_BYTES: usize = 64 * 1024;
const RATE_LIMIT_MAX_REQUESTS: usize = 120;
const RATE_LIMIT_WINDOW: Duration = Duration::from_secs(60);
const DEFAULT_PAGE_LIMIT: u32 = 50;

type ApiBody = BoxBody<Bytes, Infallible>;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CreateConversationBody {
    #[serde(default)]
    title: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct UpdateConversationBody {
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    archived: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SendMessageBody {
    content: String,
    provider_id: String,
    model: String,
    #[serde(default)]
    temperature: Option<f64>,
    #[serde(default)]
    max_tokens: Option<i64>,
    #[serde(default)]
    attachment_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct EmptyMutationBody {}

/// Least-privilege provider selection view. In particular, neither the endpoint nor the opaque
/// keychain reference crosses the companion boundary.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CompanionProvider {
    id: String,
    name: String,
    provider_type: String,
    default_model_id: Option<String>,
    default_temperature: Option<f64>,
    default_max_tokens: Option<i64>,
    is_local: bool,
    destination_class: String,
    capabilities: crate::providers::ProviderCapabilities,
    is_enabled: bool,
    credential_configured: bool,
}

impl From<crate::providers::ProviderConfig> for CompanionProvider {
    fn from(provider: crate::providers::ProviderConfig) -> Self {
        Self {
            id: provider.id,
            name: provider.name,
            provider_type: provider.provider_type,
            default_model_id: provider.default_model_id,
            default_temperature: provider.default_temperature,
            default_max_tokens: provider.default_max_tokens,
            is_local: provider.is_local,
            destination_class: provider.destination_class,
            capabilities: provider.capabilities,
            is_enabled: provider.is_enabled,
            credential_configured: provider.api_key_ref.is_some(),
        }
    }
}

/// Cached model inventory view. Raw provider metadata is deliberately omitted because callers
/// need selection/capability facts, not an adapter-specific payload echo.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CompanionModel {
    id: String,
    provider_id: String,
    name: String,
    display_name: Option<String>,
    context_window: Option<i64>,
    supports_streaming: bool,
    supports_tools: bool,
    tool_calling_mode: crate::providers::ToolCallingMode,
    supports_vision: bool,
    supports_embeddings: bool,
    is_available: bool,
    last_seen_at: Option<String>,
}

impl From<crate::providers::ModelInfo> for CompanionModel {
    fn from(model: crate::providers::ModelInfo) -> Self {
        Self {
            id: model.id,
            provider_id: model.provider_id,
            name: model.name,
            display_name: model.display_name,
            context_window: model.context_window,
            supports_streaming: model.supports_streaming,
            supports_tools: model.supports_tools,
            tool_calling_mode: model.tool_calling_mode,
            supports_vision: model.supports_vision,
            supports_embeddings: model.supports_embeddings,
            is_available: model.is_available,
            last_seen_at: model.last_seen_at,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BodyReadError {
    TooLarge,
    Invalid,
}

/// Held in `AppState` while the server is running; dropping/aborting `join_handle` stops it.
pub struct RunningCompanionApi {
    pub port: u16,
    join_handle: JoinHandle<()>,
}

impl RunningCompanionApi {
    pub fn stop(self) {
        self.join_handle.abort();
    }
}

struct ApiContext {
    /// Production always supplies the Tauri application handle. `None` exists only so the exact
    /// listener/router can be exercised over a real socket for routes that do not need AppState.
    app_handle: Option<AppHandle>,
    token: String,
    rate_limiter: Mutex<VecDeque<Instant>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompanionApiStatus {
    pub enabled: bool,
    pub running: bool,
    pub port: Option<u16>,
    pub token_configured: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompanionApiTokenReveal {
    /// Shown to the caller exactly once, at generation/regeneration time — mirroring the
    /// existing workspace-encryption recovery-key convention (`WorkspaceProtectionChange`).
    /// Never retrievable again after this response; `CompanionApiStatus.tokenConfigured`
    /// afterward only ever reports presence, not the value.
    pub token: String,
    pub status: CompanionApiStatus,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PersistedCompanionApiSettings {
    #[serde(default)]
    enabled: bool,
    #[serde(default)]
    token_ref: Option<String>,
}

fn settings_path(app: &AppHandle) -> Result<PathBuf, AppError> {
    app.path()
        .app_config_dir()
        .map(|dir| dir.join("companion_api_settings.json"))
        .map_err(|error| {
            AppError::new(
                "companion_api_settings_path_unavailable",
                format!("Could not resolve the companion API settings directory: {error}"),
            )
        })
}

fn load_settings(app: &AppHandle) -> PersistedCompanionApiSettings {
    let Ok(path) = settings_path(app) else {
        return PersistedCompanionApiSettings::default();
    };
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default()
}

fn save_settings(
    app: &AppHandle,
    settings: &PersistedCompanionApiSettings,
) -> Result<(), AppError> {
    let path = settings_path(app)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| {
            AppError::new(
                "companion_api_settings_write_failed",
                format!("Could not create {}: {error}", parent.display()),
            )
        })?;
    }
    let json = serde_json::to_string_pretty(settings).map_err(|error| {
        AppError::new(
            "companion_api_settings_write_failed",
            format!("Could not serialize companion API settings: {error}"),
        )
    })?;
    std::fs::write(&path, json).map_err(|error| {
        AppError::new(
            "companion_api_settings_write_failed",
            format!("Could not write {}: {error}", path.display()),
        )
    })?;
    Ok(())
}

/// SEC-010: a high-entropy, server-generated bearer token — two concatenated random UUIDv4s
/// (256 bits), the same "distinct random version-four UUID" convention `sidecar.rs` already
/// uses for the built-in runtime's own per-launch secret, doubled here since this token is
/// intended to be long-lived (regenerated only on explicit user action) rather than per-launch.
fn generate_token() -> String {
    format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    )
}

fn status_from(
    settings: &PersistedCompanionApiSettings,
    running: Option<&RunningCompanionApi>,
) -> CompanionApiStatus {
    CompanionApiStatus {
        enabled: settings.enabled,
        running: running.is_some(),
        port: running.map(|r| r.port),
        token_configured: settings.token_ref.is_some(),
    }
}

pub fn get_status(app: &AppHandle) -> Result<CompanionApiStatus, AppError> {
    let settings = load_settings(app);
    let state = app.state::<AppState>();
    let running = state
        .companion_api
        .lock()
        .map_err(|_| AppError::new("state_error", "Could not access companion API state."))?;
    Ok(status_from(&settings, running.as_ref()))
}

/// Restores an explicitly persisted opt-in after application state is ready. A fresh install
/// remains off because `PersistedCompanionApiSettings::default().enabled` is false.
pub async fn start_enabled_on_launch(app: &AppHandle) -> Result<(), AppError> {
    let settings = load_settings(app);
    if !settings.enabled {
        return Ok(());
    }
    let token_ref = settings.token_ref.as_deref().ok_or_else(|| {
        AppError::new(
            "companion_api_token_required",
            "The companion API is enabled but has no configured bearer token.",
        )
    })?;
    start_if_not_running(app, token_ref).await
}

/// Starts or stops the loopback server and persists the requested `enabled` flag. Idempotent:
/// enabling an already-running server or disabling an already-stopped one just returns the
/// current status.
pub async fn set_enabled(app: &AppHandle, enabled: bool) -> Result<CompanionApiStatus, AppError> {
    let mut settings = load_settings(app);

    if enabled {
        let token_ref = settings.token_ref.clone().ok_or_else(|| {
            AppError::new(
                "companion_api_token_required",
                "Generate and save a companion API token before enabling the server.",
            )
        })?;
        settings.enabled = true;
        save_settings(app, &settings)?;
        start_if_not_running(app, &token_ref).await?;
    } else {
        settings.enabled = false;
        save_settings(app, &settings)?;
        stop_if_running(app)?;
    }

    get_status(app)
}

/// Generates a fresh token, replacing any existing one immediately — the running server (if any)
/// is restarted so the previous token stops working on its very next request, matching SEC-010's
/// "revocation is immediate" bar.
pub async fn regenerate_token(app: &AppHandle) -> Result<CompanionApiTokenReveal, AppError> {
    let mut settings = load_settings(app);
    let token = generate_token();
    let reference = match &settings.token_ref {
        Some(existing) => {
            crate::secret_store::update_companion_api_token(existing, &token)?;
            existing.clone()
        }
        None => {
            let reference = crate::secret_store::new_companion_api_token_reference();
            crate::secret_store::store_companion_api_token(&reference, &token)?;
            reference
        }
    };
    settings.token_ref = Some(reference.clone());
    save_settings(app, &settings)?;

    let was_running = {
        let state = app.state::<AppState>();
        let running = state
            .companion_api
            .lock()
            .map_err(|_| AppError::new("state_error", "Could not access companion API state."))?;
        running.is_some()
    };
    if was_running {
        stop_if_running(app)?;
    }
    if settings.enabled {
        start_if_not_running(app, &reference).await?;
    }

    let status = get_status(app)?;
    Ok(CompanionApiTokenReveal { token, status })
}

async fn start_if_not_running(app: &AppHandle, token_ref: &str) -> Result<(), AppError> {
    let state = app.state::<AppState>();
    {
        let running = state
            .companion_api
            .lock()
            .map_err(|_| AppError::new("state_error", "Could not access companion API state."))?;
        if running.is_some() {
            return Ok(());
        }
    }
    let token = crate::secret_store::read_companion_api_token(token_ref)?;
    let (port, join_handle) = spawn_server(Some(app.clone()), token.to_string())
        .await
        .map_err(|error| {
            AppError::new(
                "companion_api_start_failed",
                format!("Could not start the companion API: {error}"),
            )
        })?;
    let mut running = state
        .companion_api
        .lock()
        .map_err(|_| AppError::new("state_error", "Could not access companion API state."))?;
    *running = Some(RunningCompanionApi { port, join_handle });
    Ok(())
}

fn stop_if_running(app: &AppHandle) -> Result<(), AppError> {
    let state = app.state::<AppState>();
    let mut running = state
        .companion_api
        .lock()
        .map_err(|_| AppError::new("state_error", "Could not access companion API state."))?;
    if let Some(handle) = running.take() {
        handle.stop();
    }
    Ok(())
}

async fn spawn_server(
    app_handle: Option<AppHandle>,
    token: String,
) -> std::io::Result<(u16, JoinHandle<()>)> {
    let listener = TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0))).await?;
    let port = listener.local_addr()?.port();

    let context = std::sync::Arc::new(ApiContext {
        app_handle,
        token,
        rate_limiter: Mutex::new(VecDeque::new()),
    });

    let handle = tokio::spawn(async move {
        loop {
            let (stream, _) = match listener.accept().await {
                Ok(accepted) => accepted,
                Err(_) => {
                    tokio::time::sleep(Duration::from_millis(50)).await;
                    continue;
                }
            };
            let io = TokioIo::new(stream);
            let context = std::sync::Arc::clone(&context);
            tokio::spawn(async move {
                let service = service_fn(move |request| {
                    handle_request(std::sync::Arc::clone(&context), request)
                });
                let _ = http1::Builder::new().serve_connection(io, service).await;
            });
        }
    });

    Ok((port, handle))
}

async fn handle_request(
    context: std::sync::Arc<ApiContext>,
    request: Request<Incoming>,
) -> Result<Response<ApiBody>, Infallible> {
    let request_id = request_id_for(&request);
    let method = request.method().clone();
    let path = request.uri().path().to_string();

    let (status, body) = route(&context, request).await;

    let level = if status.is_client_error() || status.is_server_error() {
        LogLevel::Warn
    } else {
        LogLevel::Info
    };
    // `try_state` rather than `state`: this must never panic the connection-handling task even
    // in the practically-unreachable case `AppState` isn't managed yet, since audit logging is a
    // secondary concern to actually answering the request.
    if let Some(app_handle) = context.app_handle.as_ref() {
        if let Some(state) = app_handle.try_state::<AppState>() {
            if let Ok(mut log) = state.observability_log.lock() {
                log.record(
                    level,
                    "companion_api",
                    Some(&request_id),
                    &format!("{method} {path} -> {}", status.as_u16()),
                );
            }
        }
    }

    let mut response = Response::builder()
        .status(status)
        .header("content-type", "application/json")
        .header("x-request-id", request_id)
        .header("x-ark-api-version", COMPANION_API_VERSION)
        .body(body)
        .unwrap_or_else(|_| {
            let mut fallback = Response::new(empty_body());
            *fallback.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
            fallback
        });
    // COR-009-style discipline applied here too: never let a browser tab read this response —
    // no Access-Control-* header is ever present, so both a simple cross-origin request and a
    // real preflight (which never carries the caller's Authorization header) fail closed.
    response.headers_mut().remove("access-control-allow-origin");
    Ok(response)
}

async fn route(context: &ApiContext, request: Request<Incoming>) -> (StatusCode, ApiBody) {
    if !is_authorized(&request, &context.token) {
        return error_response(
            StatusCode::UNAUTHORIZED,
            "unauthorized",
            "A valid bearer token is required.",
        );
    }
    if !requested_version_is_supported(&request) {
        return error_response(
            StatusCode::BAD_REQUEST,
            "unsupported_api_version",
            "This Ark build supports companion API version v1.",
        );
    }
    if !check_rate_limit(&context.rate_limiter) {
        return error_response(
            StatusCode::TOO_MANY_REQUESTS,
            "rate_limited",
            "Too many requests. Slow down and retry shortly.",
        );
    }

    let method = request.method().clone();
    let path = request.uri().path().to_string();
    let query = request.uri().query().unwrap_or("").to_string();
    let idempotency_header = request.headers().get("idempotency-key").cloned();

    // Read incrementally with a hard bound: collecting first and checking afterward would let an
    // untrusted local client allocate an arbitrarily large body before Ark rejected it.
    let body = match read_bounded_body(request.into_body()).await {
        Ok(body) => body,
        Err(BodyReadError::TooLarge) => {
            return error_response(
                StatusCode::PAYLOAD_TOO_LARGE,
                "payload_too_large",
                "Request body is too large.",
            );
        }
        Err(BodyReadError::Invalid) => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "invalid_request",
                "Could not read request body.",
            );
        }
    };

    if method == Method::GET && path == OPENAPI_PATH {
        return (
            StatusCode::OK,
            full_body(Bytes::from_static(OPENAPI_DOCUMENT.as_bytes())),
        );
    }

    if method == Method::GET && path == HEALTH_PATH {
        return ok_json(&serde_json::json!({
            "status": "ok",
            "version": COMPANION_API_VERSION,
        }));
    }

    let Some(state) = context
        .app_handle
        .as_ref()
        .and_then(|app_handle| app_handle.try_state::<AppState>())
    else {
        return error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "state_unavailable",
            "Ark is not ready.",
        );
    };

    match (method, path.as_str()) {
        (Method::GET, PROVIDERS_PATH) => match list_providers(&state) {
            Ok(providers) => ok_json(&providers),
            Err(error) => error_response(status_for(&error), &error.code, &error.message),
        },
        (Method::GET, path) if provider_models_resource_id(path).is_some() => {
            let raw_id =
                provider_models_resource_id(path).expect("route guard checked provider id");
            match list_models(&state, raw_id) {
                Ok(models) => ok_json(&models),
                Err(error) => error_response(status_for(&error), &error.code, &error.message),
            }
        }
        (Method::GET, CONVERSATIONS_PATH) => match list_conversations(&state, &query) {
            Ok(page) => ok_json(&page),
            Err(error) => error_response(status_for(&error), &error.code, &error.message),
        },
        (Method::POST, CONVERSATIONS_PATH) => {
            match create_conversation(&state, idempotency_header.as_ref(), &body) {
                Ok(conversation) => json_response(StatusCode::CREATED, &conversation),
                Err(error) => error_response(status_for(&error), &error.code, &error.message),
            }
        }
        (Method::GET, path) if conversation_messages_resource_id(path).is_some() => {
            let raw_id = conversation_messages_resource_id(path)
                .expect("route guard checked conversation id");
            match get_messages(&state, raw_id) {
                Ok(messages) => ok_json(&messages),
                Err(error) => error_response(status_for(&error), &error.code, &error.message),
            }
        }
        (Method::POST, path) if conversation_messages_resource_id(path).is_some() => {
            let raw_id = conversation_messages_resource_id(path)
                .expect("route guard checked conversation id");
            let app_handle = context
                .app_handle
                .as_ref()
                .expect("AppState is available only with a production app handle");
            match send_message(
                app_handle,
                &state,
                raw_id,
                idempotency_header.as_ref(),
                &body,
            ) {
                Ok(result) => json_response(StatusCode::CREATED, &result),
                Err(error) => error_response(status_for(&error), &error.code, &error.message),
            }
        }
        (Method::POST, path) if cancelled_message_resource_id(path).is_some() => {
            let raw_id =
                cancelled_message_resource_id(path).expect("route guard checked message id");
            let app_handle = context
                .app_handle
                .as_ref()
                .expect("AppState is available only with a production app handle");
            match cancel_message(
                app_handle,
                &state,
                raw_id,
                idempotency_header.as_ref(),
                &body,
            ) {
                Ok(message) => ok_json(&message),
                Err(error) => error_response(status_for(&error), &error.code, &error.message),
            }
        }
        (Method::PATCH, path) => {
            let Some(raw_id) = conversation_resource_id(path) else {
                return error_response(
                    StatusCode::NOT_FOUND,
                    "not_found",
                    "Unknown companion API route.",
                );
            };
            match update_conversation(&state, raw_id, idempotency_header.as_ref(), &body) {
                Ok(conversation) => ok_json(&conversation),
                Err(error) => error_response(status_for(&error), &error.code, &error.message),
            }
        }
        _ => error_response(
            StatusCode::NOT_FOUND,
            "not_found",
            "Unknown companion API route.",
        ),
    }
}

fn list_conversations(
    state: &AppState,
    query: &str,
) -> Result<crate::chat::ConversationPage, AppError> {
    let params = parse_query(query);
    let request = ConversationListRequest {
        limit: params
            .get("limit")
            .and_then(|value| value.parse::<u32>().ok())
            .or(Some(DEFAULT_PAGE_LIMIT)),
        cursor: params.get("cursor").cloned(),
        query: params.get("query").cloned(),
        archived: params.get("archived").map(|value| value == "true"),
        project_id: params.get("projectId").cloned(),
    };
    crate::commands::lock_read_db(state)?.list_conversations_page(&request)
}

fn get_messages(state: &AppState, raw_id: &str) -> Result<Vec<crate::chat::Message>, AppError> {
    let id = crate::validation::validate_entity_id(&percent_decode(raw_id), "Conversation ID")?
        .to_string();
    crate::commands::lock_read_db(state)?.get_active_messages(&id)
}

fn list_providers(state: &AppState) -> Result<Vec<CompanionProvider>, AppError> {
    crate::commands::lock_read_db(state)?
        .list_providers()
        .map(|providers| providers.into_iter().map(CompanionProvider::from).collect())
}

fn list_models(state: &AppState, raw_id: &str) -> Result<Vec<CompanionModel>, AppError> {
    let id =
        crate::validation::validate_entity_id(&percent_decode(raw_id), "Provider ID")?.to_string();
    let db = crate::commands::lock_read_db(state)?;
    db.get_provider(&id)?;
    db.list_models(&id)
        .map(|models| models.into_iter().map(CompanionModel::from).collect())
}

fn send_message(
    app: &AppHandle,
    state: &AppState,
    raw_id: &str,
    idempotency_header: Option<&hyper::header::HeaderValue>,
    body: &[u8],
) -> Result<crate::chat::SendChatResult, AppError> {
    let request: SendMessageBody = parse_json_body(body)?;
    let conversation_id =
        crate::validation::validate_entity_id(&percent_decode(raw_id), "Conversation ID")?
            .to_string();
    let idempotency_key = require_idempotency_key(idempotency_header)?;
    let request_hash = request_hash(body);
    let path = format!("{CONVERSATION_PATH_PREFIX}{conversation_id}{MESSAGES_PATH_SUFFIX}");
    let outcome = crate::generation::send_chat_message_idempotent(
        state,
        crate::chat::SendChatRequest {
            conversation_id,
            content: request.content,
            provider_id: request.provider_id,
            model: request.model,
            temperature: request.temperature,
            max_tokens: request.max_tokens,
            attachment_ids: request.attachment_ids,
            // Web search has its own explicit preview/approval boundary and is not inferred from
            // arbitrary integration input. A future endpoint must preserve that workflow.
            web_search: None,
        },
        crate::db::CompanionApiIdempotencyRequest {
            idempotency_key: &idempotency_key,
            method: "POST",
            path: &path,
            request_hash: &request_hash,
            response_status: StatusCode::CREATED.as_u16(),
        },
    )?;
    if !outcome.replayed {
        let start_result = crate::generation::start_pending_stream(
            app.clone(),
            state,
            outcome.value.assistant_message_id.clone(),
        );
        if let Err(error) = start_result {
            let message = crate::commands::lock_db(state)?
                .get_message(&outcome.value.assistant_message_id)?;
            if matches!(message.status.as_str(), "pending" | "streaming") {
                return Err(error);
            }
            // Queue/start validation can fail after the durable turn commits (for example, a
            // configured keychain reference was removed). The authoritative generation path
            // has already finalized that assistant message as `failed`; return the stored IDs
            // consistently and let this polling transport observe the typed terminal state.
        }
    }
    Ok(outcome.value)
}

fn cancel_message(
    app: &AppHandle,
    state: &AppState,
    raw_id: &str,
    idempotency_header: Option<&hyper::header::HeaderValue>,
    body: &[u8],
) -> Result<crate::chat::Message, AppError> {
    // Requiring an empty JSON object keeps the request fingerprint explicit and rejects hidden
    // or accidental cancellation parameters under `deny_unknown_fields` semantics.
    let _: EmptyMutationBody = parse_json_body(body)?;
    let message_id =
        crate::validation::validate_entity_id(&percent_decode(raw_id), "Message ID")?.to_string();
    let idempotency_key = require_idempotency_key(idempotency_header)?;
    let request_hash = request_hash(body);
    let path = format!("{MESSAGE_PATH_PREFIX}{message_id}{CANCEL_PATH_SUFFIX}");
    crate::generation::cancel_stream_idempotent(
        app.clone(),
        state,
        message_id,
        crate::db::CompanionApiIdempotencyRequest {
            idempotency_key: &idempotency_key,
            method: "POST",
            path: &path,
            request_hash: &request_hash,
            response_status: StatusCode::OK.as_u16(),
        },
    )
    .map(|outcome| outcome.value)
}

fn create_conversation(
    state: &AppState,
    idempotency_header: Option<&hyper::header::HeaderValue>,
    body: &[u8],
) -> Result<crate::chat::Conversation, AppError> {
    let request: CreateConversationBody = parse_json_body(body)?;
    let idempotency_key = require_idempotency_key(idempotency_header)?;
    let request_hash = request_hash(body);
    let db = crate::commands::lock_db(state)?;
    execute_idempotent(
        &db,
        &idempotency_key,
        "POST",
        CONVERSATIONS_PATH,
        &request_hash,
        StatusCode::CREATED,
        |db| db.create_conversation(request.title, None),
    )
}

fn update_conversation(
    state: &AppState,
    raw_id: &str,
    idempotency_header: Option<&hyper::header::HeaderValue>,
    body: &[u8],
) -> Result<crate::chat::Conversation, AppError> {
    let request: UpdateConversationBody = parse_json_body(body)?;
    if request.title.is_none() && request.archived.is_none() {
        return Err(AppError::invalid_input(
            "Conversation update must include title or archived.",
        ));
    }
    let id = crate::validation::validate_entity_id(&percent_decode(raw_id), "Conversation ID")?
        .to_string();
    let idempotency_key = require_idempotency_key(idempotency_header)?;
    let request_hash = request_hash(body);
    let path = format!("{CONVERSATION_PATH_PREFIX}{id}");
    let db = crate::commands::lock_db(state)?;
    execute_idempotent(
        &db,
        &idempotency_key,
        "PATCH",
        &path,
        &request_hash,
        StatusCode::OK,
        |db| {
            let mut conversation = db.get_conversation(&id)?;
            if let Some(title) = request.title {
                conversation = db.rename_conversation(&id, &title)?;
            }
            if let Some(archived) = request.archived {
                conversation = db.set_conversation_archived(&id, archived)?;
            }
            Ok(conversation)
        },
    )
}

fn conversation_resource_id(path: &str) -> Option<&str> {
    let id = path.strip_prefix(CONVERSATION_PATH_PREFIX)?;
    (!id.is_empty() && !id.contains('/')).then_some(id)
}

fn nested_resource_id<'a>(path: &'a str, prefix: &str, suffix: &str) -> Option<&'a str> {
    let id = path.strip_prefix(prefix)?.strip_suffix(suffix)?;
    (!id.is_empty() && !id.contains('/')).then_some(id)
}

fn conversation_messages_resource_id(path: &str) -> Option<&str> {
    nested_resource_id(path, CONVERSATION_PATH_PREFIX, MESSAGES_PATH_SUFFIX)
}

fn provider_models_resource_id(path: &str) -> Option<&str> {
    nested_resource_id(path, PROVIDER_PATH_PREFIX, "/models")
}

fn cancelled_message_resource_id(path: &str) -> Option<&str> {
    nested_resource_id(path, MESSAGE_PATH_PREFIX, CANCEL_PATH_SUFFIX)
}

fn require_idempotency_key(
    header: Option<&hyper::header::HeaderValue>,
) -> Result<String, AppError> {
    let value = header
        .and_then(|value| value.to_str().ok())
        .filter(|value| {
            !value.is_empty()
                && value.len() <= 128
                && value.bytes().all(|byte| {
                    byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':')
                })
        })
        .ok_or_else(|| {
            AppError::new(
                "idempotency_key_required",
                "Mutating companion API requests require a valid Idempotency-Key header.",
            )
        })?;
    Ok(value.to_string())
}

fn request_hash(body: &[u8]) -> String {
    format!("{:x}", Sha256::digest(body))
}

fn parse_json_body<T: DeserializeOwned>(body: &[u8]) -> Result<T, AppError> {
    serde_json::from_slice(body).map_err(|_| {
        AppError::invalid_input("Request body must be valid JSON with only documented fields.")
    })
}

fn execute_idempotent<T, F>(
    db: &crate::db::Database,
    idempotency_key: &str,
    method: &str,
    path: &str,
    request_hash: &str,
    response_status: StatusCode,
    operation: F,
) -> Result<T, AppError>
where
    T: Serialize + DeserializeOwned,
    F: FnOnce(&crate::db::Database) -> Result<T, AppError>,
{
    db.execute_companion_api_idempotent(
        &crate::db::CompanionApiIdempotencyRequest {
            idempotency_key,
            method,
            path,
            request_hash,
            response_status: response_status.as_u16(),
        },
        operation,
    )
    .map(|result| result.value)
}

fn status_for(error: &AppError) -> StatusCode {
    match error.code.as_str() {
        "invalid_input" | "idempotency_key_required" => StatusCode::BAD_REQUEST,
        "idempotency_conflict" => StatusCode::CONFLICT,
        "not_found" => StatusCode::NOT_FOUND,
        "workspace_maintenance_busy" => StatusCode::SERVICE_UNAVAILABLE,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

fn request_id_for<B>(request: &Request<B>) -> String {
    request
        .headers()
        .get("x-request-id")
        .and_then(|value| value.to_str().ok())
        .filter(|value| {
            !value.is_empty()
                && value.len() <= 128
                && value.bytes().all(|byte| {
                    byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':')
                })
        })
        .map(str::to_string)
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string())
}

fn requested_version_is_supported<B>(request: &Request<B>) -> bool {
    request
        .headers()
        .get("x-ark-api-version")
        .is_none_or(|value| value.as_bytes() == COMPANION_API_VERSION.as_bytes())
}

async fn read_bounded_body<B>(mut body: B) -> Result<Bytes, BodyReadError>
where
    B: hyper::body::Body<Data = Bytes> + Unpin,
{
    let mut received = Vec::new();
    while let Some(frame) = BodyExt::frame(&mut body).await {
        let frame = frame.map_err(|_| BodyReadError::Invalid)?;
        if let Some(data) = frame.data_ref() {
            let next_len = received
                .len()
                .checked_add(data.len())
                .ok_or(BodyReadError::TooLarge)?;
            if next_len > MAX_REQUEST_BODY_BYTES {
                return Err(BodyReadError::TooLarge);
            }
            received.extend_from_slice(data);
        }
    }
    Ok(Bytes::from(received))
}

/// Generic over the body type (rather than hardcoded to `Incoming`) purely so this pure,
/// security-critical predicate can be unit-tested directly against a `Request<Full<Bytes>>`
/// built in-process, without needing a real network connection.
fn is_authorized<B>(request: &Request<B>, token: &str) -> bool {
    let Some(header_value) = request.headers().get(hyper::header::AUTHORIZATION) else {
        return false;
    };
    let Ok(header_str) = header_value.to_str() else {
        return false;
    };
    header_str
        .strip_prefix("Bearer ")
        .is_some_and(|value| value == token)
}

/// A simple sliding-window request log, sufficient for a single-token loopback server (there is
/// currently at most one legitimate caller). Pruned to the current window on every check so the
/// queue never grows unbounded even under sustained traffic.
fn check_rate_limit(limiter: &Mutex<VecDeque<Instant>>) -> bool {
    let Ok(mut recent) = limiter.lock() else {
        return false;
    };
    let now = Instant::now();
    while let Some(oldest) = recent.front() {
        if now.duration_since(*oldest) > RATE_LIMIT_WINDOW {
            recent.pop_front();
        } else {
            break;
        }
    }
    if recent.len() >= RATE_LIMIT_MAX_REQUESTS {
        return false;
    }
    recent.push_back(now);
    true
}

fn parse_query(query: &str) -> std::collections::HashMap<String, String> {
    query
        .split('&')
        .filter(|pair| !pair.is_empty())
        .filter_map(|pair| {
            let mut parts = pair.splitn(2, '=');
            let key = parts.next()?;
            let value = parts.next().unwrap_or("");
            Some((percent_decode(key), percent_decode(value)))
        })
        .collect()
}

fn percent_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'+' => {
                output.push(b' ');
                index += 1;
            }
            b'%' if index + 2 < bytes.len() => {
                let hex = std::str::from_utf8(&bytes[index + 1..index + 3]).ok();
                if let Some(decoded) = hex.and_then(|h| u8::from_str_radix(h, 16).ok()) {
                    output.push(decoded);
                    index += 3;
                } else {
                    output.push(bytes[index]);
                    index += 1;
                }
            }
            other => {
                output.push(other);
                index += 1;
            }
        }
    }
    String::from_utf8_lossy(&output).into_owned()
}

fn ok_json<T: Serialize>(value: &T) -> (StatusCode, ApiBody) {
    json_response(StatusCode::OK, value)
}

fn json_response<T: Serialize>(status: StatusCode, value: &T) -> (StatusCode, ApiBody) {
    match serde_json::to_vec(value) {
        Ok(bytes) => (status, full_body(Bytes::from(bytes))),
        Err(_) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "serialization_failed",
            "Could not serialize response.",
        ),
    }
}

fn error_response(status: StatusCode, code: &str, message: &str) -> (StatusCode, ApiBody) {
    let envelope = serde_json::json!({ "error": { "code": code, "message": message } });
    let bytes = serde_json::to_vec(&envelope).unwrap_or_else(|_| b"{}".to_vec());
    (status, full_body(Bytes::from(bytes)))
}

fn full_body(bytes: Bytes) -> ApiBody {
    BodyExt::boxed(Full::new(bytes).map_err(|never: Infallible| match never {}))
}

fn empty_body() -> ApiBody {
    BodyExt::boxed(Empty::<Bytes>::new().map_err(|never: Infallible| match never {}))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{BTreeSet, HashMap};
    use std::fs;

    fn openapi_document() -> serde_json::Value {
        serde_json::from_str(OPENAPI_DOCUMENT)
            .expect("published OpenAPI document must be valid JSON")
    }

    fn object_keys(value: &serde_json::Value) -> BTreeSet<String> {
        value
            .as_object()
            .expect("schema sample must serialize as an object")
            .keys()
            .cloned()
            .collect()
    }

    fn required_schema_fields(document: &serde_json::Value, schema: &str) -> BTreeSet<String> {
        document["components"]["schemas"][schema]["required"]
            .as_array()
            .expect("published object schema must declare required fields")
            .iter()
            .map(|field| {
                field
                    .as_str()
                    .expect("required field must be a string")
                    .to_string()
            })
            .collect()
    }

    fn sample_conversation() -> crate::chat::Conversation {
        crate::chat::Conversation {
            id: "conversation-1".to_string(),
            title: "Example".to_string(),
            created_at: "2026-08-17T00:00:00Z".to_string(),
            updated_at: "2026-08-17T00:00:00Z".to_string(),
            provider_id: Some("ollama".to_string()),
            model_id: Some("model-1".to_string()),
            current_message_id: None,
            system_prompt: None,
            temperature: None,
            max_tokens: None,
            archived: false,
            project_id: None,
            pinned_at: None,
            persona_id: None,
            response_style: None,
            tone: None,
        }
    }

    fn sample_message() -> crate::chat::Message {
        crate::chat::Message {
            id: "message-1".to_string(),
            conversation_id: "conversation-1".to_string(),
            parent_message_id: None,
            revision_of_message_id: None,
            path_index: 0,
            role: "user".to_string(),
            content: "Hello".to_string(),
            status: "complete".to_string(),
            created_at: "2026-08-17T00:00:00Z".to_string(),
            updated_at: "2026-08-17T00:00:00Z".to_string(),
            provider_id: Some("ollama".to_string()),
            model_id: Some("model-1".to_string()),
            token_count: None,
            error_message: None,
            metadata_json: None,
            branch_name: None,
        }
    }

    fn sample_provider() -> CompanionProvider {
        CompanionProvider::from(crate::providers::ProviderConfig {
            id: "provider-1".to_string(),
            name: "Private provider".to_string(),
            provider_type: crate::config::DEFAULT_PROVIDER_TYPE.to_string(),
            base_url: Some("http://127.0.0.1:11434".to_string()),
            api_key_ref: Some("ark/provider/secret-reference".to_string()),
            default_model_id: Some("model-1".to_string()),
            default_temperature: Some(0.7),
            default_max_tokens: Some(2048),
            is_local: true,
            allow_insecure_remote: false,
            destination_class: "loopback".to_string(),
            capabilities: crate::providers::ProviderCapabilities::for_provider_type(
                crate::config::DEFAULT_PROVIDER_TYPE,
            ),
            is_user_managed: false,
            is_enabled: true,
            created_at: "2026-08-17T00:00:00Z".to_string(),
            updated_at: "2026-08-17T00:00:00Z".to_string(),
        })
    }

    fn sample_model() -> CompanionModel {
        CompanionModel::from(crate::providers::ModelInfo {
            id: "provider-1:model-1".to_string(),
            provider_id: "provider-1".to_string(),
            name: "model-1".to_string(),
            display_name: Some("Model One".to_string()),
            context_window: Some(8192),
            supports_streaming: true,
            supports_tools: false,
            tool_calling_mode: crate::providers::ToolCallingMode::Unsupported,
            supports_vision: false,
            supports_embeddings: false,
            is_available: true,
            last_seen_at: Some("2026-08-17T00:00:00Z".to_string()),
            metadata_json: Some("{\"rawProviderField\":true}".to_string()),
            created_at: "2026-08-17T00:00:00Z".to_string(),
            updated_at: "2026-08-17T00:00:00Z".to_string(),
        })
    }

    #[test]
    fn published_openapi_document_matches_every_authenticated_route() {
        let document = openapi_document();
        assert_eq!(document["openapi"], "3.1.0");
        assert_eq!(document["info"]["version"], "1.0.0");
        assert_eq!(document["security"][0]["bearerAuth"], serde_json::json!([]));

        let documented_paths: BTreeSet<_> = document["paths"]
            .as_object()
            .expect("OpenAPI paths must be an object")
            .keys()
            .map(String::as_str)
            .collect();
        let implemented_paths = BTreeSet::from([
            OPENAPI_PATH,
            HEALTH_PATH,
            CONVERSATIONS_PATH,
            PROVIDERS_PATH,
            "/v1/conversations/{conversationId}",
            "/v1/conversations/{conversationId}/messages",
            "/v1/providers/{providerId}/models",
            "/v1/messages/{messageId}/cancel",
        ]);
        assert_eq!(documented_paths, implemented_paths);

        let implemented_operations = [
            (OPENAPI_PATH, "get", "200", false),
            (HEALTH_PATH, "get", "200", false),
            (PROVIDERS_PATH, "get", "200", false),
            ("/v1/providers/{providerId}/models", "get", "200", false),
            (CONVERSATIONS_PATH, "get", "200", false),
            (CONVERSATIONS_PATH, "post", "201", true),
            ("/v1/conversations/{conversationId}", "patch", "200", true),
            (
                "/v1/conversations/{conversationId}/messages",
                "get",
                "200",
                false,
            ),
            (
                "/v1/conversations/{conversationId}/messages",
                "post",
                "201",
                true,
            ),
            ("/v1/messages/{messageId}/cancel", "post", "200", true),
        ];
        for (path, method, success_status, is_mutation) in implemented_operations {
            let operation = &document["paths"][path][method];
            assert!(
                operation.is_object(),
                "{path} must document {}",
                method.to_uppercase()
            );
            assert!(
                operation["parameters"]
                    .as_array()
                    .expect("operation parameters must be an array")
                    .iter()
                    .any(|parameter| {
                        parameter["$ref"] == "#/components/parameters/RequestedApiVersion"
                    }),
                "{path} must document version negotiation"
            );
            assert!(
                operation["responses"]["401"].is_object(),
                "{path} must document authentication failure"
            );
            assert!(
                operation["responses"]["429"].is_object(),
                "{path} must document rate limiting"
            );
            if is_mutation {
                assert!(
                    operation["parameters"]
                        .as_array()
                        .expect("mutation parameters must be an array")
                        .iter()
                        .any(|parameter| {
                            parameter["$ref"] == "#/components/parameters/IdempotencyKey"
                        }),
                    "{method} {path} must require an idempotency key"
                );
                assert!(
                    operation["responses"]["409"].is_object(),
                    "{method} {path} must document idempotency conflicts"
                );
            }
            assert_eq!(
                operation["responses"][success_status]["headers"]["X-Ark-Api-Version"]["$ref"],
                "#/components/headers/ApiVersion"
            );
            assert_eq!(
                operation["responses"][success_status]["headers"]["X-Request-Id"]["$ref"],
                "#/components/headers/RequestId"
            );
        }
    }

    #[test]
    fn published_openapi_schemas_match_the_serialized_rust_response_fields() {
        let document = openapi_document();
        let conversation =
            serde_json::to_value(sample_conversation()).expect("serialize conversation");
        assert_eq!(
            object_keys(&conversation),
            required_schema_fields(&document, "Conversation")
        );

        let message = serde_json::to_value(sample_message()).expect("serialize message");
        assert_eq!(
            object_keys(&message),
            required_schema_fields(&document, "Message")
        );

        let page = crate::chat::ConversationPage {
            items: vec![sample_conversation()],
            next_cursor: None,
            search_snippets: HashMap::new(),
        };
        let page = serde_json::to_value(page).expect("serialize conversation page");
        assert_eq!(
            object_keys(&page),
            required_schema_fields(&document, "ConversationPage")
        );

        let provider = serde_json::to_value(sample_provider()).expect("serialize provider");
        assert_eq!(
            object_keys(&provider),
            required_schema_fields(&document, "ProviderSummary")
        );
        assert!(provider.get("baseUrl").is_none());
        assert!(provider.get("apiKeyRef").is_none());
        assert_eq!(provider["credentialConfigured"], true);
        assert_eq!(
            object_keys(&provider["capabilities"]),
            required_schema_fields(&document, "ProviderCapabilities")
        );

        let model = serde_json::to_value(sample_model()).expect("serialize model");
        assert_eq!(
            object_keys(&model),
            required_schema_fields(&document, "ModelSummary")
        );
        assert!(model.get("metadataJson").is_none());

        let send_result = serde_json::to_value(crate::chat::SendChatResult {
            conversation_id: "conversation-1".to_string(),
            user_message_id: "message-user".to_string(),
            assistant_message_id: "message-assistant".to_string(),
        })
        .expect("serialize send result");
        assert_eq!(
            object_keys(&send_result),
            required_schema_fields(&document, "SendChatResult")
        );
    }

    #[test]
    fn cancellation_body_accepts_only_an_empty_json_object() {
        parse_json_body::<EmptyMutationBody>(b"{}").expect("empty object is valid");
        assert!(parse_json_body::<EmptyMutationBody>(b"{\"force\":true}").is_err());
        assert!(parse_json_body::<EmptyMutationBody>(b"").is_err());
    }

    #[test]
    fn percent_decode_handles_plus_and_hex_escapes() {
        assert_eq!(percent_decode("hello+world"), "hello world");
        assert_eq!(percent_decode("a%20b%3D1"), "a b=1");
        assert_eq!(percent_decode("plain"), "plain");
    }

    #[test]
    fn parse_query_splits_multiple_pairs_and_decodes_values() {
        let params = parse_query("limit=10&query=hello%20world&archived=true");
        assert_eq!(params.get("limit").map(String::as_str), Some("10"));
        assert_eq!(params.get("query").map(String::as_str), Some("hello world"));
        assert_eq!(params.get("archived").map(String::as_str), Some("true"));
    }

    #[test]
    fn parse_query_handles_empty_string() {
        assert!(parse_query("").is_empty());
    }

    #[test]
    fn rate_limiter_allows_up_to_the_limit_then_rejects() {
        let limiter = Mutex::new(VecDeque::new());
        for _ in 0..RATE_LIMIT_MAX_REQUESTS {
            assert!(check_rate_limit(&limiter));
        }
        assert!(!check_rate_limit(&limiter));
    }

    #[test]
    fn generated_tokens_are_high_entropy_and_distinct() {
        let a = generate_token();
        let b = generate_token();
        assert_ne!(a, b);
        assert_eq!(a.len(), 64);
    }

    // ── is_authorized: the core security boundary — SEC-010 requires every route (no
    // exemptions) to require this exact custom-header bearer check, never a cookie. ──

    #[test]
    fn is_authorized_accepts_the_exact_bearer_token_in_the_authorization_header() {
        let request = Request::builder()
            .header(hyper::header::AUTHORIZATION, "Bearer correct-token")
            .body(())
            .expect("build request");
        assert!(is_authorized(&request, "correct-token"));
    }

    #[test]
    fn is_authorized_rejects_a_missing_header() {
        let request = Request::builder().body(()).expect("build request");
        assert!(!is_authorized(&request, "correct-token"));
    }

    #[test]
    fn is_authorized_rejects_a_wrong_token() {
        let request = Request::builder()
            .header(hyper::header::AUTHORIZATION, "Bearer wrong-token")
            .body(())
            .expect("build request");
        assert!(!is_authorized(&request, "correct-token"));
    }

    #[test]
    fn is_authorized_rejects_a_token_not_sent_as_a_bearer_header() {
        // SEC-010: never a cookie, never a bare (unprefixed) value — only the exact
        // `Authorization: Bearer <token>` header form is accepted.
        let request = Request::builder()
            .header(hyper::header::COOKIE, "session=correct-token")
            .body(())
            .expect("build request");
        assert!(!is_authorized(&request, "correct-token"));

        let request = Request::builder()
            .header(hyper::header::AUTHORIZATION, "correct-token")
            .body(())
            .expect("build request");
        assert!(!is_authorized(&request, "correct-token"));
    }

    #[test]
    fn request_version_negotiation_accepts_v1_or_path_default_and_rejects_unknown_versions() {
        let defaulted = Request::builder().body(()).expect("build request");
        assert!(requested_version_is_supported(&defaulted));

        let v1 = Request::builder()
            .header("x-ark-api-version", "v1")
            .body(())
            .expect("build request");
        assert!(requested_version_is_supported(&v1));

        let future = Request::builder()
            .header("x-ark-api-version", "v2")
            .body(())
            .expect("build request");
        assert!(!requested_version_is_supported(&future));
    }

    #[test]
    fn request_ids_are_bounded_and_safe_before_reflection_or_logging() {
        let valid = Request::builder()
            .header("x-request-id", "integration:request-1")
            .body(())
            .expect("build request");
        assert_eq!(request_id_for(&valid), "integration:request-1");

        let unsafe_value = Request::builder()
            .header("x-request-id", "contains spaces")
            .body(())
            .expect("build request");
        let generated = request_id_for(&unsafe_value);
        assert_ne!(generated, "contains spaces");
        uuid::Uuid::parse_str(&generated).expect("replacement request id must be a UUID");

        let oversized = Request::builder()
            .header("x-request-id", "a".repeat(129))
            .body(())
            .expect("build request");
        assert_ne!(request_id_for(&oversized), "a".repeat(129));
    }

    #[test]
    fn mutation_idempotency_keys_and_resource_paths_are_strictly_bounded() {
        let valid = hyper::header::HeaderValue::from_static("client:request-1");
        assert_eq!(
            require_idempotency_key(Some(&valid)).expect("valid key"),
            "client:request-1"
        );
        assert!(require_idempotency_key(None).is_err());
        let unsafe_key = hyper::header::HeaderValue::from_static("contains spaces");
        assert!(require_idempotency_key(Some(&unsafe_key)).is_err());

        assert_eq!(
            conversation_resource_id("/v1/conversations/conversation-1"),
            Some("conversation-1")
        );
        assert_eq!(conversation_resource_id("/v1/conversations/"), None);
        assert_eq!(
            conversation_resource_id("/v1/conversations/conversation-1/messages"),
            None
        );
        assert_eq!(
            conversation_messages_resource_id("/v1/conversations/conversation-1/messages"),
            Some("conversation-1")
        );
        assert_eq!(
            conversation_messages_resource_id("/v1/conversations/conversation-1/branch/messages"),
            None
        );
        assert_eq!(
            provider_models_resource_id("/v1/providers/provider-1/models"),
            Some("provider-1")
        );
        assert_eq!(
            cancelled_message_resource_id("/v1/messages/message-1/cancel"),
            Some("message-1")
        );
    }

    #[test]
    fn idempotent_mutation_replays_the_original_result_and_rejects_key_reuse() {
        let path = std::env::temp_dir().join(format!(
            "ark-companion-idempotency-test-{}.sqlite3",
            uuid::Uuid::new_v4()
        ));
        let db = crate::db::Database::open(&path).expect("open test database");
        let hash = request_hash(br#"{"title":"First"}"#);
        let first = execute_idempotent(
            &db,
            "request-1",
            "POST",
            CONVERSATIONS_PATH,
            &hash,
            StatusCode::CREATED,
            |db| db.create_conversation(Some("First".to_string()), None),
        )
        .expect("first mutation succeeds");
        let replay = execute_idempotent(
            &db,
            "request-1",
            "POST",
            CONVERSATIONS_PATH,
            &hash,
            StatusCode::CREATED,
            |db| db.create_conversation(Some("Must not be created".to_string()), None),
        )
        .expect("matching retry replays");
        assert_eq!(replay.id, first.id);

        let page = db
            .list_conversations_page(&crate::chat::ConversationListRequest {
                limit: Some(100),
                cursor: None,
                query: None,
                archived: None,
                project_id: None,
            })
            .expect("list conversations");
        assert_eq!(page.items.len(), 1, "retry must not repeat the mutation");

        let conflict = execute_idempotent(
            &db,
            "request-1",
            "POST",
            CONVERSATIONS_PATH,
            &request_hash(br#"{"title":"Different"}"#),
            StatusCode::CREATED,
            |db| db.create_conversation(Some("Different".to_string()), None),
        )
        .expect_err("same key with a different body must fail");
        assert_eq!(conflict.code, "idempotency_conflict");

        drop(db);
        let _ = fs::remove_file(&path);
        let _ = fs::remove_file(path.with_extension("sqlite3-wal"));
        let _ = fs::remove_file(path.with_extension("sqlite3-shm"));
    }

    #[tokio::test]
    async fn request_bodies_are_rejected_as_soon_as_the_stream_exceeds_the_bound() {
        let exact = Full::new(Bytes::from(vec![0_u8; MAX_REQUEST_BODY_BYTES]));
        assert_eq!(
            read_bounded_body(exact)
                .await
                .expect("exact-limit body succeeds")
                .len(),
            MAX_REQUEST_BODY_BYTES
        );

        let oversized = Full::new(Bytes::from(vec![0_u8; MAX_REQUEST_BODY_BYTES + 1]));
        assert_eq!(
            read_bounded_body(oversized).await,
            Err(BodyReadError::TooLarge)
        );
    }

    #[test]
    fn poisoned_rate_limiter_fails_closed() {
        let limiter = Mutex::new(VecDeque::new());
        let _ = std::panic::catch_unwind(|| {
            let _guard = limiter.lock().expect("initial lock");
            panic!("poison for test");
        });
        assert!(!check_rate_limit(&limiter));
    }

    #[test]
    fn status_for_maps_known_error_codes_and_falls_back_to_internal_error() {
        assert_eq!(
            status_for(&AppError::invalid_input("bad")),
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            status_for(&AppError::not_found("thing")),
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            status_for(&AppError::new("workspace_maintenance_busy", "busy")),
            StatusCode::SERVICE_UNAVAILABLE
        );
        assert_eq!(
            status_for(&AppError::new("idempotency_key_required", "missing")),
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            status_for(&AppError::new("idempotency_conflict", "reused")),
            StatusCode::CONFLICT
        );
        assert_eq!(
            status_for(&AppError::new("database_error", "oops")),
            StatusCode::INTERNAL_SERVER_ERROR
        );
    }

    async fn body_to_string(body: ApiBody) -> String {
        let bytes = BodyExt::collect(body)
            .await
            .expect("collect body")
            .to_bytes();
        String::from_utf8(bytes.to_vec()).expect("utf8 body")
    }

    #[tokio::test]
    async fn ok_json_returns_200_with_the_serialized_value() {
        let (status, body) = ok_json(&serde_json::json!({ "hello": "world" }));
        assert_eq!(status, StatusCode::OK);
        let text = body_to_string(body).await;
        assert!(text.contains("\"hello\":\"world\""));
    }

    #[tokio::test]
    async fn error_response_uses_the_typed_envelope_shape() {
        let (status, body) = error_response(StatusCode::BAD_REQUEST, "invalid_input", "Bad input.");
        assert_eq!(status, StatusCode::BAD_REQUEST);
        let text = body_to_string(body).await;
        let parsed: serde_json::Value = serde_json::from_str(&text).expect("valid JSON");
        assert_eq!(parsed["error"]["code"], "invalid_input");
        assert_eq!(parsed["error"]["message"], "Bad input.");
    }

    #[tokio::test]
    async fn real_loopback_server_conforms_for_auth_version_limits_and_openapi() {
        let (port, server) = spawn_server(None, "socket-test-token".to_string())
            .await
            .expect("bind production companion API server");
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .expect("build client");
        let base_url = format!("http://127.0.0.1:{port}");

        let unauthorized = client
            .get(format!("{base_url}{HEALTH_PATH}"))
            .send()
            .await
            .expect("unauthorized request completes");
        assert_eq!(unauthorized.status(), reqwest::StatusCode::UNAUTHORIZED);
        assert!(
            unauthorized
                .headers()
                .get("access-control-allow-origin")
                .is_none(),
            "production server must never emit CORS permission"
        );

        let health = client
            .get(format!("{base_url}{HEALTH_PATH}"))
            .bearer_auth("socket-test-token")
            .header("x-request-id", "socket:health-1")
            .header("x-ark-api-version", COMPANION_API_VERSION)
            .send()
            .await
            .expect("authorized health request completes");
        assert_eq!(health.status(), reqwest::StatusCode::OK);
        assert_eq!(health.headers()["x-request-id"], "socket:health-1");
        assert_eq!(health.headers()["x-ark-api-version"], COMPANION_API_VERSION);
        assert_eq!(
            health
                .json::<serde_json::Value>()
                .await
                .expect("health JSON"),
            serde_json::json!({"status": "ok", "version": "v1"})
        );

        let unsupported = client
            .get(format!("{base_url}{HEALTH_PATH}"))
            .bearer_auth("socket-test-token")
            .header("x-ark-api-version", "v2")
            .send()
            .await
            .expect("unsupported version request completes");
        assert_eq!(unsupported.status(), reqwest::StatusCode::BAD_REQUEST);
        assert_eq!(
            unsupported
                .json::<serde_json::Value>()
                .await
                .expect("version error JSON")["error"]["code"],
            "unsupported_api_version"
        );

        let openapi = client
            .get(format!("{base_url}{OPENAPI_PATH}"))
            .bearer_auth("socket-test-token")
            .send()
            .await
            .expect("OpenAPI request completes");
        assert_eq!(openapi.status(), reqwest::StatusCode::OK);
        assert_eq!(
            openapi
                .json::<serde_json::Value>()
                .await
                .expect("OpenAPI JSON"),
            openapi_document()
        );

        let oversized = client
            .get(format!("{base_url}{HEALTH_PATH}"))
            .bearer_auth("socket-test-token")
            .body(vec![0_u8; MAX_REQUEST_BODY_BYTES + 1])
            .send()
            .await
            .expect("oversized request completes");
        assert_eq!(oversized.status(), reqwest::StatusCode::PAYLOAD_TOO_LARGE);

        server.abort();
    }
}
