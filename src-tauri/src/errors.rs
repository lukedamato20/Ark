use serde::Serialize;

#[derive(Debug, Serialize, Clone)]
pub struct AppError {
    pub code: String,
    pub message: String,
}

impl AppError {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }

    pub fn database(error: impl std::fmt::Display) -> Self {
        Self::new("database_error", format!("Local database error: {error}"))
    }

    pub fn provider(error: impl Into<String>) -> Self {
        Self::new("provider_error", error)
    }

    pub fn not_found(entity: impl Into<String>) -> Self {
        Self::new("not_found", format!("{} was not found.", entity.into()))
    }

    pub fn invalid_input(message: impl Into<String>) -> Self {
        Self::new("invalid_input", message)
    }
}

impl std::fmt::Display for AppError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for AppError {}

impl From<rusqlite::Error> for AppError {
    fn from(value: rusqlite::Error) -> Self {
        Self::database(value)
    }
}

impl From<std::io::Error> for AppError {
    fn from(value: std::io::Error) -> Self {
        Self::new("io_error", format!("Local file error: {value}"))
    }
}

impl From<reqwest::Error> for AppError {
    fn from(value: reqwest::Error) -> Self {
        if value.is_timeout() {
            Self::provider("Provider request timed out.")
        } else if value.is_connect() {
            Self::provider("Provider is unreachable. Check that Ollama is running.")
        } else {
            Self::provider(format!("Provider request failed: {value}"))
        }
    }
}
