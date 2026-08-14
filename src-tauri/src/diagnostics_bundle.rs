//! OPS-001: assembles a single reviewable text bundle from already-redacted sources — a
//! hardware snapshot, the sidecar's runtime diagnostics/logs (redacted by `sidecar.rs`), and
//! the structured app log (redacted by `observability.rs`/`redaction.rs`) — for local support
//! use. `save_diagnostics_bundle` writes back exactly the text the frontend was given, so what
//! gets saved can never differ from what the user reviewed before saving — no second assembly
//! step that could drift from the preview.
//!
//! Deliberately excluded: prompt/message content, attachment text, and the workspace's actual
//! absolute path (shown redacted — see `redaction::redact`'s path handling — since even a
//! default-looking path can reveal an OS username). None of those have a place in a bundle
//! meant to be handed to another person for support.

use crate::errors::AppError;
use crate::redaction::redact;
use crate::AppState;
use serde::Serialize;
use sysinfo::System;

const MAX_RUNTIME_LOG_LINES: usize = 50;
const MAX_APP_LOG_LINES: usize = 100;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticsBundle {
    pub generated_at: String,
    pub preview_text: String,
}

pub fn build_diagnostics_bundle(state: &AppState) -> Result<DiagnosticsBundle, AppError> {
    let generated_at = crate::db::now();

    let mut system = System::new_all();
    system.refresh_all();
    let os = format!(
        "{} {}",
        System::name().unwrap_or_else(|| "Unknown OS".to_string()),
        System::long_os_version().unwrap_or_default()
    )
    .trim()
    .to_string();
    let cpu = system
        .cpus()
        .first()
        .map(|cpu| cpu.brand().to_string())
        .unwrap_or_else(|| "Unknown CPU".to_string());

    let workspace_root_redacted = {
        let workspace_info = state
            .workspace
            .lock()
            .map_err(|_| AppError::new("state_error", "Could not access workspace state."))?
            .clone();
        redact(&workspace_info.root_path, &[])
    };

    let runtime = crate::commands::lock_sidecar(state)?.diagnostics(true);
    let runtime_logs: Vec<String> = runtime
        .recent_logs
        .iter()
        .rev()
        .take(MAX_RUNTIME_LOG_LINES)
        .map(|entry| {
            format!(
                "{} [{}] {}",
                entry.timestamp_ms, entry.stream, entry.message
            )
        })
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();

    let app_log_lines = {
        let log = state
            .observability_log
            .lock()
            .map_err(|_| AppError::new("state_error", "Could not access the diagnostics log."))?;
        let mut lines: Vec<String> = log
            .recent(MAX_APP_LOG_LINES)
            .iter()
            .map(|record| {
                format!(
                    "{} [{}] {} ({}) {}",
                    record.timestamp_ms,
                    format!("{:?}", record.level).to_lowercase(),
                    record.category,
                    record.correlation_id.as_deref().unwrap_or("-"),
                    record.message
                )
            })
            .collect();
        // The in-memory ring resets every restart; the file tail can still hold a crash record
        // (or any other event) from a session that ended before this one started. Both are
        // already-redacted lines, so they're safe to fold together for the preview.
        if let Some(path) = log.file_path() {
            let file_lines = crate::observability::read_recent_file_lines(&path, MAX_APP_LOG_LINES);
            for line in file_lines {
                if !lines.contains(&line) {
                    lines.push(line);
                }
            }
        }
        lines
    };

    let mut preview_text = String::new();
    preview_text.push_str(&format!(
        "Ark diagnostics bundle — generated {generated_at}\n"
    ));
    preview_text.push_str(&format!("App version: {}\n", env!("CARGO_PKG_VERSION")));
    preview_text.push_str(&format!("OS: {os}\n"));
    preview_text.push_str(&format!("CPU: {cpu} ({} cores)\n", system.cpus().len()));
    preview_text.push_str(&format!(
        "Memory: {} / {} bytes available\n",
        system.available_memory(),
        system.total_memory()
    ));
    preview_text.push_str(&format!("Workspace location: {workspace_root_redacted}\n"));
    preview_text.push_str(&format!(
        "\n-- Managed runtime --\nState: {:?}\nPID present: {}\nPort: {}\nFailure: {}\n",
        runtime.state,
        runtime.pid.is_some(),
        runtime
            .port
            .map(|port| port.to_string())
            .unwrap_or_else(|| "none".to_string()),
        runtime
            .failure
            .as_ref()
            .map(|failure| format!("{:?}: {}", failure.category, failure.message))
            .unwrap_or_else(|| "none".to_string()),
    ));
    preview_text.push_str("\n-- Recent runtime log lines --\n");
    if runtime_logs.is_empty() {
        preview_text.push_str("(none)\n");
    } else {
        for line in &runtime_logs {
            preview_text.push_str(line);
            preview_text.push('\n');
        }
    }
    preview_text.push_str("\n-- Recent app log lines --\n");
    if app_log_lines.is_empty() {
        preview_text.push_str("(none)\n");
    } else {
        for line in &app_log_lines {
            preview_text.push_str(line);
            preview_text.push('\n');
        }
    }

    Ok(DiagnosticsBundle {
        generated_at,
        preview_text,
    })
}

/// Writes exactly `bundle_text` to `destination_path` — no re-assembly, so what was reviewed is
/// byte-for-byte what gets saved. `destination_path` is a user-chosen save location (via the
/// frontend's save dialog), not an Ark-managed path, so no directory-creation/hardening happens
/// here — the same trust boundary as any other "export to a file the user picked" command
/// (`export_conversation_json`/`export_conversation_markdown`).
pub fn save_diagnostics_bundle(destination_path: &str, bundle_text: &str) -> Result<(), AppError> {
    std::fs::write(destination_path, bundle_text).map_err(|error| {
        AppError::new(
            "diagnostics_bundle_save_failed",
            format!("Could not save the diagnostics bundle: {error}"),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_writes_exactly_the_reviewed_text_and_nothing_else() {
        let path = std::env::temp_dir().join(format!(
            "ark-diagnostics-bundle-{}.txt",
            uuid::Uuid::new_v4()
        ));
        let text = "line one\nline two\n";

        save_diagnostics_bundle(path.to_str().expect("utf8 path"), text).expect("save succeeds");

        let saved = std::fs::read_to_string(&path).expect("file was written");
        assert_eq!(saved, text);

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn save_reports_a_typed_error_for_an_unwritable_destination() {
        // A path inside a directory that does not exist — `std::fs::write` never creates parent
        // directories on any platform, so this fails deterministically everywhere, unlike a
        // platform-specific "bad path" string.
        let missing_dir =
            std::env::temp_dir().join(format!("ark-missing-dir-{}", uuid::Uuid::new_v4()));
        let path = missing_dir.join("bundle.txt");

        let error = save_diagnostics_bundle(path.to_str().expect("utf8 path"), "content")
            .expect_err("a destination inside a nonexistent directory must fail");
        assert_eq!(error.code, "diagnostics_bundle_save_failed");
    }
}
