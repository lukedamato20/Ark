use crate::chat::ChatMessage;
use crate::config::{BUILT_IN_PROVIDER_TYPE, DEFAULT_PROVIDER_TYPE, LOCAL_INFERENCE_HOST_PROVIDER_TYPE};
use crate::errors::AppError;
use futures_util::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

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
    pub streaming_enabled: bool,
    pub is_local: bool,
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

#[derive(Debug, Clone)]
pub struct ProviderChatRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    pub temperature: Option<f64>,
    pub max_tokens: Option<i64>,
}

#[derive(Debug, Clone, Default)]
pub struct ProviderChatUsage {
    pub input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
}

// ── Ollama ────────────────────────────────────────────────────────────────────

pub struct OllamaProvider {
    provider: ProviderConfig,
    client: Client,
}

// ── Local inference host (OpenAI-compatible) ──────────────────────────────────

pub struct LocalInferenceHostProvider {
    provider: ProviderConfig,
    client: Client,
}

// ── Runtime dispatcher ────────────────────────────────────────────────────────

pub enum ProviderRuntime {
    Ollama(OllamaProvider),
    LocalInferenceHost(LocalInferenceHostProvider),
}

impl ProviderRuntime {
    pub fn from_config(provider: ProviderConfig) -> Result<Self, AppError> {
        match provider.provider_type.as_str() {
            DEFAULT_PROVIDER_TYPE => Ok(Self::Ollama(OllamaProvider::new(provider)?)),
            LOCAL_INFERENCE_HOST_PROVIDER_TYPE | BUILT_IN_PROVIDER_TYPE => {
                Ok(Self::LocalInferenceHost(LocalInferenceHostProvider::new(provider)?))
            }
            _ => Err(AppError::invalid_input(format!(
                "Provider type '{}' is not supported.",
                provider.provider_type
            ))),
        }
    }

    pub async fn health(&self) -> ProviderHealth {
        match self {
            Self::Ollama(p) => p.health().await,
            Self::LocalInferenceHost(p) => p.health().await,
        }
    }

    pub async fn list_models(&self, now: &str) -> Result<Vec<ModelInfo>, AppError> {
        match self {
            Self::Ollama(p) => p.list_models(now).await,
            Self::LocalInferenceHost(p) => p.list_models(now).await,
        }
    }

    pub async fn stream_chat<F>(
        &self,
        request: ProviderChatRequest,
        on_delta: F,
    ) -> Result<ProviderChatUsage, AppError>
    where
        F: FnMut(&str) -> Result<(), AppError>,
    {
        match self {
            Self::Ollama(p) => p.stream_chat(request, on_delta).await,
            Self::LocalInferenceHost(p) => p.stream_chat(request, on_delta).await,
        }
    }
}

// ── OllamaProvider impl ───────────────────────────────────────────────────────

impl OllamaProvider {
    pub fn new(provider: ProviderConfig) -> Result<Self, AppError> {
        Ok(Self {
            provider,
            client: Client::builder().timeout(Duration::from_secs(60)).build()?,
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
        match self.client.get(url).timeout(Duration::from_secs(3)).send().await {
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
        let response = self.client.get(url).timeout(Duration::from_secs(5)).send().await?;

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

    pub async fn stream_chat<F>(
        &self,
        request: ProviderChatRequest,
        mut on_delta: F,
    ) -> Result<ProviderChatUsage, AppError>
    where
        F: FnMut(&str) -> Result<(), AppError>,
    {
        let base_url = self
            .provider
            .base_url
            .as_deref()
            .ok_or_else(|| AppError::provider("Ollama base URL is not configured."))?;

        if request.model.trim().is_empty() {
            return Err(AppError::invalid_input("Select a local model before sending a message."));
        }

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

        let response = self.client.post(url).json(&body).send().await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(AppError::provider(format!(
                "Ollama chat request failed with HTTP {status}. {error_text}"
            )));
        }

        let mut stream = response.bytes_stream();
        let mut buffer = String::new();
        let mut usage = ProviderChatUsage::default();

        while let Some(chunk) = stream.next().await {
            let bytes = chunk?;
            buffer.push_str(&String::from_utf8_lossy(&bytes));

            while let Some(newline_index) = buffer.find('\n') {
                let line = buffer[..newline_index].trim().to_string();
                buffer = buffer[newline_index + 1..].to_string();

                if line.is_empty() {
                    continue;
                }

                let event: OllamaChatStreamEvent = serde_json::from_str(&line).map_err(|error| {
                    AppError::provider(format!("Invalid Ollama streaming response: {error}"))
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

        Ok(usage)
    }
}

// ── LocalInferenceHostProvider impl ──────────────────────────────────────────

impl LocalInferenceHostProvider {
    pub fn new(provider: ProviderConfig) -> Result<Self, AppError> {
        Ok(Self {
            provider,
            client: Client::builder().timeout(Duration::from_secs(120)).build()?,
        })
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
        if let Ok(resp) = self.client.get(&health_url).timeout(Duration::from_secs(3)).send().await {
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
        match self.client.get(&models_url).timeout(Duration::from_secs(3)).send().await {
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
                message: "Local inference host is not reachable. Start the server and refresh models.".to_string(),
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
        let base_url = self
            .provider
            .base_url
            .as_deref()
            .ok_or_else(|| AppError::provider("Local inference host base URL is not configured."))?;

        let url = format!("{}/v1/models", base_url.trim_end_matches('/'));
        let response = self.client.get(&url).timeout(Duration::from_secs(5)).send().await?;

        if !response.status().is_success() {
            return Err(AppError::provider(format!(
                "Local inference host model list failed with HTTP {}.",
                response.status()
            )));
        }

        let list: OpenAIModelsResponse = response.json().await.map_err(|error| {
            AppError::provider(format!("Invalid model list from local inference host: {error}"))
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

    pub async fn stream_chat<F>(
        &self,
        request: ProviderChatRequest,
        mut on_delta: F,
    ) -> Result<ProviderChatUsage, AppError>
    where
        F: FnMut(&str) -> Result<(), AppError>,
    {
        let base_url = self
            .provider
            .base_url
            .as_deref()
            .ok_or_else(|| AppError::provider("Local inference host base URL is not configured."))?;

        if request.model.trim().is_empty() {
            return Err(AppError::invalid_input("Select a model before sending a message."));
        }

        let url = format!("{}/v1/chat/completions", base_url.trim_end_matches('/'));
        let body = OpenAIChatRequest {
            model: request.model,
            messages: request.messages,
            stream: true,
            temperature: request.temperature,
            max_tokens: request.max_tokens,
        };

        let response = self.client.post(&url).json(&body).send().await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(AppError::provider(format!(
                "Local inference host chat request failed with HTTP {status}. {error_text}"
            )));
        }

        let mut stream = response.bytes_stream();
        let mut buffer = String::new();
        let mut usage = ProviderChatUsage::default();

        while let Some(chunk) = stream.next().await {
            let bytes = chunk?;
            buffer.push_str(&String::from_utf8_lossy(&bytes));

            while let Some(newline_index) = buffer.find('\n') {
                let line = buffer[..newline_index].trim().to_string();
                buffer = buffer[newline_index + 1..].to_string();

                if line.is_empty() {
                    continue;
                }

                // SSE format: "data: {json}" or "data: [DONE]"
                let Some(data) = line.strip_prefix("data: ") else {
                    continue;
                };

                if data.trim() == "[DONE]" {
                    return Ok(usage);
                }

                let event: OpenAIChatStreamEvent = match serde_json::from_str(data) {
                    Ok(event) => event,
                    Err(_) => continue,
                };

                for choice in &event.choices {
                    if let Some(content) = &choice.delta.content {
                        if !content.is_empty() {
                            on_delta(content)?;
                        }
                    }
                }

                // Capture usage from the final chunk when the server includes it.
                if let Some(event_usage) = event.usage {
                    usage.input_tokens = event_usage.prompt_tokens;
                    usage.output_tokens = event_usage.completion_tokens;
                }
            }
        }

        Ok(usage)
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
            streaming_enabled: true,
            is_local: true,
            is_enabled: true,
            created_at: "2026-07-02T00:00:00Z".to_string(),
            updated_at: "2026-07-02T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn creates_ollama_runtime_from_provider_config() {
        let runtime = ProviderRuntime::from_config(provider_config(DEFAULT_PROVIDER_TYPE));
        assert!(matches!(runtime, Ok(ProviderRuntime::Ollama(_))));
    }

    #[test]
    fn creates_local_inference_host_runtime_from_provider_config() {
        let runtime =
            ProviderRuntime::from_config(provider_config(LOCAL_INFERENCE_HOST_PROVIDER_TYPE));
        assert!(matches!(runtime, Ok(ProviderRuntime::LocalInferenceHost(_))));
    }

    #[test]
    fn rejects_unsupported_provider_runtime() {
        let error = match ProviderRuntime::from_config(provider_config("cloud")) {
            Ok(_) => panic!("unsupported provider should fail"),
            Err(error) => error,
        };
        assert_eq!(error.code, "invalid_input");
    }
}
