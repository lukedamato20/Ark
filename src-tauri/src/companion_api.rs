//! FTR-010: a disabled-by-default local HTTP API for external integrations and the future
//! mobile companion (MOB-009). Reuses SEC-002's proven proxy pattern (`proxy.rs`): a
//! loopback-only listener, custom-header bearer auth with zero exempt routes (not even
//! `/v1/health`), and no response this server sends ever carries an `Access-Control-*` header —
//! so a cross-origin browser page can never read a response even if it somehow knew the token,
//! and a real CORS preflight (which never carries the caller's intended `Authorization` header)
//! always fails the auth check first regardless.
//!
//! Every route is served by the exact same `Database`/application-service functions the Tauri
//! command surface uses (`commands::lock_read_db`, `Database::list_conversations_page`,
//! `Database::get_active_messages`) — there is no second, parallel data-access path and no raw
//! SQL or filesystem access reachable from the wire.
//!
//! **Scope of this pass, matching SEC-010's threat model and stated honestly rather than
//! silently narrowed:** SEC-010 calls for loopback and paired-LAN modes to have *distinct*
//! controls; paired-LAN mode depends on MOB-009's per-device pairing lifecycle, which does not
//! exist yet, so this implements the loopback control only and binds `127.0.0.1` exclusively —
//! there is no LAN-reachable mode to accidentally enable. The only two operations exposed are
//! read-only (list conversations, read one conversation's active-path messages) — enough to
//! prove the versioned/authenticated/rate-limited/typed-error/audited shape end-to-end without
//! taking on a write surface's extra risk in the same pass that first turns this server on.

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
use serde::{Deserialize, Serialize};
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
const MAX_REQUEST_BODY_BYTES: usize = 64 * 1024;
const RATE_LIMIT_MAX_REQUESTS: usize = 120;
const RATE_LIMIT_WINDOW: Duration = Duration::from_secs(60);
const DEFAULT_PAGE_LIMIT: u32 = 50;

type ApiBody = BoxBody<Bytes, Infallible>;

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
    app_handle: AppHandle,
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

/// Starts or stops the loopback server and persists the requested `enabled` flag. Idempotent:
/// enabling an already-running server or disabling an already-stopped one just returns the
/// current status.
pub async fn set_enabled(app: &AppHandle, enabled: bool) -> Result<CompanionApiStatus, AppError> {
    let mut settings = load_settings(app);

    if enabled {
        let token_ref = match &settings.token_ref {
            Some(reference) => reference.clone(),
            None => {
                let reference = crate::secret_store::new_companion_api_token_reference();
                crate::secret_store::store_companion_api_token(&reference, &generate_token())?;
                reference
            }
        };
        settings.token_ref = Some(token_ref.clone());
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
    let (port, join_handle) =
        spawn_server(app.clone(), token.to_string())
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
    app_handle: AppHandle,
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
    let request_id = request
        .headers()
        .get("x-request-id")
        .and_then(|value| value.to_str().ok())
        .map(str::to_string)
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
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
    if let Some(state) = context.app_handle.try_state::<AppState>() {
        if let Ok(mut log) = state.observability_log.lock() {
            log.record(
                level,
                "companion_api",
                Some(&request_id),
                &format!("{method} {path} -> {}", status.as_u16()),
            );
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

    // No route in this pass reads a request body, but a client could still send one — bound and
    // discard it rather than leaving it unread (which would otherwise require closing the
    // connection after every request to stay correct with HTTP keep-alive).
    match BodyExt::collect(request.into_body()).await {
        Ok(collected) => {
            if collected.to_bytes().len() > MAX_REQUEST_BODY_BYTES {
                return error_response(
                    StatusCode::PAYLOAD_TOO_LARGE,
                    "payload_too_large",
                    "Request body is too large.",
                );
            }
        }
        Err(_) => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "invalid_request",
                "Could not read request body.",
            );
        }
    }

    if method == Method::GET && path == "/v1/health" {
        return ok_json(&serde_json::json!({
            "status": "ok",
            "version": COMPANION_API_VERSION,
        }));
    }

    let Some(state) = context.app_handle.try_state::<AppState>() else {
        return error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "state_unavailable",
            "Ark is not ready.",
        );
    };

    match (method, path.as_str()) {
        (Method::GET, "/v1/conversations") => match list_conversations(&state, &query) {
            Ok(page) => ok_json(&page),
            Err(error) => error_response(status_for(&error), &error.code, &error.message),
        },
        (Method::GET, path)
            if path.starts_with("/v1/conversations/") && path.ends_with("/messages") =>
        {
            let id = &path["/v1/conversations/".len()..path.len() - "/messages".len()];
            match get_messages(&state, id) {
                Ok(messages) => ok_json(&messages),
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

fn status_for(error: &AppError) -> StatusCode {
    match error.code.as_str() {
        "invalid_input" => StatusCode::BAD_REQUEST,
        "not_found" => StatusCode::NOT_FOUND,
        "workspace_maintenance_busy" => StatusCode::SERVICE_UNAVAILABLE,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    }
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
        return true;
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
    match serde_json::to_vec(value) {
        Ok(bytes) => (StatusCode::OK, full_body(Bytes::from(bytes))),
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
}
