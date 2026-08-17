pub const DEFAULT_PROVIDER_ID: &str = "ollama";
pub const DEFAULT_PROVIDER_NAME: &str = "Ollama";
pub const DEFAULT_PROVIDER_TYPE: &str = "ollama";
pub const DEFAULT_OLLAMA_BASE_URL: &str = "http://localhost:11434";

pub const LOCAL_INFERENCE_HOST_PROVIDER_ID: &str = "local_inference_host";
pub const LOCAL_INFERENCE_HOST_PROVIDER_NAME: &str = "Local inference host";
pub const LOCAL_INFERENCE_HOST_PROVIDER_TYPE: &str = "local_inference_host";
pub const LOCAL_INFERENCE_HOST_BASE_URL: &str = "http://localhost:8080";

/// FTR-007: a curated remote adapter using the same OpenAI-compatible wire implementation as
/// `local_inference_host`, but with a fixed official endpoint and required bearer credential.
/// It is never seeded; a user must explicitly create it and acknowledge the remote route.
pub const OPENAI_PROVIDER_TYPE: &str = "openai";
pub const OPENAI_PROVIDER_BASE_URL: &str = "https://api.openai.com";

pub const BUILT_IN_PROVIDER_ID: &str = "built_in";
pub const BUILT_IN_PROVIDER_NAME: &str = "Built-in (llama.cpp)";
pub const BUILT_IN_PROVIDER_TYPE: &str = "built_in";
pub const BUILT_IN_PROVIDER_BASE_URL: &str = "http://127.0.0.1:11435";

pub const DEFAULT_TEMPERATURE: f64 = 0.7;
pub const DEFAULT_MAX_TOKENS: i64 = 2048;

pub fn is_seeded_provider_id(provider_id: &str) -> bool {
    matches!(
        provider_id,
        DEFAULT_PROVIDER_ID | LOCAL_INFERENCE_HOST_PROVIDER_ID | BUILT_IN_PROVIDER_ID
    )
}

/// FTR-003: portable, workspace-scoped fallback instructions. Stored in the existing
/// `app_settings` table rather than device settings because they are part of the workspace's
/// behavior and must travel with its database.
pub const APPLICATION_INSTRUCTIONS_SETTING_KEY: &str = "generation.application_instructions";
