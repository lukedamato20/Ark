//! PERF-001: a thin, opt-in layer over `observability::DiagnosticsLog` for numeric performance
//! measurements — durations, counts, and stable identifiers only, never prompt/response content.
//! Reuses the diagnostics log's existing bounded ring, best-effort file persistence, and
//! inclusion in the diagnostics bundle rather than building a second storage/export path: a
//! metric is just a `DiagnosticsLog::record` call at `category` `"perf.<surface>"`, with the
//! measurement fields rendered as `key=value` pairs by `format_metric`.
//!
//! Every call site gates on `DeviceSettings.perf_metrics_enabled`, read fresh (never cached) via
//! `device_settings::load_device_settings` — when the setting is off, nothing is measured or
//! recorded, matching the opt-in/local policy this module exists to satisfy.

use crate::observability::LogLevel;
use crate::AppState;
use tauri::AppHandle;

/// Renders `fields` as a single space-separated `key=value` message. Pure and independent of any
/// running app, so it's directly unit-testable — the same "factor out the decision/formatting
/// logic" precedent `device_settings::resolve_device_settings` and `generation::should_notify`
/// already established.
pub fn format_metric(fields: &[(&str, String)]) -> String {
    fields
        .iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Records one performance metric under `category` (by convention, `"perf.<surface>"` — e.g.
/// `"perf.generation"`, `"perf.cancellation"`) if and only if `perf_metrics_enabled` is currently
/// on. `state.observability_log` is the same log the diagnostics bundle already reads, so a
/// recorded metric is visible there with no further plumbing.
pub fn record_if_enabled(
    app: &AppHandle,
    state: &AppState,
    category: &str,
    correlation_id: Option<&str>,
    fields: &[(&str, String)],
) {
    let settings = crate::device_settings::load_device_settings(app, None);
    if !settings.perf_metrics_enabled {
        return;
    }
    let Ok(mut log) = state.observability_log.lock() else {
        return;
    };
    log.record(
        LogLevel::Info,
        category,
        correlation_id,
        &format_metric(fields),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_metric_joins_key_value_pairs_with_spaces() {
        let message = format_metric(&[
            ("ttft_ms", "42".to_string()),
            ("delta_count", "17".to_string()),
        ]);
        assert_eq!(message, "ttft_ms=42 delta_count=17");
    }

    #[test]
    fn format_metric_of_a_single_field_has_no_trailing_space() {
        let message = format_metric(&[("ack_ms", "8".to_string())]);
        assert_eq!(message, "ack_ms=8");
    }

    #[test]
    fn format_metric_of_no_fields_is_empty() {
        assert_eq!(format_metric(&[]), "");
    }
}
