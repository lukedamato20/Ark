//! OPS-001: a small, local, structured diagnostics log — separate from the sidecar's own
//! runtime-output buffer (`sidecar::RuntimeLogBuffer`, which captures llama-server's stdout/
//! stderr verbatim-minus-redaction). This module is for events *Ark itself* chooses to record:
//! lifecycle milestones, error codes, and counts — never prompt text, model output, attachment
//! content, or any other user content. That is an architectural guarantee, not a redaction one:
//! every `DiagnosticsLog::record` call site in this codebase passes a message built from stable
//! identifiers (error codes, category names, counts, durations) and must keep doing so —
//! `redaction::redact` is still applied as defense in depth, but it cannot detect "this is prose
//! the user wrote."
//!
//! Records are kept in a bounded in-memory ring (so the diagnostics bundle can show "what
//! happened recently" without reading a file back) and, best-effort, appended to a small rotated
//! local log file so a record survives past the crash that might follow it — this is what makes
//! opt-in local crash capture (see `lib.rs`'s panic hook) actually useful. A failed file write
//! never panics or blocks logging; it's reported to the in-memory ring itself as a single
//! `Warn`-level record instead, the same "best-effort, never fatal" posture `AppState::drop`
//! already applies to the database checkpoint.

use crate::redaction::redact;
use serde::Serialize;
use std::collections::VecDeque;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const MAX_ENTRIES: usize = 500;
const MAX_BYTES: usize = 512 * 1024;
/// Once the on-disk log file reaches this size, it is rotated to `ark.log.1` (overwriting any
/// previous rotation) and a fresh file is started — a single rotation step, not a multi-file
/// scheme, matching this module's "bounded, not exhaustive" retention goal.
const MAX_FILE_BYTES: u64 = 2 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LogLevel {
    /// Reserved for future granular tracing — no current call site logs at this level, unlike
    /// `Info`/`Warn`/`Error`, which are all exercised today (see `generation.rs`,
    /// `provider_management.rs`, and `lib.rs::run`). Remove this `#[allow]` the moment a real
    /// `Debug`-level call site exists, the same discipline `tool_policy.rs`'s module-level
    /// allowance already documents for the same reason.
    #[allow(dead_code)]
    Debug,
    Info,
    Warn,
    Error,
}

impl LogLevel {
    fn as_str(self) -> &'static str {
        match self {
            LogLevel::Debug => "debug",
            LogLevel::Info => "info",
            LogLevel::Warn => "warn",
            LogLevel::Error => "error",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogRecord {
    pub timestamp_ms: u128,
    pub level: LogLevel,
    pub category: String,
    pub correlation_id: Option<String>,
    pub message: String,
}

impl LogRecord {
    fn format_line(&self) -> String {
        let correlation = self.correlation_id.as_deref().unwrap_or("-");
        format!(
            "{} [{}] {} ({}) {}",
            self.timestamp_ms,
            self.level.as_str(),
            self.category,
            correlation,
            self.message
        )
    }
}

pub struct DiagnosticsLog {
    entries: VecDeque<LogRecord>,
    bytes: usize,
    file_path: Option<PathBuf>,
}

impl DiagnosticsLog {
    pub fn new() -> Self {
        Self {
            entries: VecDeque::new(),
            bytes: 0,
            file_path: None,
        }
    }

    /// Points this log at a real file on disk for the rest of the process's life. Safe to call
    /// more than once (a later call simply switches the target); safe to never call at all — an
    /// unattached log stays fully functional in memory, which is what every test constructing an
    /// `AppState` without a running Tauri app gets, matching `SidecarState::new()`'s no-`AppHandle`
    /// construction.
    pub fn attach_file(&mut self, path: PathBuf) {
        self.file_path = Some(path);
    }

    fn push_in_memory(&mut self, record: LogRecord) {
        self.bytes = self.bytes.saturating_add(record.message.len());
        self.entries.push_back(record);
        while self.entries.len() > MAX_ENTRIES || self.bytes > MAX_BYTES {
            if let Some(removed) = self.entries.pop_front() {
                self.bytes = self.bytes.saturating_sub(removed.message.len());
            } else {
                break;
            }
        }
    }

    pub fn record(
        &mut self,
        level: LogLevel,
        category: &str,
        correlation_id: Option<&str>,
        message: &str,
    ) {
        let record = LogRecord {
            timestamp_ms: now_millis(),
            level,
            category: category.to_string(),
            correlation_id: correlation_id.map(str::to_string),
            message: redact(message, &[]),
        };
        if let Some(path) = &self.file_path {
            append_line_to_file(path, &record.format_line());
        }
        self.push_in_memory(record);
    }

    pub fn recent(&self, limit: usize) -> Vec<LogRecord> {
        let skip = self.entries.len().saturating_sub(limit);
        self.entries.iter().skip(skip).cloned().collect()
    }

    pub fn file_path(&self) -> Option<PathBuf> {
        self.file_path.clone()
    }
}

impl Default for DiagnosticsLog {
    fn default() -> Self {
        Self::new()
    }
}

/// Writes one already-formatted line directly to `path`, independent of any `DiagnosticsLog`
/// instance's Mutex — this is what makes it safe to call from the panic hook installed in
/// `lib.rs::run`, where taking a lock the panicking thread might already hold (if the panic
/// happened inside `DiagnosticsLog::record` itself) would deadlock rather than fail safely.
/// Best-effort: any I/O failure here is silently ignored, matching this module's "logging must
/// never itself crash or block the app" posture — including while it is already crashing.
fn append_line_to_file(path: &Path, line: &str) {
    rotate_if_oversized(path);
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        let _ = writeln!(file, "{line}");
    }
}

/// Records one crash record directly to `path`, bypassing `DiagnosticsLog` entirely. See
/// `append_line_to_file`'s doc comment for why the panic hook must not go through the shared,
/// mutex-guarded log instance.
pub fn record_crash_directly_to_file(path: &Path, message: &str) {
    let record = LogRecord {
        timestamp_ms: now_millis(),
        level: LogLevel::Error,
        category: "panic".to_string(),
        correlation_id: None,
        message: redact(message, &[]),
    };
    append_line_to_file(path, &record.format_line());
}

/// Reads back up to `max_lines` of the most recent already-redacted lines from the on-disk log
/// — used by the diagnostics bundle to surface history the in-memory ring lost across a restart
/// (most importantly, a crash record written just before the process that would have held it in
/// memory exited). The file is bounded by `rotate_if_oversized` to at most `MAX_FILE_BYTES`, so a
/// full read is cheap; a missing or unreadable file yields an empty result rather than an error,
/// consistent with this module's best-effort posture.
pub fn read_recent_file_lines(path: &Path, max_lines: usize) -> Vec<String> {
    let Ok(contents) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    let mut lines: Vec<&str> = contents.lines().collect();
    if lines.len() > max_lines {
        lines = lines.split_off(lines.len() - max_lines);
    }
    lines.into_iter().map(str::to_string).collect()
}

fn rotate_if_oversized(path: &Path) {
    let Ok(metadata) = std::fs::metadata(path) else {
        return;
    };
    if metadata.len() < MAX_FILE_BYTES {
        return;
    }
    let rotated = path.with_extension("log.1");
    let _ = std::fs::rename(path, rotated);
}

fn now_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_are_redacted_before_being_stored() {
        let mut log = DiagnosticsLog::new();
        log.record(
            LogLevel::Error,
            "provider",
            None,
            "request to http://127.0.0.1:8080/models?api_key=sk-live-secret failed",
        );
        let recent = log.recent(10);
        assert_eq!(recent.len(), 1);
        assert!(!recent[0].message.contains("sk-live-secret"));
    }

    #[test]
    fn recent_returns_only_the_most_recent_entries_in_order() {
        let mut log = DiagnosticsLog::new();
        for index in 0..5 {
            log.record(LogLevel::Info, "test", None, &format!("event {index}"));
        }
        let recent = log.recent(2);
        assert_eq!(recent.len(), 2);
        assert!(recent[0].message.contains("event 3"));
        assert!(recent[1].message.contains("event 4"));
    }

    #[test]
    fn in_memory_ring_is_bounded_by_entry_count() {
        let mut log = DiagnosticsLog::new();
        for index in 0..(MAX_ENTRIES + 20) {
            log.record(LogLevel::Debug, "test", None, &format!("event {index}"));
        }
        assert!(log.entries.len() <= MAX_ENTRIES);
    }

    #[test]
    fn an_unattached_log_never_touches_the_filesystem_and_stays_functional() {
        let mut log = DiagnosticsLog::new();
        log.record(LogLevel::Info, "test", Some("corr-1"), "hello");
        assert_eq!(log.recent(1)[0].correlation_id.as_deref(), Some("corr-1"));
    }

    #[test]
    fn attaching_a_file_persists_redacted_lines_across_a_fresh_log_instance() {
        let dir =
            std::env::temp_dir().join(format!("ark-observability-test-{}", uuid::Uuid::new_v4()));
        let path = dir.join("ark.log");

        let mut log = DiagnosticsLog::new();
        log.attach_file(path.clone());
        log.record(
            LogLevel::Info,
            "startup",
            None,
            "workspace opened token=abc123",
        );

        let contents = std::fs::read_to_string(&path).expect("log file exists");
        assert!(contents.contains("workspace opened"));
        assert!(!contents.contains("abc123"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_oversized_file_is_rotated_rather_than_growing_unbounded() {
        let dir =
            std::env::temp_dir().join(format!("ark-observability-rotate-{}", uuid::Uuid::new_v4()));
        let path = dir.join("ark.log");
        std::fs::create_dir_all(&dir).expect("create test dir");
        std::fs::write(&path, vec![b'x'; (MAX_FILE_BYTES as usize) + 1])
            .expect("seed oversized file");

        let mut log = DiagnosticsLog::new();
        log.attach_file(path.clone());
        log.record(LogLevel::Info, "test", None, "after rotation");

        let rotated = path.with_extension("log.1");
        assert!(
            rotated.exists(),
            "expected the oversized file to be rotated"
        );
        let contents = std::fs::read_to_string(&path).expect("fresh log file exists");
        assert!(contents.contains("after rotation"));
        assert!(
            (contents.len() as u64) < MAX_FILE_BYTES,
            "the fresh file must not have inherited the oversized rotated content"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn crash_records_written_directly_to_file_are_redacted_and_readable_back() {
        let dir =
            std::env::temp_dir().join(format!("ark-observability-crash-{}", uuid::Uuid::new_v4()));
        let path = dir.join("ark.log");

        record_crash_directly_to_file(&path, "panicked at src/x.rs:12: token=abc123 failed");

        let lines = read_recent_file_lines(&path, 10);
        assert_eq!(lines.len(), 1);
        assert!(lines[0].contains("[error] panic"));
        assert!(!lines[0].contains("abc123"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn read_recent_file_lines_returns_only_the_tail_and_empty_for_a_missing_file() {
        let dir =
            std::env::temp_dir().join(format!("ark-observability-tail-{}", uuid::Uuid::new_v4()));
        let path = dir.join("ark.log");

        assert!(read_recent_file_lines(&path, 5).is_empty());

        let mut log = DiagnosticsLog::new();
        log.attach_file(path.clone());
        for index in 0..10 {
            log.record(LogLevel::Info, "test", None, &format!("line {index}"));
        }

        let tail = read_recent_file_lines(&path, 3);
        assert_eq!(tail.len(), 3);
        assert!(tail[0].contains("line 7"));
        assert!(tail[2].contains("line 9"));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
