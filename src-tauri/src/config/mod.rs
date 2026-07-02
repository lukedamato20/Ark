pub const DEFAULT_PROVIDER_ID: &str = "ollama";
pub const DEFAULT_PROVIDER_NAME: &str = "Ollama";
pub const DEFAULT_PROVIDER_TYPE: &str = "ollama";
pub const DEFAULT_OLLAMA_BASE_URL: &str = "http://localhost:11434";

pub const LOCAL_INFERENCE_HOST_PROVIDER_ID: &str = "local_inference_host";
pub const LOCAL_INFERENCE_HOST_PROVIDER_NAME: &str = "Local inference host";
pub const LOCAL_INFERENCE_HOST_PROVIDER_TYPE: &str = "local_inference_host";
pub const LOCAL_INFERENCE_HOST_BASE_URL: &str = "http://localhost:8080";

pub const BUILT_IN_PROVIDER_ID: &str = "built_in";
pub const BUILT_IN_PROVIDER_NAME: &str = "Built-in (llama.cpp)";
pub const BUILT_IN_PROVIDER_TYPE: &str = "built_in";
pub const BUILT_IN_PROVIDER_BASE_URL: &str = "http://127.0.0.1:11435";
pub const BUILT_IN_DEFAULT_PORT: u16 = 11435;

pub const DEFAULT_TEMPERATURE: f64 = 0.7;
pub const DEFAULT_MAX_TOKENS: i64 = 2048;
