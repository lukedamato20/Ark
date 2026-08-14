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
    /// COR-010: classifies the specific SQLite failure class where possible, instead of
    /// collapsing every database error into one generic code. This is what lets the frontend
    /// present a typed recovery action (retry / choose a different workspace / open read-only)
    /// rather than a single indefinite "something went wrong" state.
    fn from(value: rusqlite::Error) -> Self {
        if let rusqlite::Error::SqliteFailure(sqlite_error, _) = &value {
            match sqlite_error.code {
                rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked => {
                    return Self::new(
                        "database_locked",
                        "The local database is locked. Ark may already be running — close other instances and try again.",
                    );
                }
                rusqlite::ErrorCode::DatabaseCorrupt | rusqlite::ErrorCode::NotADatabase => {
                    return Self::new(
                        "database_corrupt",
                        "The local database appears to be corrupted or is not a valid Ark database file.",
                    );
                }
                rusqlite::ErrorCode::DiskFull => {
                    return Self::new(
                        "disk_full",
                        "The disk is full. Free up space and try again.",
                    );
                }
                rusqlite::ErrorCode::ReadOnly | rusqlite::ErrorCode::PermissionDenied => {
                    return Self::new(
                        "workspace_read_only",
                        "The workspace folder is read-only. Choose a different workspace or fix its permissions.",
                    );
                }
                _ => {}
            }
        }

        Self::database(value)
    }
}

impl From<std::io::Error> for AppError {
    fn from(value: std::io::Error) -> Self {
        if value.kind() == std::io::ErrorKind::PermissionDenied
            || matches!(value.raw_os_error(), Some(13 | 19 | 30))
        {
            return Self::new(
                "workspace_read_only",
                format!("The workspace or configuration folder is not writable: {value}"),
            );
        }
        if value.kind() == std::io::ErrorKind::StorageFull
            || matches!(value.raw_os_error(), Some(28 | 39 | 112))
        {
            return Self::new(
                "disk_full",
                format!("There is not enough free disk space to complete the operation: {value}"),
            );
        }
        Self::new("io_error", format!("Local file error: {value}"))
    }
}

impl From<reqwest::Error> for AppError {
    fn from(value: reqwest::Error) -> Self {
        if value.is_timeout() {
            Self::provider("Provider request timed out.")
        } else if value.is_connect() {
            Self::provider("Provider is unreachable. Check that Ollama is running.")
        } else if value.is_redirect() {
            // SEC-001: providers' HTTP clients run with redirect::Policy::none() — reqwest
            // surfaces a blocked redirect as this distinct error variant (not as the 3xx
            // response itself), so this is the expected, correct outcome of that policy, not a
            // failure to report vaguely.
            Self::new(
                "redirect_blocked",
                "The provider tried to redirect this request, which Ark blocks for privacy/security. \
                 Verify the provider's base URL is correct.",
            )
        } else {
            Self::provider(format!("Provider request failed: {value}"))
        }
    }
}
