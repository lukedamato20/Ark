use crate::chat::ChatMessage;
use crate::config::{
    BUILT_IN_PROVIDER_TYPE, DEFAULT_PROVIDER_TYPE, LOCAL_INFERENCE_HOST_PROVIDER_TYPE,
};
use crate::errors::AppError;
use async_trait::async_trait;
use futures_util::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[cfg(test)]
pub(crate) mod test_support;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderConfig {
    pub id: String,
    pub name: String,
    pub provider_type: String,
    pub base_url: Option<String>,
    pub api_key_ref: Option<String>,
    pub default_model_id: Option<String>,
    pub default_temperature: Option<f64>,
    pub default_max_tokens: Option<i64>,
    pub is_local: bool,
    /// Persisted, explicitly warned development-mode exception for HTTP outside loopback.
    pub allow_insecure_remote: bool,
    /// SEC-001: `"loopback" | "private_lan" | "public"`, derived from
    /// [`crate::security::classify_destination`] at read time — never trusted from storage.
    /// The frontend renders its endpoint-class badge/tooltip from this field rather than
    /// re-parsing the URL itself, so classification logic lives in exactly one place.
    pub destination_class: String,
    /// ARC-003: computed from `provider_type` at read time (see `db::map_provider`), the same
    /// way `destination_class` above is computed rather than stored — the frontend drives UI
    /// affordances (e.g. hiding a "pull model" button) from this instead of hardcoding
    /// per-provider-type assumptions.
    pub capabilities: ProviderCapabilities,
    pub is_enabled: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelInfo {
    pub id: String,
    pub provider_id: String,
    pub name: String,
    pub display_name: Option<String>,
    pub context_window: Option<i64>,
    pub supports_streaming: bool,
    pub supports_tools: bool,
    pub supports_vision: bool,
    pub supports_embeddings: bool,
    pub is_available: bool,
    pub last_seen_at: Option<String>,
    pub metadata_json: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderHealth {
    pub provider_id: String,
    pub is_reachable: bool,
    pub status: String,
    pub message: String,
}

/// ARC-003: describes what a provider *type* (protocol) supports, independent of any one
/// configured instance. Consumers (backend command handlers and the frontend UI) check these
/// flags instead of matching on `provider_type` strings, so adding a provider that, say,
/// doesn't support model pull/delete doesn't require touching every call site that might have
/// assumed all providers do. Deliberately does NOT duplicate `ProviderConfig.destinationClass`/
/// `isLocal` — those are computed per-instance from the configured base URL (see
/// `db::map_provider`), not a fixed property of the protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderCapabilities {
    pub streaming: bool,
    pub model_listing: bool,
    pub model_pull: bool,
    pub model_delete: bool,
    pub model_unload: bool,
    /// Whether Ark attaches its own `Authorization: Bearer <token>` for this provider type. See
    /// `LocalInferenceHostProvider::api_key`'s doc comment — only `built_in` gets one; a
    /// user-configured `local_inference_host` manages its own auth independently of Ark.
    pub requires_auth: bool,
    /// Whether this provider's model-listing endpoint is expected to report a real
    /// `contextWindow` per model. Currently `false` for every adapter — context-window
    /// discovery isn't implemented yet for either protocol (see `ModelInfo::context_window`,
    /// always `None` today) — kept as an explicit capability flag so a future adapter that does
    /// discover it, or a future improvement to an existing one, has somewhere accurate to
    /// declare that rather than the frontend having to guess from whether the field happens to
    /// be populated.
    pub reports_context_window: bool,
    pub vision: bool,
    pub embeddings: bool,
    pub tools: bool,
}

impl ProviderCapabilities {
    const fn none() -> Self {
        Self {
            streaming: false,
            model_listing: false,
            model_pull: false,
            model_delete: false,
            model_unload: false,
            requires_auth: false,
            reports_context_window: false,
            vision: false,
            embeddings: false,
            tools: false,
        }
    }

    /// Pure function of the provider *type* string — the same source `ProviderRegistry::create`
    /// dispatches on — so this can be computed both when constructing a live `Box<dyn Provider>`
    /// and when reading a `ProviderConfig` straight from the database (see `db::map_provider`),
    /// without needing a live provider instance either time.
    pub fn for_provider_type(provider_type: &str) -> Self {
        match provider_type {
            DEFAULT_PROVIDER_TYPE => Self {
                streaming: true,
                model_listing: true,
                model_pull: true,
                model_delete: true,
                requires_auth: false,
                ..Self::none()
            },
            BUILT_IN_PROVIDER_TYPE => Self {
                streaming: true,
                model_listing: true,
                requires_auth: true,
                ..Self::none()
            },
            LOCAL_INFERENCE_HOST_PROVIDER_TYPE => Self {
                streaming: true,
                model_listing: true,
                requires_auth: false,
                ..Self::none()
            },
            // An unrecognized provider type can't even be constructed (see
            // `ProviderRegistry::create`) — every capability is absent rather than guessed.
            _ => Self::none(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProviderChatRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    pub temperature: Option<f64>,
    pub max_tokens: Option<i64>,
    /// Optional caller deadline for the whole generation. `None` permits indefinite total
    /// duration while the independent connect/header/idle guards still apply.
    pub user_deadline: Option<Duration>,
}

#[derive(Debug, Clone, Default)]
pub struct ProviderChatUsage {
    pub input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
}

/// COR-003: bounds only the initial TCP connect phase, not the request/response lifecycle —
/// a slow-to-accept server fails fast, but an actively streaming generation is never killed
/// by this.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// Bounds the wait for HTTP response headers after request dispatch. This is intentionally
/// independent from both TCP connection and response-body idle timeouts.
const HEADER_TIMEOUT: Duration = Duration::from_secs(30);

/// COR-003: fires only when the provider stops sending bytes entirely for this long — not a
/// cap on total generation time. A generation that keeps producing tokens, however slowly,
/// can run indefinitely; one that goes silent (hung process, dropped connection) is detected
/// and surfaced instead of leaving the UI waiting forever.
const IDLE_READ_TIMEOUT: Duration = Duration::from_secs(120);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ProviderTimeoutPolicy {
    connect: Duration,
    header: Duration,
    idle: Duration,
}

impl Default for ProviderTimeoutPolicy {
    fn default() -> Self {
        Self {
            connect: CONNECT_TIMEOUT,
            header: HEADER_TIMEOUT,
            idle: IDLE_READ_TIMEOUT,
        }
    }
}

fn phase_timeout(
    configured: Duration,
    deadline: Option<tokio::time::Instant>,
) -> Result<(Duration, bool), AppError> {
    let Some(deadline) = deadline else {
        return Ok((configured, false));
    };
    let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
    if remaining.is_zero() {
        return Err(AppError::new(
            "stream_user_deadline",
            "The generation reached its requested deadline.",
        ));
    }
    Ok((configured.min(remaining), remaining <= configured))
}

fn stream_timeout_error(
    provider_name: &str,
    phase: &str,
    configured: Duration,
    deadline_limited: bool,
) -> AppError {
    if deadline_limited {
        AppError::new(
            "stream_user_deadline",
            "The generation reached its requested deadline.",
        )
    } else {
        AppError::new(
            format!("stream_{phase}_timeout"),
            format!(
                "{provider_name} exceeded the configured {phase} timeout of {} ms.",
                configured.as_millis()
            ),
        )
    }
}

/// COR-003: extracts complete newline-delimited lines from a raw byte buffer, leaving any
/// trailing incomplete line for the next call. Splits on the raw `\n` byte — which is always
/// safe across a UTF-8 boundary, since `\n` (0x0A) can never appear as a continuation byte
/// (those are all 0x80-0xBF) — rather than converting each network chunk to a `String` before
/// accumulating it, which silently corrupts a multi-byte character whose bytes happen to be
/// split across two chunks (each half gets independently lossy-converted to the U+FFFD
/// replacement character instead of being reassembled). Each complete line is decoded with a
/// lossy fallback only after all of its bytes are known, and trimmed (which also strips a
/// preceding `\r`, so CRLF and LF line endings both work).
fn drain_complete_lines(buffer: &mut Vec<u8>) -> Vec<String> {
    let mut lines = Vec::new();
    while let Some(newline_index) = buffer.iter().position(|&byte| byte == b'\n') {
        let line_bytes: Vec<u8> = buffer.drain(..=newline_index).collect();
        let line = String::from_utf8_lossy(&line_bytes[..line_bytes.len() - 1]);
        lines.push(line.trim().to_string());
    }
    lines
}

// ── Ollama ────────────────────────────────────────────────────────────────────

pub struct OllamaProvider {
    provider: ProviderConfig,
    client: Client,
    timeouts: ProviderTimeoutPolicy,
}

// ── Local inference host (OpenAI-compatible) ──────────────────────────────────

pub struct LocalInferenceHostProvider {
    provider: ProviderConfig,
    client: Client,
    /// SEC-002: bearer token for the managed built-in runtime, held only in memory for the
    /// life of this runtime instance — never part of `ProviderConfig` (which is also returned
    /// to the frontend over IPC and must never carry a secret), never logged, never persisted.
    /// `None` for a user-configured "local inference host" provider, which manages its own
    /// server and authentication independently of Ark.
    api_key: Option<String>,
    timeouts: ProviderTimeoutPolicy,
}

// ── Provider trait ────────────────────────────────────────────────────────────

/// ARC-003: the capability-based provider abstraction. Every current and future provider
/// adapter (Ollama, the OpenAI-compatible local inference host, the bundled built-in runtime,
/// and any test/future provider) implements this trait; every consumer — `generation.rs`,
/// `diagnostics.rs`, `provider_management.rs` — depends only on `dyn Provider` and
/// `ProviderCapabilities`, never on a concrete adapter type or a match over `provider_type`.
/// That is what makes adding a new provider type a matter of implementing this trait and adding
/// one arm to `ProviderRegistry::create` — not touching generation orchestration, diagnostics,
/// or model-management code, all of which only ever call trait methods.
///
/// `pull_model`/`delete_model` default to a clear "not supported" error — only `OllamaProvider`
/// overrides them today, since Ollama is the only protocol Ark integrates with that exposes
/// pull/delete over its API. This is "unsupported capabilities are absent/disabled with a
/// reason" in practice: a caller that invokes `pull_model` on a provider that doesn't support it
/// gets a typed `AppError` explaining exactly that, not a panic, not a silent no-op, and not a
/// match arm someone forgot to add.
#[async_trait]
pub trait Provider: Send + Sync {
    fn capabilities(&self) -> ProviderCapabilities;

    async fn health(&self) -> ProviderHealth;

    async fn list_models(&self, now: &str) -> Result<Vec<ModelInfo>, AppError>;

    async fn stream_chat(
        &self,
        request: ProviderChatRequest,
        on_delta: &mut (dyn for<'a> FnMut(&'a str) -> Result<(), AppError> + Send),
    ) -> Result<ProviderChatUsage, AppError>;

    async fn pull_model(
        &self,
        _model_name: &str,
        _on_progress: &mut (dyn FnMut(OllamaPullProgress) + Send),
    ) -> Result<(), AppError> {
        Err(AppError::invalid_input(
            "This provider does not support pulling models.",
        ))
    }

    async fn delete_model(&self, _model_name: &str) -> Result<(), AppError> {
        Err(AppError::invalid_input(
            "This provider does not support deleting models.",
        ))
    }
}

// ── Provider registry ─────────────────────────────────────────────────────────

/// ARC-003: the single place a `provider_type` string is mapped to a concrete adapter — provider
/// *registration*, not generation orchestration. This is the one match statement that
/// necessarily grows when a new provider type is added; nothing else does.
pub struct ProviderRegistry;

impl ProviderRegistry {
    pub fn create(provider: ProviderConfig) -> Result<Box<dyn Provider>, AppError> {
        Self::create_with_bearer_token(provider, None)
    }

    /// SEC-002: `bearer_token` is attached as `Authorization: Bearer <token>` on every request
    /// when present. Only meaningful for the built-in provider type — see
    /// `LocalInferenceHostProvider::api_key`.
    pub fn create_with_bearer_token(
        provider: ProviderConfig,
        bearer_token: Option<String>,
    ) -> Result<Box<dyn Provider>, AppError> {
        match provider.provider_type.as_str() {
            DEFAULT_PROVIDER_TYPE => Ok(Box::new(OllamaProvider::new(provider)?)),
            LOCAL_INFERENCE_HOST_PROVIDER_TYPE | BUILT_IN_PROVIDER_TYPE => Ok(Box::new(
                LocalInferenceHostProvider::new(provider, bearer_token)?,
            )),
            _ => Err(AppError::invalid_input(format!(
                "Provider type '{}' is not supported.",
                provider.provider_type
            ))),
        }
    }
}

#[async_trait]
impl Provider for OllamaProvider {
    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities::for_provider_type(&self.provider.provider_type)
    }

    async fn health(&self) -> ProviderHealth {
        self.health().await
    }

    async fn list_models(&self, now: &str) -> Result<Vec<ModelInfo>, AppError> {
        self.list_models(now).await
    }

    async fn stream_chat(
        &self,
        request: ProviderChatRequest,
        on_delta: &mut (dyn for<'a> FnMut(&'a str) -> Result<(), AppError> + Send),
    ) -> Result<ProviderChatUsage, AppError> {
        self.stream_chat(request, on_delta).await
    }

    async fn pull_model(
        &self,
        model_name: &str,
        on_progress: &mut (dyn FnMut(OllamaPullProgress) + Send),
    ) -> Result<(), AppError> {
        self.pull_model(model_name, on_progress).await
    }

    async fn delete_model(&self, model_name: &str) -> Result<(), AppError> {
        self.delete_model(model_name).await
    }
}

#[async_trait]
impl Provider for LocalInferenceHostProvider {
    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities::for_provider_type(&self.provider.provider_type)
    }

    async fn health(&self) -> ProviderHealth {
        self.health().await
    }

    async fn list_models(&self, now: &str) -> Result<Vec<ModelInfo>, AppError> {
        self.list_models(now).await
    }

    async fn stream_chat(
        &self,
        request: ProviderChatRequest,
        on_delta: &mut (dyn for<'a> FnMut(&'a str) -> Result<(), AppError> + Send),
    ) -> Result<ProviderChatUsage, AppError> {
        self.stream_chat(request, on_delta).await
    }

    // pull_model/delete_model intentionally not overridden: an OpenAI-compatible server has no
    // Ark-integrated pull/delete protocol, so the trait's default "not supported" error is the
    // correct, honest behavior — not a gap to fill in later.
}

// ── OllamaProvider impl ───────────────────────────────────────────────────────

impl OllamaProvider {
    pub fn new(provider: ProviderConfig) -> Result<Self, AppError> {
        Self::new_with_timeouts(provider, ProviderTimeoutPolicy::default())
    }

    fn new_with_timeouts(
        provider: ProviderConfig,
        timeouts: ProviderTimeoutPolicy,
    ) -> Result<Self, AppError> {
        if let Some(base_url) = provider.base_url.as_deref() {
            crate::security::enforce_persisted_destination_policy(
                base_url,
                provider.is_local,
                provider.allow_insecure_remote,
            )?;
        }
        Ok(Self {
            provider,
            // SEC-001: redirects disabled so a compromised/misbehaving server cannot silently
            // upgrade a request's destination class (e.g. a validated local URL redirecting to
            // a public host). Ollama's local API has no legitimate reason to redirect.
            // COR-003: no blanket request-duration `.timeout()` — that previously killed any
            // generation running longer than the configured value even while it was actively
            // producing tokens. `connect_timeout` only bounds the initial TCP handshake; an
            // active generation is bounded instead by the idle-read timeout in stream_chat,
            // which only fires when the provider stops sending bytes.
            client: Client::builder()
                .connect_timeout(timeouts.connect)
                .redirect(reqwest::redirect::Policy::none())
                .build()?,
            timeouts,
        })
    }

    pub async fn health(&self) -> ProviderHealth {
        let Some(base_url) = self.provider.base_url.as_deref() else {
            return ProviderHealth {
                provider_id: self.provider.id.clone(),
                is_reachable: false,
                status: "missing_base_url".to_string(),
                message: "Ollama base URL is not configured.".to_string(),
            };
        };

        let url = format!("{}/api/version", base_url.trim_end_matches('/'));
        match self
            .client
            .get(url)
            .timeout(Duration::from_secs(3))
            .send()
            .await
        {
            Ok(response) if response.status().is_success() => ProviderHealth {
                provider_id: self.provider.id.clone(),
                is_reachable: true,
                status: "reachable".to_string(),
                message: "Ollama is reachable.".to_string(),
            },
            Ok(response) => ProviderHealth {
                provider_id: self.provider.id.clone(),
                is_reachable: false,
                status: "unhealthy".to_string(),
                message: format!("Ollama returned HTTP {}.", response.status()),
            },
            Err(error) if error.is_connect() => ProviderHealth {
                provider_id: self.provider.id.clone(),
                is_reachable: false,
                status: "unreachable".to_string(),
                message: "Ollama is not reachable. Start Ollama and refresh models.".to_string(),
            },
            Err(error) => ProviderHealth {
                provider_id: self.provider.id.clone(),
                is_reachable: false,
                status: "error".to_string(),
                message: format!("Ollama health check failed: {error}"),
            },
        }
    }

    pub async fn list_models(&self, now: &str) -> Result<Vec<ModelInfo>, AppError> {
        let base_url = self
            .provider
            .base_url
            .as_deref()
            .ok_or_else(|| AppError::provider("Ollama base URL is not configured."))?;

        let url = format!("{}/api/tags", base_url.trim_end_matches('/'));
        let response = self
            .client
            .get(url)
            .timeout(Duration::from_secs(5))
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(AppError::provider(format!(
                "Ollama model list failed with HTTP {}.",
                response.status()
            )));
        }

        let tags: OllamaTagsResponse = response.json().await?;

        Ok(tags
            .models
            .into_iter()
            .map(|model| {
                let metadata_json = serde_json::to_string(&model).ok();
                ModelInfo {
                    id: format!("{}:{}", self.provider.id, model.name),
                    provider_id: self.provider.id.clone(),
                    display_name: Some(model.name.clone()),
                    name: model.name,
                    context_window: None,
                    supports_streaming: true,
                    supports_tools: false,
                    supports_vision: false,
                    supports_embeddings: false,
                    is_available: true,
                    last_seen_at: Some(now.to_string()),
                    metadata_json,
                    created_at: now.to_string(),
                    updated_at: now.to_string(),
                }
            })
            .collect())
    }

    pub async fn stream_chat(
        &self,
        request: ProviderChatRequest,
        on_delta: &mut (dyn for<'a> FnMut(&'a str) -> Result<(), AppError> + Send),
    ) -> Result<ProviderChatUsage, AppError> {
        let base_url = self
            .provider
            .base_url
            .as_deref()
            .ok_or_else(|| AppError::provider("Ollama base URL is not configured."))?;

        if request.model.trim().is_empty() {
            return Err(AppError::invalid_input(
                "Select a local model before sending a message.",
            ));
        }

        let deadline = request
            .user_deadline
            .map(|duration| tokio::time::Instant::now() + duration);
        let url = format!("{}/api/chat", base_url.trim_end_matches('/'));
        let body = OllamaChatRequest {
            model: request.model,
            messages: request.messages,
            stream: true,
            options: OllamaOptions {
                temperature: request.temperature,
                num_predict: request.max_tokens,
            },
        };

        let (header_wait, deadline_limited) = phase_timeout(self.timeouts.header, deadline)?;
        let response = tokio::time::timeout(header_wait, self.client.post(url).json(&body).send())
            .await
            .map_err(|_| {
                stream_timeout_error("Ollama", "header", self.timeouts.header, deadline_limited)
            })??;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(AppError::provider(format!(
                "Ollama chat request failed with HTTP {status}. {error_text}"
            )));
        }

        let mut stream = response.bytes_stream();
        let mut buffer: Vec<u8> = Vec::new();
        let mut usage = ProviderChatUsage::default();

        // COR-003: `done` must actually be observed — a stream that ends (connection closed,
        // provider crashed, network drop) without ever sending it must not be reported as a
        // successful completion.
        loop {
            let (idle_wait, deadline_limited) = phase_timeout(self.timeouts.idle, deadline)?;
            let next_chunk = match tokio::time::timeout(idle_wait, stream.next()).await {
                Ok(chunk) => chunk,
                Err(_) => {
                    return Err(stream_timeout_error(
                        "Ollama",
                        "idle",
                        self.timeouts.idle,
                        deadline_limited,
                    ));
                }
            };

            let Some(chunk) = next_chunk else {
                return Err(AppError::new(
                    "stream_incomplete",
                    "Ollama closed the connection before signaling the response was complete.",
                ));
            };
            let bytes = chunk?;
            buffer.extend_from_slice(&bytes);

            for line in drain_complete_lines(&mut buffer) {
                if line.is_empty() {
                    continue;
                }

                let event: OllamaChatStreamEvent =
                    serde_json::from_str(&line).map_err(|error| {
                        AppError::new(
                            "protocol_error",
                            format!("Invalid Ollama streaming response: {error}"),
                        )
                    })?;

                if let Some(message) = event.message {
                    if !message.content.is_empty() {
                        on_delta(&message.content)?;
                    }
                }

                if event.done {
                    usage.input_tokens = event.prompt_eval_count;
                    usage.output_tokens = event.eval_count;
                    return Ok(usage);
                }
            }
        }
    }
}

// ── LocalInferenceHostProvider impl ──────────────────────────────────────────

impl LocalInferenceHostProvider {
    pub fn new(provider: ProviderConfig, api_key: Option<String>) -> Result<Self, AppError> {
        Self::new_with_timeouts(provider, api_key, ProviderTimeoutPolicy::default())
    }

    fn new_with_timeouts(
        provider: ProviderConfig,
        api_key: Option<String>,
        timeouts: ProviderTimeoutPolicy,
    ) -> Result<Self, AppError> {
        if let Some(base_url) = provider.base_url.as_deref() {
            crate::security::enforce_persisted_destination_policy(
                base_url,
                provider.is_local,
                provider.allow_insecure_remote,
            )?;
        }
        Ok(Self {
            provider,
            // SEC-001: see OllamaProvider::new — redirects are disabled to prevent a
            // validated destination from being silently reclassified via HTTP redirect.
            // COR-003: see OllamaProvider::new — no blanket request-duration timeout; long
            // generations are bounded by the idle-read timeout in stream_chat instead.
            client: Client::builder()
                .connect_timeout(timeouts.connect)
                .redirect(reqwest::redirect::Policy::none())
                .build()?,
            api_key,
            timeouts,
        })
    }

    /// SEC-002: attaches `Authorization: Bearer <token>` when this runtime holds one — see
    /// `api_key`'s doc comment for who does and doesn't get one.
    fn authorize(&self, builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        match &self.api_key {
            Some(key) => builder.bearer_auth(key),
            None => builder,
        }
    }

    pub async fn health(&self) -> ProviderHealth {
        let Some(base_url) = self.provider.base_url.as_deref() else {
            return ProviderHealth {
                provider_id: self.provider.id.clone(),
                is_reachable: false,
                status: "missing_base_url".to_string(),
                message: "Local inference host base URL is not configured.".to_string(),
            };
        };

        let base = base_url.trim_end_matches('/');

        // Prefer /health (llama.cpp server exposes this); fall back to /v1/models.
        let health_url = format!("{base}/health");
        if let Ok(resp) = self
            .authorize(self.client.get(&health_url))
            .timeout(Duration::from_secs(3))
            .send()
            .await
        {
            if resp.status().is_success() {
                return ProviderHealth {
                    provider_id: self.provider.id.clone(),
                    is_reachable: true,
                    status: "reachable".to_string(),
                    message: "Local inference host is reachable.".to_string(),
                };
            }
        }

        let models_url = format!("{base}/v1/models");
        match self
            .authorize(self.client.get(&models_url))
            .timeout(Duration::from_secs(3))
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => ProviderHealth {
                provider_id: self.provider.id.clone(),
                is_reachable: true,
                status: "reachable".to_string(),
                message: "Local inference host is reachable.".to_string(),
            },
            Ok(resp) => ProviderHealth {
                provider_id: self.provider.id.clone(),
                is_reachable: false,
                status: "unhealthy".to_string(),
                message: format!("Local inference host returned HTTP {}.", resp.status()),
            },
            Err(error) if error.is_connect() => ProviderHealth {
                provider_id: self.provider.id.clone(),
                is_reachable: false,
                status: "unreachable".to_string(),
                message:
                    "Local inference host is not reachable. Start the server and refresh models."
                        .to_string(),
            },
            Err(error) => ProviderHealth {
                provider_id: self.provider.id.clone(),
                is_reachable: false,
                status: "error".to_string(),
                message: format!("Local inference host health check failed: {error}"),
            },
        }
    }

    pub async fn list_models(&self, now: &str) -> Result<Vec<ModelInfo>, AppError> {
        let base_url = self.provider.base_url.as_deref().ok_or_else(|| {
            AppError::provider("Local inference host base URL is not configured.")
        })?;

        let url = format!("{}/v1/models", base_url.trim_end_matches('/'));
        let response = self
            .authorize(self.client.get(&url))
            .timeout(Duration::from_secs(5))
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(AppError::provider(format!(
                "Local inference host model list failed with HTTP {}.",
                response.status()
            )));
        }

        let list: OpenAIModelsResponse = response.json().await.map_err(|error| {
            AppError::provider(format!(
                "Invalid model list from local inference host: {error}"
            ))
        })?;

        Ok(list
            .data
            .into_iter()
            .map(|model| ModelInfo {
                id: format!("{}:{}", self.provider.id, model.id),
                provider_id: self.provider.id.clone(),
                name: model.id.clone(),
                display_name: Some(model.id),
                context_window: None,
                supports_streaming: true,
                supports_tools: false,
                supports_vision: false,
                supports_embeddings: false,
                is_available: true,
                last_seen_at: Some(now.to_string()),
                metadata_json: None,
                created_at: now.to_string(),
                updated_at: now.to_string(),
            })
            .collect())
    }

    pub async fn stream_chat(
        &self,
        request: ProviderChatRequest,
        on_delta: &mut (dyn for<'a> FnMut(&'a str) -> Result<(), AppError> + Send),
    ) -> Result<ProviderChatUsage, AppError> {
        let base_url = self.provider.base_url.as_deref().ok_or_else(|| {
            AppError::provider("Local inference host base URL is not configured.")
        })?;

        if request.model.trim().is_empty() {
            return Err(AppError::invalid_input(
                "Select a model before sending a message.",
            ));
        }

        let deadline = request
            .user_deadline
            .map(|duration| tokio::time::Instant::now() + duration);
        let url = format!("{}/v1/chat/completions", base_url.trim_end_matches('/'));
        let body = OpenAIChatRequest {
            model: request.model,
            messages: request.messages,
            stream: true,
            temperature: request.temperature,
            max_tokens: request.max_tokens,
        };

        let (header_wait, deadline_limited) = phase_timeout(self.timeouts.header, deadline)?;
        let response = tokio::time::timeout(
            header_wait,
            self.authorize(self.client.post(&url).json(&body)).send(),
        )
        .await
        .map_err(|_| {
            stream_timeout_error(
                "Local inference host",
                "header",
                self.timeouts.header,
                deadline_limited,
            )
        })??;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(AppError::provider(format!(
                "Local inference host chat request failed with HTTP {status}. {error_text}"
            )));
        }

        let mut stream = response.bytes_stream();
        let mut buffer: Vec<u8> = Vec::new();
        let mut usage = ProviderChatUsage::default();
        // COR-003: a stream that ends without ever seeing `[DONE]` or a populated
        // `finish_reason` must not be reported as a successful completion.
        let mut saw_completion_marker = false;

        loop {
            let (idle_wait, deadline_limited) = phase_timeout(self.timeouts.idle, deadline)?;
            let next_chunk = match tokio::time::timeout(idle_wait, stream.next()).await {
                Ok(chunk) => chunk,
                Err(_) => {
                    return Err(stream_timeout_error(
                        "Local inference host",
                        "idle",
                        self.timeouts.idle,
                        deadline_limited,
                    ));
                }
            };

            let Some(chunk) = next_chunk else {
                if saw_completion_marker {
                    return Ok(usage);
                }
                return Err(AppError::new(
                    "stream_incomplete",
                    "Local inference host closed the connection before signaling the response was complete.",
                ));
            };
            let bytes = chunk?;
            buffer.extend_from_slice(&bytes);

            for line in drain_complete_lines(&mut buffer) {
                if line.is_empty() {
                    continue;
                }

                // SSE format: "data: {json}" or "data: [DONE]". Lines with any other prefix
                // (SSE comments starting with ':', "event:", "id:", "retry:") are part of the
                // protocol and are legitimately not data frames — skip only those, not
                // malformed JSON in an actual data frame.
                let Some(data) = line.strip_prefix("data:").map(str::trim_start) else {
                    continue;
                };

                if data.is_empty() {
                    continue;
                }

                if data.trim() == "[DONE]" {
                    return Ok(usage);
                }

                let event: OpenAIChatStreamEvent = serde_json::from_str(data).map_err(|error| {
                    AppError::new(
                        "protocol_error",
                        format!("Invalid local inference host streaming response: {error}"),
                    )
                })?;

                for choice in &event.choices {
                    if let Some(content) = &choice.delta.content {
                        if !content.is_empty() {
                            on_delta(content)?;
                        }
                    }
                    if choice.finish_reason.is_some() {
                        saw_completion_marker = true;
                    }
                }

                // Capture usage from the final chunk when the server includes it.
                if let Some(event_usage) = event.usage {
                    usage.input_tokens = event_usage.prompt_tokens;
                    usage.output_tokens = event_usage.completion_tokens;
                }
            }
        }
    }
}

// ── Ollama model management ───────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OllamaPullProgress {
    pub provider_id: String,
    pub model_name: String,
    pub status: String,
    pub total: Option<i64>,
    pub completed: Option<i64>,
    pub digest: Option<String>,
    pub error: Option<String>,
}

impl OllamaProvider {
    pub async fn pull_model(
        &self,
        model_name: &str,
        on_progress: &mut (dyn FnMut(OllamaPullProgress) + Send),
    ) -> Result<(), AppError> {
        let base_url = self
            .provider
            .base_url
            .as_deref()
            .ok_or_else(|| AppError::provider("Ollama base URL is not configured."))?;

        let url = format!("{}/api/pull", base_url.trim_end_matches('/'));
        let body = serde_json::json!({ "model": model_name, "stream": true });

        let response = self.client.post(url).json(&body).send().await?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(AppError::provider(format!(
                "Ollama pull failed with HTTP {status}: {text}"
            )));
        }

        let mut stream = response.bytes_stream();
        let mut buffer = String::new();

        while let Some(chunk) = stream.next().await {
            let bytes = chunk?;
            buffer.push_str(&String::from_utf8_lossy(&bytes));

            while let Some(newline_pos) = buffer.find('\n') {
                let line = buffer[..newline_pos].trim().to_string();
                buffer = buffer[newline_pos + 1..].to_string();

                if line.is_empty() {
                    continue;
                }

                if let Ok(event) = serde_json::from_str::<OllamaPullEvent>(&line) {
                    let done = event.status.as_deref() == Some("success");
                    on_progress(OllamaPullProgress {
                        provider_id: self.provider.id.clone(),
                        model_name: model_name.to_string(),
                        status: event.status.unwrap_or_default(),
                        total: event.total,
                        completed: event.completed,
                        digest: event.digest,
                        error: event.error,
                    });
                    if done {
                        return Ok(());
                    }
                }
            }
        }

        Ok(())
    }

    pub async fn delete_model(&self, model_name: &str) -> Result<(), AppError> {
        let base_url = self
            .provider
            .base_url
            .as_deref()
            .ok_or_else(|| AppError::provider("Ollama base URL is not configured."))?;

        let url = format!("{}/api/delete", base_url.trim_end_matches('/'));
        let body = serde_json::json!({ "model": model_name });

        let response = self.client.delete(url).json(&body).send().await?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(AppError::provider(format!(
                "Ollama delete failed with HTTP {status}: {text}"
            )));
        }

        Ok(())
    }
}

// ── Ollama DTOs ───────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Serialize)]
struct OllamaTagsResponse {
    models: Vec<OllamaTagModel>,
}

#[derive(Debug, Deserialize, Serialize)]
struct OllamaTagModel {
    name: String,
    #[serde(default)]
    modified_at: Option<String>,
    #[serde(default)]
    size: Option<i64>,
    #[serde(default)]
    digest: Option<String>,
    #[serde(default)]
    details: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct OllamaPullEvent {
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    digest: Option<String>,
    #[serde(default)]
    total: Option<i64>,
    #[serde(default)]
    completed: Option<i64>,
    #[serde(default)]
    error: Option<String>,
}

#[derive(Debug, Serialize)]
struct OllamaChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    stream: bool,
    options: OllamaOptions,
}

#[derive(Debug, Serialize)]
struct OllamaOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    num_predict: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct OllamaChatStreamEvent {
    #[serde(default)]
    message: Option<OllamaStreamMessage>,
    #[serde(default)]
    done: bool,
    #[serde(default)]
    prompt_eval_count: Option<i64>,
    #[serde(default)]
    eval_count: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct OllamaStreamMessage {
    content: String,
}

// ── OpenAI-compatible DTOs ────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
struct OpenAIChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct OpenAIModelsResponse {
    data: Vec<OpenAIModel>,
}

#[derive(Debug, Deserialize)]
struct OpenAIModel {
    id: String,
}

#[derive(Debug, Deserialize)]
struct OpenAIChatStreamEvent {
    choices: Vec<OpenAIStreamChoice>,
    #[serde(default)]
    usage: Option<OpenAIUsage>,
}

#[derive(Debug, Deserialize)]
struct OpenAIStreamChoice {
    delta: OpenAIStreamDelta,
    #[serde(default)]
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAIStreamDelta {
    #[serde(default)]
    content: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAIUsage {
    #[serde(default)]
    prompt_tokens: Option<i64>,
    #[serde(default)]
    completion_tokens: Option<i64>,
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn provider_config(provider_type: &str) -> ProviderConfig {
        ProviderConfig {
            id: "provider".to_string(),
            name: "Provider".to_string(),
            provider_type: provider_type.to_string(),
            base_url: Some("http://localhost:11434".to_string()),
            api_key_ref: None,
            default_model_id: None,
            default_temperature: Some(0.7),
            default_max_tokens: Some(2048),
            is_local: true,
            allow_insecure_remote: false,
            destination_class: "loopback".to_string(),
            capabilities: ProviderCapabilities::for_provider_type(provider_type),
            is_enabled: true,
            created_at: "2026-07-02T00:00:00Z".to_string(),
            updated_at: "2026-07-02T00:00:00Z".to_string(),
        }
    }

    // ARC-003: `ProviderRegistry::create` returns an opaque `Box<dyn Provider>` — there is no
    // variant to pattern-match on anymore, by design (that's the whole point: callers depend on
    // the trait, never on which concrete adapter backs it). What's observable and worth testing
    // is that the registry actually wires up the adapter whose capability profile matches the
    // requested provider type.
    #[test]
    fn creates_a_provider_with_ollama_capabilities_for_the_default_provider_type() {
        let provider = ProviderRegistry::create(provider_config(DEFAULT_PROVIDER_TYPE))
            .expect("registry constructs");
        assert_eq!(
            provider.capabilities(),
            ProviderCapabilities::for_provider_type(DEFAULT_PROVIDER_TYPE)
        );
        assert!(
            provider.capabilities().model_pull,
            "Ollama must support model pull"
        );
    }

    #[test]
    fn creates_a_provider_with_local_inference_host_capabilities_for_that_provider_type() {
        let provider =
            ProviderRegistry::create(provider_config(LOCAL_INFERENCE_HOST_PROVIDER_TYPE))
                .expect("registry constructs");
        assert_eq!(
            provider.capabilities(),
            ProviderCapabilities::for_provider_type(LOCAL_INFERENCE_HOST_PROVIDER_TYPE)
        );
        assert!(
            !provider.capabilities().model_pull,
            "a local inference host must not claim model pull support"
        );
    }

    #[test]
    fn built_in_provider_type_requires_auth_but_local_inference_host_does_not() {
        // Both provider types are backed by the same `LocalInferenceHostProvider` adapter, but
        // they are not interchangeable — only `built_in` carries Ark's own bearer token (see
        // `LocalInferenceHostProvider::api_key`). Capabilities must track the *type*, not just
        // which struct implements the trait.
        let built_in = ProviderRegistry::create(provider_config(BUILT_IN_PROVIDER_TYPE))
            .expect("registry constructs");
        let local_host =
            ProviderRegistry::create(provider_config(LOCAL_INFERENCE_HOST_PROVIDER_TYPE))
                .expect("registry constructs");
        assert!(built_in.capabilities().requires_auth);
        assert!(!local_host.capabilities().requires_auth);
    }

    // ── SEC-002: bearer token authentication ─────────────────────────────────

    #[test]
    fn attaches_bearer_auth_header_when_a_token_is_present() {
        let provider = LocalInferenceHostProvider::new(
            provider_config(BUILT_IN_PROVIDER_TYPE),
            Some("super-secret-launch-token".to_string()),
        )
        .expect("provider constructs");

        let client = Client::new();
        let request = provider
            .authorize(client.get("http://127.0.0.1:11435/v1/models"))
            .build()
            .expect("request builds");

        let auth_header = request
            .headers()
            .get(reqwest::header::AUTHORIZATION)
            .expect("Authorization header must be present");
        assert_eq!(auth_header, "Bearer super-secret-launch-token");
    }

    #[test]
    fn omits_bearer_auth_header_when_no_token_is_configured() {
        // A user-configured "local inference host" provider never receives a token — it
        // manages its own server and authentication independently of Ark.
        let provider = LocalInferenceHostProvider::new(
            provider_config(LOCAL_INFERENCE_HOST_PROVIDER_TYPE),
            None,
        )
        .expect("provider constructs");

        let client = Client::new();
        let request = provider
            .authorize(client.get("http://127.0.0.1:8080/v1/models"))
            .build()
            .expect("request builds");

        assert!(request
            .headers()
            .get(reqwest::header::AUTHORIZATION)
            .is_none());
    }

    #[test]
    fn rejects_unsupported_provider_type() {
        let error = match ProviderRegistry::create(provider_config("cloud")) {
            Ok(_) => panic!("unsupported provider should fail"),
            Err(error) => error,
        };
        assert_eq!(error.code, "invalid_input");
    }

    #[test]
    fn an_unrecognized_provider_type_has_no_capabilities() {
        // Mirrors `rejects_unsupported_provider_type`: since the registry refuses to construct
        // an unrecognized provider type at all, its capability profile is "nothing supported"
        // rather than a guess — there is no live instance to ask, so nothing should be assumed.
        let capabilities = ProviderCapabilities::for_provider_type("cloud");
        assert_eq!(capabilities, ProviderCapabilities::none());
    }

    // ── COR-003: drain_complete_lines ────────────────────────────────────────

    #[test]
    fn drain_complete_lines_extracts_multiple_lines_from_one_chunk() {
        let mut buffer = b"line one\nline two\nline three\n".to_vec();
        let lines = drain_complete_lines(&mut buffer);
        assert_eq!(lines, vec!["line one", "line two", "line three"]);
        assert!(buffer.is_empty(), "fully consumed buffer must be empty");
    }

    #[test]
    fn drain_complete_lines_leaves_an_incomplete_trailing_line_buffered() {
        let mut buffer = b"complete line\nincomplete tail".to_vec();
        let lines = drain_complete_lines(&mut buffer);
        assert_eq!(lines, vec!["complete line"]);
        assert_eq!(buffer, b"incomplete tail");

        // Completing the line on a later call must yield exactly the reassembled line.
        buffer.extend_from_slice(b" now complete\n");
        let lines = drain_complete_lines(&mut buffer);
        assert_eq!(lines, vec!["incomplete tail now complete"]);
    }

    #[test]
    fn drain_complete_lines_handles_crlf_and_lf_line_endings() {
        let mut buffer = b"crlf line\r\nlf line\nanother crlf\r\n".to_vec();
        let lines = drain_complete_lines(&mut buffer);
        assert_eq!(lines, vec!["crlf line", "lf line", "another crlf"]);
    }

    #[test]
    fn drain_complete_lines_yields_empty_strings_for_blank_lines() {
        let mut buffer = b"first\n\nthird\n".to_vec();
        let lines = drain_complete_lines(&mut buffer);
        assert_eq!(lines, vec!["first", "", "third"]);
    }

    #[test]
    fn drain_complete_lines_reassembles_multibyte_utf8_split_across_chunk_boundaries() {
        // "🚀" is a 4-byte UTF-8 sequence (F0 9F 9A 80). Split it mid-character across two
        // "chunks" the way a real network read could — the old implementation converted each
        // chunk to a String independently via from_utf8_lossy before accumulating, which
        // corrupts exactly this case into replacement characters. This proves the fix: no
        // conversion happens until a complete line's bytes are all present.
        let rocket = "🚀".as_bytes();
        assert_eq!(rocket.len(), 4);

        let mut buffer: Vec<u8> = Vec::new();
        // Chunk 1: "before " + first 2 bytes of the rocket emoji.
        buffer.extend_from_slice(b"before ");
        buffer.extend_from_slice(&rocket[..2]);
        assert!(
            drain_complete_lines(&mut buffer).is_empty(),
            "no newline yet, nothing to drain"
        );

        // Chunk 2: remaining 2 bytes of the emoji + " after\n".
        buffer.extend_from_slice(&rocket[2..]);
        buffer.extend_from_slice(b" after\n");

        let lines = drain_complete_lines(&mut buffer);
        assert_eq!(lines, vec!["before 🚀 after"]);
    }

    #[test]
    fn drain_complete_lines_handles_a_single_byte_at_a_time() {
        // Pathological case: every "chunk" is exactly one byte, including mid-multibyte-char.
        let source = "Hello 世界\n".as_bytes();
        let mut buffer: Vec<u8> = Vec::new();
        let mut collected: Vec<String> = Vec::new();

        for &byte in source {
            buffer.push(byte);
            collected.extend(drain_complete_lines(&mut buffer));
        }

        assert_eq!(collected, vec!["Hello 世界"]);
    }

    #[test]
    fn drain_complete_lines_is_a_no_op_on_premature_eof_with_no_trailing_newline() {
        // Simulates a connection that closed mid-line: bytes remain buffered, nothing is
        // ever emitted as a "complete" line, and no panic/corruption occurs.
        let mut buffer = b"partial response with no terminator".to_vec();
        let lines = drain_complete_lines(&mut buffer);
        assert!(lines.is_empty());
        assert_eq!(buffer, b"partial response with no terminator");
    }

    // ── FND-004 / COR-003: real async-I/O provider protocol tests ───────────
    //
    // These exercise the full `stream_chat` implementation — real TCP, real reqwest
    // byte-stream reads, real chunk boundaries — against a mock HTTP server, not just the
    // pure `drain_complete_lines` function above. They directly verify this task's own
    // acceptance criteria: an immediate/complete stream succeeds, a stream that ends without
    // its protocol's completion marker fails as `stream_incomplete` while preserving whatever
    // partial content already arrived, and a malformed frame fails as `protocol_error`.

    use super::test_support::{
        start_mock_stream_server, start_scripted_stream_server, MockChunk, MockResponsePlan,
    };
    use std::sync::{Arc, Mutex};

    fn ollama_provider_for_port(port: u16) -> OllamaProvider {
        let mut config = provider_config(DEFAULT_PROVIDER_TYPE);
        config.base_url = Some(format!("http://127.0.0.1:{port}"));
        OllamaProvider::new(config).expect("provider constructs")
    }

    fn ollama_provider_with_timeouts(port: u16, timeouts: ProviderTimeoutPolicy) -> OllamaProvider {
        let mut config = provider_config(DEFAULT_PROVIDER_TYPE);
        config.base_url = Some(format!("http://127.0.0.1:{port}"));
        OllamaProvider::new_with_timeouts(config, timeouts).expect("provider constructs")
    }

    fn local_inference_provider_for_port(port: u16) -> LocalInferenceHostProvider {
        let mut config = provider_config(LOCAL_INFERENCE_HOST_PROVIDER_TYPE);
        config.base_url = Some(format!("http://127.0.0.1:{port}"));
        LocalInferenceHostProvider::new(config, None).expect("provider constructs")
    }

    fn test_timeouts(header_ms: u64, idle_ms: u64) -> ProviderTimeoutPolicy {
        ProviderTimeoutPolicy {
            connect: Duration::from_millis(100),
            header: Duration::from_millis(header_ms),
            idle: Duration::from_millis(idle_ms),
        }
    }

    /// ARC-003 acceptance: "Ollama and local OpenAI-compatible adapters pass one contract suite
    /// plus protocol-specific suites." This is that shared suite — behavior every `Provider`
    /// impl must uphold regardless of protocol — exercised once per adapter below via
    /// `ollama_provider_passes_the_shared_provider_contract` and
    /// `local_inference_host_provider_passes_the_shared_provider_contract`. Protocol-specific
    /// behavior (NDJSON vs SSE framing, completion markers, etc.) stays in the dedicated
    /// `ollama_stream_chat_*`/`local_inference_host_stream_chat_*` tests elsewhere in this file.
    /// Port 1 is used as a reliably-unreachable destination: it requires root/administrator
    /// privileges to bind, so nothing in a test environment is ever listening on it and a
    /// connection to it is refused immediately rather than timing out.
    async fn assert_provider_contract(provider: &dyn Provider, expected_provider_id: &str) {
        let health = provider.health().await;
        assert_eq!(
            health.provider_id, expected_provider_id,
            "health() must always echo the provider's own id, reachable or not"
        );
        assert!(
            !health.is_reachable,
            "nothing listens on port 1 — health must report unreachable, not panic or hang"
        );

        let models_result = provider.list_models("2026-01-01T00:00:00Z").await;
        assert!(
            models_result.is_err(),
            "list_models against an unreachable server must surface a typed error, not panic or hang"
        );

        // Every adapter this suite constructs is expected to support streaming and model
        // listing — the two capabilities every `Provider` impl in this codebase currently has.
        let capabilities = provider.capabilities();
        assert!(capabilities.streaming);
        assert!(capabilities.model_listing);
    }

    #[tokio::test]
    async fn ollama_provider_passes_the_shared_provider_contract() {
        let provider = ollama_provider_for_port(1);
        assert_provider_contract(&provider, "provider").await;
    }

    #[tokio::test]
    async fn local_inference_host_provider_passes_the_shared_provider_contract() {
        let provider = local_inference_provider_for_port(1);
        assert_provider_contract(&provider, "provider").await;
    }

    fn chat_request() -> ProviderChatRequest {
        ProviderChatRequest {
            model: "test-model".to_string(),
            messages: vec![ChatMessage {
                role: "user".to_string(),
                content: "hello".to_string(),
            }],
            temperature: Some(0.7),
            max_tokens: Some(64),
            user_deadline: None,
        }
    }

    /// Collects every delta delivered via `on_delta` into a single string, for assertion —
    /// shared via `Arc<Mutex<..>>` so it can be inspected after `stream_chat` returns.
    fn collector() -> (Arc<Mutex<String>>, impl FnMut(&str) -> Result<(), AppError>) {
        let collected = Arc::new(Mutex::new(String::new()));
        let for_closure = collected.clone();
        let on_delta = move |delta: &str| {
            for_closure.lock().unwrap().push_str(delta);
            Ok(())
        };
        (collected, on_delta)
    }

    #[tokio::test]
    async fn ollama_stream_chat_succeeds_on_a_complete_ndjson_stream() {
        let port = start_mock_stream_server(
            "HTTP/1.1 200 OK",
            vec![MockChunk::new(
                "{\"message\":{\"content\":\"Hello\"},\"done\":false}\n\
                 {\"message\":{\"content\":\", world\"},\"done\":false}\n\
                 {\"message\":{\"content\":\"\"},\"done\":true,\"prompt_eval_count\":5,\"eval_count\":10}\n"
                    .as_bytes()
                    .to_vec(),
            )],
        )
        .await;

        let provider = ollama_provider_for_port(port);
        let (collected, mut on_delta) = collector();
        let usage = provider
            .stream_chat(chat_request(), &mut on_delta)
            .await
            .expect("stream completes");

        assert_eq!(*collected.lock().unwrap(), "Hello, world");
        assert_eq!(usage.input_tokens, Some(5));
        assert_eq!(usage.output_tokens, Some(10));
    }

    #[tokio::test]
    async fn ollama_stream_chat_fails_as_incomplete_and_preserves_partial_content_on_premature_close(
    ) {
        // Valid NDJSON, but the connection closes before a `"done":true` line ever arrives —
        // simulating a crashed Ollama process or a dropped network connection mid-generation.
        let port = start_mock_stream_server(
            "HTTP/1.1 200 OK",
            vec![MockChunk::new(
                "{\"message\":{\"content\":\"Partial\"},\"done\":false}\n\
                 {\"message\":{\"content\":\" answer\"},\"done\":false}\n"
                    .as_bytes()
                    .to_vec(),
            )],
        )
        .await;

        let provider = ollama_provider_for_port(port);
        let (collected, mut on_delta) = collector();
        let error = match provider.stream_chat(chat_request(), &mut on_delta).await {
            Ok(_) => panic!("a stream with no completion marker must not report success"),
            Err(error) => error,
        };

        assert_eq!(error.code, "stream_incomplete");
        // The deltas that did arrive before the connection closed must still have reached the
        // caller — this is what "preserve partial content" means in practice: the caller (in
        // production, spawn_provider_stream) already has this text buffered/checkpointed and
        // will mark the message `interrupted`, not silently discard it.
        assert_eq!(*collected.lock().unwrap(), "Partial answer");
    }

    #[tokio::test]
    async fn ollama_stream_chat_fails_with_protocol_error_on_malformed_json_frame() {
        let port = start_mock_stream_server(
            "HTTP/1.1 200 OK",
            vec![MockChunk::new(
                "{\"message\":{\"content\":\"Hello\"},\"done\":false}\n\
                 this is not valid json at all\n"
                    .as_bytes()
                    .to_vec(),
            )],
        )
        .await;

        let provider = ollama_provider_for_port(port);
        let (_collected, mut on_delta) = collector();
        let error = match provider.stream_chat(chat_request(), &mut on_delta).await {
            Ok(_) => panic!("a malformed frame must not report success"),
            Err(error) => error,
        };
        assert_eq!(error.code, "protocol_error");
    }

    #[tokio::test]
    async fn ollama_stream_chat_reconstructs_content_split_across_real_network_writes() {
        // The final NDJSON line is split into two separate TCP writes with a delay between
        // them — a real (if artificially slow) simulation of a delta arriving across two
        // reqwest byte-stream reads, exercising the actual async buffering path rather than a
        // synthetic in-memory byte array.
        let line = "{\"message\":{\"content\":\"Splitcontent\"},\"done\":false}\n\
                     {\"message\":{\"content\":\"\"},\"done\":true}\n";
        let bytes = line.as_bytes();
        let midpoint = bytes.len() / 2;
        let port = start_mock_stream_server(
            "HTTP/1.1 200 OK",
            vec![
                MockChunk::new(bytes[..midpoint].to_vec()),
                MockChunk::delayed(Duration::from_millis(30), bytes[midpoint..].to_vec()),
            ],
        )
        .await;

        let provider = ollama_provider_for_port(port);
        let (collected, mut on_delta) = collector();
        provider
            .stream_chat(chat_request(), &mut on_delta)
            .await
            .expect("stream completes");

        assert_eq!(*collected.lock().unwrap(), "Splitcontent");
    }

    #[tokio::test]
    async fn ollama_simulator_fragments_every_byte_and_captures_the_exact_request() {
        let fixture = "{\"message\":{\"content\":\"Grüße 👋\"},\"done\":false}\n\
                       {\"message\":{\"content\":\"\"},\"done\":true}\n";
        let plan = MockResponsePlan::new(
            "HTTP/1.1 200 OK",
            MockChunk::fragment_every_byte(fixture.as_bytes()),
        )
        .with_header_delay(Duration::from_millis(5));
        let (port, mut requests) = start_scripted_stream_server(vec![plan]).await;

        let provider = ollama_provider_for_port(port);
        let (collected, mut on_delta) = collector();
        provider
            .stream_chat(chat_request(), &mut on_delta)
            .await
            .expect("a stream fragmented at every byte boundary completes");

        assert_eq!(*collected.lock().unwrap(), "Grüße 👋");
        let request = requests.recv().await.expect("captured request");
        assert_eq!(request.method, "POST");
        assert_eq!(request.path, "/api/chat");
        assert_eq!(request.header("content-type"), Some("application/json"));
        let payload: serde_json::Value =
            serde_json::from_slice(&request.body).expect("valid JSON request body");
        assert_eq!(payload["model"], "test-model");
        assert_eq!(payload["messages"][0]["role"], "user");
        assert_eq!(payload["messages"][0]["content"], "hello");
        assert_eq!(payload["stream"], true);
    }

    #[tokio::test]
    async fn ollama_simulator_supports_cancellation_between_delta_and_terminal_frame() {
        let (port, _requests) = start_scripted_stream_server(vec![MockResponsePlan::new(
            "HTTP/1.1 200 OK",
            vec![
                MockChunk::new("{\"message\":{\"content\":\"first\"},\"done\":false}\n".as_bytes()),
                MockChunk::delayed(
                    Duration::from_millis(20),
                    "{\"message\":{\"content\":\"late\"},\"done\":false}\n\
                     {\"message\":{\"content\":\"\"},\"done\":true}\n"
                        .as_bytes(),
                ),
            ],
        )])
        .await;

        let provider = ollama_provider_for_port(port);
        let mut deltas = Vec::new();
        let error = provider
            .stream_chat(chat_request(), &mut |delta| {
                deltas.push(delta.to_string());
                Err(AppError::new("cancelled", "test cancellation"))
            })
            .await
            .expect_err("the callback cancellation must stop the stream");

        assert_eq!(error.code, "cancelled");
        assert_eq!(deltas, ["first"]);
    }

    #[tokio::test]
    async fn ollama_simulator_can_script_a_failed_attempt_followed_by_an_explicit_retry() {
        let (port, mut requests) = start_scripted_stream_server(vec![
            MockResponsePlan::new(
                "HTTP/1.1 503 Service Unavailable",
                vec![MockChunk::new(b"temporarily unavailable")],
            ),
            MockResponsePlan::new(
                "HTTP/1.1 200 OK",
                vec![MockChunk::new(
                    "{\"message\":{\"content\":\"retry succeeded\"},\"done\":false}\n\
                     {\"message\":{\"content\":\"\"},\"done\":true}\n"
                        .as_bytes(),
                )],
            ),
        ])
        .await;
        let provider = ollama_provider_for_port(port);

        let (_first_collected, mut first_delta) = collector();
        let first_error = provider
            .stream_chat(chat_request(), &mut first_delta)
            .await
            .expect_err("the first scripted attempt fails");
        assert_eq!(first_error.code, "provider_error");

        let (retry_collected, mut retry_delta) = collector();
        provider
            .stream_chat(chat_request(), &mut retry_delta)
            .await
            .expect("the caller's explicit retry succeeds");
        assert_eq!(*retry_collected.lock().unwrap(), "retry succeeded");

        let first_request = requests.recv().await.expect("first request captured");
        let retry_request = requests.recv().await.expect("retry request captured");
        assert_eq!(first_request.path, "/api/chat");
        assert_eq!(retry_request.path, "/api/chat");
        assert_eq!(first_request.body, retry_request.body);
    }

    #[tokio::test]
    async fn immediate_completion_fixture_is_stable_across_one_thousand_runs() {
        const RUNS: usize = 1_000;
        let fixture =
            "{\"message\":{\"content\":\"ok\"},\"done\":false}\n{\"message\":{\"content\":\"\"},\"done\":true}\n";
        let plans = (0..RUNS)
            .map(|_| {
                MockResponsePlan::new("HTTP/1.1 200 OK", vec![MockChunk::new(fixture.as_bytes())])
            })
            .collect();
        let (port, _requests) = start_scripted_stream_server(plans).await;
        let provider = ollama_provider_for_port(port);

        for run in 0..RUNS {
            let (collected, mut on_delta) = collector();
            provider
                .stream_chat(chat_request(), &mut on_delta)
                .await
                .unwrap_or_else(|error| panic!("immediate run {run} failed: {error}"));
            assert_eq!(*collected.lock().unwrap(), "ok", "run {run}");
        }
    }

    #[test]
    fn connect_header_and_idle_timeouts_are_independently_configurable() {
        let policy = ProviderTimeoutPolicy {
            connect: Duration::from_millis(7),
            header: Duration::from_millis(11),
            idle: Duration::from_millis(13),
        };
        let provider =
            OllamaProvider::new_with_timeouts(provider_config(DEFAULT_PROVIDER_TYPE), policy)
                .expect("provider constructs");
        assert_eq!(provider.timeouts, policy);
    }

    #[tokio::test]
    async fn ollama_header_timeout_is_typed_and_independent() {
        let plan = MockResponsePlan::new(
            "HTTP/1.1 200 OK",
            vec![MockChunk::new(
                "{\"message\":{\"content\":\"\"},\"done\":true}\n".as_bytes(),
            )],
        )
        .with_header_delay(Duration::from_millis(30));
        let (port, _requests) = start_scripted_stream_server(vec![plan]).await;
        let provider = ollama_provider_with_timeouts(port, test_timeouts(5, 100));
        let (_collected, mut on_delta) = collector();
        let error = provider
            .stream_chat(chat_request(), &mut on_delta)
            .await
            .expect_err("delayed headers must time out");
        assert_eq!(error.code, "stream_header_timeout");
    }

    #[tokio::test]
    async fn ollama_idle_timeout_is_typed_and_independent() {
        let plan = MockResponsePlan::new(
            "HTTP/1.1 200 OK",
            vec![MockChunk::delayed(
                Duration::from_millis(30),
                "{\"message\":{\"content\":\"\"},\"done\":true}\n".as_bytes(),
            )],
        );
        let (port, _requests) = start_scripted_stream_server(vec![plan]).await;
        let provider = ollama_provider_with_timeouts(port, test_timeouts(100, 5));
        let (_collected, mut on_delta) = collector();
        let error = provider
            .stream_chat(chat_request(), &mut on_delta)
            .await
            .expect_err("silent body must time out");
        assert_eq!(error.code, "stream_idle_timeout");
    }

    #[tokio::test]
    async fn caller_deadline_is_distinct_from_header_and_idle_timeouts() {
        let plan = MockResponsePlan::new(
            "HTTP/1.1 200 OK",
            vec![MockChunk::new(
                "{\"message\":{\"content\":\"\"},\"done\":true}\n".as_bytes(),
            )],
        )
        .with_header_delay(Duration::from_millis(30));
        let (port, _requests) = start_scripted_stream_server(vec![plan]).await;
        let provider = ollama_provider_with_timeouts(port, test_timeouts(100, 100));
        let mut request = chat_request();
        request.user_deadline = Some(Duration::from_millis(5));
        let (_collected, mut on_delta) = collector();
        let error = provider
            .stream_chat(request, &mut on_delta)
            .await
            .expect_err("caller deadline must win");
        assert_eq!(error.code, "stream_user_deadline");
    }

    #[tokio::test]
    async fn slow_progress_can_exceed_the_idle_window_without_a_total_timeout() {
        let mut chunks = (0..20)
            .map(|_| {
                MockChunk::delayed(
                    Duration::from_millis(10),
                    "{\"message\":{\"content\":\"x\"},\"done\":false}\n".as_bytes(),
                )
            })
            .collect::<Vec<_>>();
        chunks.push(MockChunk::delayed(
            Duration::from_millis(10),
            "{\"message\":{\"content\":\"\"},\"done\":true}\n".as_bytes(),
        ));
        let (port, _requests) =
            start_scripted_stream_server(vec![MockResponsePlan::new("HTTP/1.1 200 OK", chunks)])
                .await;
        let provider = ollama_provider_with_timeouts(port, test_timeouts(100, 100));
        let (collected, mut on_delta) = collector();
        provider
            .stream_chat(chat_request(), &mut on_delta)
            .await
            .expect("continuous slow progress must not hit a total timeout");
        assert_eq!(*collected.lock().unwrap(), "xxxxxxxxxxxxxxxxxxxx");
    }

    #[tokio::test]
    async fn openai_sse_accepts_comments_empty_data_and_crlf_or_lf_frames() {
        let body = ": heartbeat\r\n\r\ndata:\r\n\r\n\
                    data:{\"choices\":[{\"delta\":{\"content\":\"CRLF\"}}]}\r\n\
                    : another comment\n\
                    data: {\"choices\":[{\"delta\":{\"content\":\" + LF\"}}]}\n\
                    data: [DONE]\n";
        let port =
            start_mock_stream_server("HTTP/1.1 200 OK", vec![MockChunk::new(body.as_bytes())])
                .await;
        let provider = local_inference_provider_for_port(port);
        let (collected, mut on_delta) = collector();
        provider
            .stream_chat(chat_request(), &mut on_delta)
            .await
            .expect("valid SSE variants complete");
        assert_eq!(*collected.lock().unwrap(), "CRLF + LF");
    }

    #[tokio::test]
    async fn local_inference_host_stream_chat_succeeds_on_a_complete_sse_stream() {
        let port = start_mock_stream_server(
            "HTTP/1.1 200 OK",
            vec![MockChunk::new(
                "data: {\"choices\":[{\"delta\":{\"content\":\"Hi\"}}]}\n\
                 data: {\"choices\":[{\"delta\":{\"content\":\" there\"}}],\"usage\":{\"prompt_tokens\":3,\"completion_tokens\":2}}\n\
                 data: [DONE]\n"
                    .as_bytes()
                    .to_vec(),
            )],
        )
        .await;

        let provider = local_inference_provider_for_port(port);
        let (collected, mut on_delta) = collector();
        let usage = provider
            .stream_chat(chat_request(), &mut on_delta)
            .await
            .expect("stream completes");

        assert_eq!(*collected.lock().unwrap(), "Hi there");
        assert_eq!(usage.input_tokens, Some(3));
        assert_eq!(usage.output_tokens, Some(2));
    }

    #[tokio::test]
    async fn local_inference_host_stream_chat_fails_as_incomplete_on_premature_close() {
        let port = start_mock_stream_server(
            "HTTP/1.1 200 OK",
            vec![MockChunk::new(
                "data: {\"choices\":[{\"delta\":{\"content\":\"Partial\"}}]}\n"
                    .as_bytes()
                    .to_vec(),
            )],
        )
        .await;

        let provider = local_inference_provider_for_port(port);
        let (collected, mut on_delta) = collector();
        let error = match provider.stream_chat(chat_request(), &mut on_delta).await {
            Ok(_) => panic!("a stream with no [DONE]/finish_reason must not report success"),
            Err(error) => error,
        };
        assert_eq!(error.code, "stream_incomplete");
        assert_eq!(*collected.lock().unwrap(), "Partial");
    }

    #[tokio::test]
    async fn local_inference_host_stream_chat_succeeds_via_finish_reason_without_literal_done() {
        // Some OpenAI-compatible servers signal completion via `finish_reason` on the last
        // choice; per this task's acceptance criteria ("requires [DONE] or a valid finish
        // reason"), that alone must be accepted as a valid completion marker even if the
        // connection closes without ever sending a literal `data: [DONE]` line.
        let port = start_mock_stream_server(
            "HTTP/1.1 200 OK",
            vec![MockChunk::new(
                "data: {\"choices\":[{\"delta\":{\"content\":\"Done\"},\"finish_reason\":\"stop\"}]}\n"
                    .as_bytes()
                    .to_vec(),
            )],
        )
        .await;

        let provider = local_inference_provider_for_port(port);
        let (collected, mut on_delta) = collector();
        provider
            .stream_chat(chat_request(), &mut on_delta)
            .await
            .expect("finish_reason alone must complete the stream");

        assert_eq!(*collected.lock().unwrap(), "Done");
    }

    #[tokio::test]
    async fn local_inference_host_stream_chat_fails_with_protocol_error_on_malformed_data_frame() {
        let port = start_mock_stream_server(
            "HTTP/1.1 200 OK",
            vec![MockChunk::new(
                "data: {not valid json\n".as_bytes().to_vec(),
            )],
        )
        .await;

        let provider = local_inference_provider_for_port(port);
        let (_collected, mut on_delta) = collector();
        let error = match provider.stream_chat(chat_request(), &mut on_delta).await {
            Ok(_) => panic!("a malformed data frame must not report success"),
            Err(error) => error,
        };
        assert_eq!(error.code, "protocol_error");
    }

    #[tokio::test]
    async fn ollama_stream_chat_rejects_non_success_http_status() {
        let port = start_mock_stream_server(
            "HTTP/1.1 500 Internal Server Error",
            vec![MockChunk::new(b"server exploded".to_vec())],
        )
        .await;

        let provider = ollama_provider_for_port(port);
        let (_collected, mut on_delta) = collector();
        let error = match provider.stream_chat(chat_request(), &mut on_delta).await {
            Ok(_) => panic!("a 500 response must not report success"),
            Err(error) => error,
        };
        assert_eq!(error.code, "provider_error");
    }

    /// SEC-001 (real, executable verification — not just code review): proves
    /// `redirect::Policy::none()` on the provider HTTP clients actually prevents a redirect
    /// from being followed, by pointing a real `Location` header at a second "evil" server and
    /// asserting that second server is never contacted.
    #[tokio::test]
    async fn ollama_client_does_not_follow_redirects() {
        use std::sync::atomic::{AtomicBool, Ordering};
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        use tokio::net::TcpListener;

        let evil_listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind evil server");
        let evil_port = evil_listener.local_addr().expect("evil addr").port();
        let evil_was_contacted = Arc::new(AtomicBool::new(false));
        let evil_flag = evil_was_contacted.clone();
        tokio::spawn(async move {
            if evil_listener.accept().await.is_ok() {
                evil_flag.store(true, Ordering::SeqCst);
            }
        });

        // A hand-built mock server (not the generic start_mock_stream_server helper) because
        // this test needs a real `Location` response header, not just body bytes.
        let redirect_listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind redirect server");
        let redirecting_port = redirect_listener
            .local_addr()
            .expect("redirect addr")
            .port();
        tokio::spawn(async move {
            let Ok((mut socket, _)) = redirect_listener.accept().await else {
                return;
            };

            // Drain the request before responding — matching test_support's pattern. Writing
            // a response while the client is still sending its request body can reset the
            // connection before the client ever reads the response, which would fail this
            // test for a reason unrelated to what it's actually verifying.
            let mut buf = [0u8; 4096];
            loop {
                match socket.read(&mut buf).await {
                    Ok(0) => break,
                    Err(_) => break,
                    Ok(n) => {
                        if buf[..n].windows(4).any(|w| w == b"\r\n\r\n") {
                            break;
                        }
                    }
                }
            }

            let response = format!(
                "HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:{evil_port}/\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
            );
            let _ = socket.write_all(response.as_bytes()).await;
        });

        let provider = ollama_provider_for_port(redirecting_port);
        let (_collected, mut on_delta) = collector();
        let error = match provider.stream_chat(chat_request(), &mut on_delta).await {
            Ok(_) => panic!("a 302 must not be silently followed to success"),
            Err(error) => error,
        };
        // With redirect::Policy::none(), reqwest returns the 302 response itself rather than
        // following it — this project's own `if !response.status().is_success()` handling then
        // correctly surfaces it as a failed request. The literal "302" in the message is the
        // proof that the redirect was received but not transparently followed.
        assert_eq!(error.code, "provider_error");
        assert!(
            error.message.contains("302"),
            "error should report the un-followed redirect status: {error:?}"
        );

        // Give the evil server a moment to have been contacted, if it was going to be.
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert!(
            !evil_was_contacted.load(Ordering::SeqCst),
            "the redirect target must never be contacted when redirects are disabled"
        );
    }
}
