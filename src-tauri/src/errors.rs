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
            // SEC-001: this branch is for a redirect *policy that errors* (e.g.
            // `redirect::Policy::limited(n)` after exceeding `n` hops) — see
            // `errors::tests::classifies_an_exceeded_redirect_limit_as_redirect_blocked` for a
            // real reqwest error that actually triggers it. Every provider's own HTTP client
            // instead uses `redirect::Policy::none()`, which does not error at all: it returns
            // the 3xx response itself, which `providers/mod.rs`'s own
            // `if !response.status().is_success()` handling then reports as `provider_error`
            // (see `providers::tests::ollama_client_does_not_follow_redirects`) — so in this
            // codebase as it stands today, `redirect_blocked` is reachable but not yet reached
            // by any real call site. Kept because it is still the correct classification for
            // any future reqwest client configured with an erroring redirect policy.
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

#[cfg(test)]
mod tests {
    use super::*;

    // ── AppError constructors ──────────────────────────────────────────────

    #[test]
    fn constructors_produce_the_expected_codes() {
        assert_eq!(AppError::new("x", "y").code, "x");
        assert_eq!(AppError::database("boom").code, "database_error");
        assert_eq!(AppError::provider("boom").code, "provider_error");
        assert_eq!(AppError::not_found("Conversation").code, "not_found");
        assert!(AppError::not_found("Conversation")
            .message
            .contains("Conversation"));
        assert_eq!(AppError::invalid_input("bad").code, "invalid_input");
    }

    #[test]
    fn display_includes_code_and_message() {
        let error = AppError::new("some_code", "some message");
        assert_eq!(error.to_string(), "some_code: some message");
    }

    // ── rusqlite::Error classification ─────────────────────────────────────

    fn sqlite_failure(raw_extended_code: i32) -> rusqlite::Error {
        rusqlite::Error::SqliteFailure(rusqlite::ffi::Error::new(raw_extended_code), None)
    }

    #[test]
    fn classifies_busy_and_locked_as_database_locked() {
        assert_eq!(AppError::from(sqlite_failure(5)).code, "database_locked"); // SQLITE_BUSY
        assert_eq!(AppError::from(sqlite_failure(6)).code, "database_locked"); // SQLITE_LOCKED
    }

    #[test]
    fn classifies_corrupt_and_notadb_as_database_corrupt() {
        assert_eq!(AppError::from(sqlite_failure(11)).code, "database_corrupt"); // SQLITE_CORRUPT
        assert_eq!(AppError::from(sqlite_failure(26)).code, "database_corrupt");
        // SQLITE_NOTADB
    }

    #[test]
    fn classifies_full_as_disk_full() {
        assert_eq!(AppError::from(sqlite_failure(13)).code, "disk_full"); // SQLITE_FULL
    }

    #[test]
    fn classifies_readonly_and_permission_as_workspace_read_only() {
        assert_eq!(
            AppError::from(sqlite_failure(8)).code,
            "workspace_read_only"
        ); // SQLITE_READONLY
        assert_eq!(
            AppError::from(sqlite_failure(3)).code,
            "workspace_read_only"
        ); // SQLITE_PERM
    }

    #[test]
    fn an_unclassified_sqlite_failure_falls_back_to_the_generic_database_code() {
        // SQLITE_MISUSE — a real SQLite result code, but not one of the four special-cased
        // classes above, proving the fallback path (Self::database) is reachable and correct.
        assert_eq!(AppError::from(sqlite_failure(21)).code, "database_error");
    }

    #[test]
    fn a_non_sqlite_failure_rusqlite_error_falls_back_to_the_generic_database_code() {
        // InvalidQuery isn't a `SqliteFailure` variant at all, proving the match on `if let
        // rusqlite::Error::SqliteFailure(..)` correctly falls through for every other variant.
        assert_eq!(
            AppError::from(rusqlite::Error::InvalidQuery).code,
            "database_error"
        );
    }

    // ── std::io::Error classification ──────────────────────────────────────

    #[test]
    fn classifies_permission_denied_kind_as_workspace_read_only() {
        let error = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied");
        assert_eq!(AppError::from(error).code, "workspace_read_only");
    }

    #[test]
    fn classifies_known_readonly_raw_os_errors_as_workspace_read_only() {
        for raw in [13, 19, 30] {
            let error = std::io::Error::from_raw_os_error(raw);
            assert_eq!(
                AppError::from(error).code,
                "workspace_read_only",
                "raw OS error {raw} should classify as workspace_read_only"
            );
        }
    }

    #[test]
    fn classifies_storage_full_kind_as_disk_full() {
        let error = std::io::Error::new(std::io::ErrorKind::StorageFull, "full");
        assert_eq!(AppError::from(error).code, "disk_full");
    }

    #[test]
    fn classifies_known_disk_full_raw_os_errors_as_disk_full() {
        for raw in [28, 39, 112] {
            let error = std::io::Error::from_raw_os_error(raw);
            assert_eq!(
                AppError::from(error).code,
                "disk_full",
                "raw OS error {raw} should classify as disk_full"
            );
        }
    }

    #[test]
    fn an_unclassified_io_error_falls_back_to_io_error_code() {
        let error = std::io::Error::new(std::io::ErrorKind::NotFound, "missing");
        assert_eq!(AppError::from(error).code, "io_error");
    }

    // ── reqwest::Error classification ────────────────────────────────────
    //
    // `reqwest::Error` has no public constructor, so these classifications can only be proven
    // with a real (loopback-only) network condition that actually produces the specific error
    // kind `errors.rs` branches on — a hand-built value would prove nothing about whether the
    // real classification logic (`.is_timeout()`/`.is_connect()`/`.is_redirect()`) is correct.

    #[tokio::test]
    async fn classifies_a_connection_refusal_as_a_provider_unreachable_message() {
        // Nothing is listening on this freshly bound-then-dropped port, so the connection is
        // refused immediately — deterministic, no timeout needed.
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind to find a free port");
        let port = listener.local_addr().expect("local addr").port();
        drop(listener);

        let client = reqwest::Client::new();
        let error = client
            .get(format!("http://127.0.0.1:{port}/"))
            .send()
            .await
            .expect_err("nothing is listening on this port");
        assert!(
            error.is_connect(),
            "expected a connect-class reqwest error: {error:?}"
        );

        let app_error = AppError::from(error);
        assert_eq!(app_error.code, "provider_error");
        assert!(app_error.message.contains("unreachable"));
    }

    #[tokio::test]
    async fn classifies_an_exceeded_redirect_limit_as_redirect_blocked() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        use tokio::net::TcpListener;

        // A server that always redirects to itself — with a zero-redirect policy, reqwest's
        // very first redirect response trips its own limit and surfaces as `.is_redirect()`,
        // exercising the branch this codebase's actual providers never reach (they use
        // `redirect::Policy::none()`, which returns the redirect response itself rather than
        // erroring — see `providers::tests::ollama_client_does_not_follow_redirects`).
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind redirect server");
        let port = listener.local_addr().expect("local addr").port();
        tokio::spawn(async move {
            loop {
                let Ok((mut socket, _)) = listener.accept().await else {
                    return;
                };
                tokio::spawn(async move {
                    let mut buf = [0u8; 4096];
                    loop {
                        match socket.read(&mut buf).await {
                            Ok(0) | Err(_) => break,
                            Ok(n) if buf[..n].windows(4).any(|w| w == b"\r\n\r\n") => break,
                            Ok(_) => continue,
                        }
                    }
                    let response = format!(
                        "HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:{port}/\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                    );
                    let _ = socket.write_all(response.as_bytes()).await;
                });
            }
        });

        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::limited(0))
            .build()
            .expect("client builds");
        let error = client
            .get(format!("http://127.0.0.1:{port}/"))
            .send()
            .await
            .expect_err("a zero-redirect policy must error on the first redirect");
        assert!(
            error.is_redirect(),
            "expected a redirect-class reqwest error: {error:?}"
        );

        assert_eq!(AppError::from(error).code, "redirect_blocked");
    }
}
