//! ARC-006: the device-scoped settings store. Distinct from both the workspace SQLite database
//! (`app_settings` table — portable, tied to a specific workspace file, shared if that file is
//! moved to another machine) and the frontend's `localStorage` (browser-storage, cleared
//! trivially, not reachable from Rust): this is a small JSON file in the OS's per-user
//! application-config directory, the same on every workspace this installation ever opens.
//!
//! Theme and the built-in runtime's model path are genuinely device properties — a display
//! preference and a path into *this machine's* filesystem, neither of which has a sensible
//! meaning carried into a different workspace file or a different computer — so persisting them
//! into the portable workspace database (as the old `appearance.theme` SQLite setting did) was
//! the actual bug ARC-006's settings-ownership audit found. See `docs/settings-catalog.md`.

use crate::errors::AppError;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceSettings {
    pub theme: String,
    /// Absolute path to the last GGUF model file selected for the built-in runtime on this
    /// device. `None` until the user starts the built-in runtime at least once.
    pub built_in_model_path: Option<String>,
    /// OPS-001: opt-in, off by default. When true, an uncaught panic is recorded (redacted) to
    /// the local diagnostics log file so it can be included in a diagnostics bundle after
    /// restart — never transmitted anywhere automatically; export is always a separate,
    /// reviewed, user-initiated action. `#[serde(default)]` so a device settings file saved
    /// before this field existed still parses instead of falling back to the legacy-seed path.
    #[serde(default)]
    pub crash_capture_enabled: bool,
    /// CMP-006: opt-in, off by default. When true, a generation that completes, fails, or is
    /// interrupted while the main window is unfocused shows a generic native OS notification —
    /// never the conversation title or any response content. `#[serde(default)]` for the same
    /// back-compat reason as `crash_capture_enabled`.
    #[serde(default)]
    pub completion_notifications_enabled: bool,
    /// PERF-001: opt-in, off by default. When true, `perf_metrics::record_if_enabled` writes
    /// local performance measurements (durations/counts/identifiers only — see that module's
    /// doc) into the same diagnostics log `crash_capture_enabled` already gates; when false,
    /// nothing is measured or recorded anywhere. `#[serde(default)]` for the same back-compat
    /// reason as the two fields above.
    #[serde(default)]
    pub perf_metrics_enabled: bool,
}

impl Default for DeviceSettings {
    fn default() -> Self {
        Self {
            theme: "dark".to_string(),
            built_in_model_path: None,
            crash_capture_enabled: false,
            completion_notifications_enabled: false,
            perf_metrics_enabled: false,
        }
    }
}

fn device_settings_path(app: &AppHandle) -> Result<PathBuf, AppError> {
    app.path()
        .app_config_dir()
        .map(|dir| dir.join("device_settings.json"))
        .map_err(|error| {
            AppError::new(
                "device_settings_path_unavailable",
                format!("Could not resolve the device settings directory: {error}"),
            )
        })
}

/// Reads the device settings file, if one exists and parses. On first run under this mechanism
/// (no file yet) — or a corrupt/unreadable file, treated the same as absent rather than a fatal
/// startup error, matching COR-010's "never block startup over a durability nicety" precedent —
/// `legacy_theme_seed` (the old workspace-scoped `appearance.theme` SQLite setting, if this
/// workspace has one) seeds the initial theme once, so a user who already had a saved preference
/// doesn't see it silently reset to the hardcoded default the first time they open Ark after
/// this change. Every read after the first, from any workspace, is served entirely from this
/// device-scoped file and never touches SQLite again.
pub fn load_device_settings(app: &AppHandle, legacy_theme_seed: Option<&str>) -> DeviceSettings {
    let Ok(path) = device_settings_path(app) else {
        return DeviceSettings::default();
    };
    resolve_device_settings(
        std::fs::read_to_string(&path).ok().as_deref(),
        legacy_theme_seed,
    )
}

/// The actual decision logic behind `load_device_settings`, factored out so it can be
/// unit-tested without a running Tauri app (`AppHandle` can't be constructed otherwise): given
/// the device settings file's raw content (`None` if it doesn't exist or couldn't be read) and
/// the legacy SQLite-seed value, decide what `DeviceSettings` to use. A file that exists and
/// parses always wins outright — the legacy seed is consulted only on a genuinely first run.
fn resolve_device_settings(
    raw_file_content: Option<&str>,
    legacy_theme_seed: Option<&str>,
) -> DeviceSettings {
    if let Some(raw) = raw_file_content {
        if let Ok(settings) = serde_json::from_str::<DeviceSettings>(raw) {
            return settings;
        }
    }

    let mut settings = DeviceSettings::default();
    if matches!(legacy_theme_seed, Some("dark") | Some("light")) {
        settings.theme = legacy_theme_seed.expect("matched Some above").to_string();
    }
    settings
}

pub fn save_device_settings(app: &AppHandle, settings: &DeviceSettings) -> Result<(), AppError> {
    let path = device_settings_path(app)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| {
            AppError::new(
                "device_settings_write_failed",
                format!("Could not create {}: {error}", parent.display()),
            )
        })?;
    }
    let json = serde_json::to_string_pretty(settings).map_err(|error| {
        AppError::new(
            "device_settings_write_failed",
            format!("Could not serialize device settings: {error}"),
        )
    })?;
    std::fs::write(&path, json).map_err(|error| {
        AppError::new(
            "device_settings_write_failed",
            format!("Could not write {}: {error}", path.display()),
        )
    })?;
    Ok(())
}

/// Validates and persists a full replacement `DeviceSettings` value — the frontend always sends
/// its complete, already-merged current state (see `ArkClient.updateDeviceSettings`), so there
/// is no partial-update/merge logic to get right here.
pub fn update_device_settings(
    app: &AppHandle,
    settings: DeviceSettings,
) -> Result<DeviceSettings, AppError> {
    if settings.theme != "dark" && settings.theme != "light" {
        return Err(AppError::invalid_input("Theme must be dark or light."));
    }
    save_device_settings(app, &settings)?;
    Ok(settings)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `AppHandle` can't be constructed without a running Tauri app, so these tests exercise
    /// the pure, app-handle-independent logic directly — the same standard already established
    /// for `diagnostics::performance_guidance`.
    #[test]
    fn default_device_settings_use_dark_theme_and_no_model_path() {
        let settings = DeviceSettings::default();
        assert_eq!(settings.theme, "dark");
        assert_eq!(settings.built_in_model_path, None);
    }

    #[test]
    fn device_settings_round_trip_through_json() {
        let settings = DeviceSettings {
            theme: "light".to_string(),
            built_in_model_path: Some("C:\\models\\model.gguf".to_string()),
            crash_capture_enabled: true,
            completion_notifications_enabled: true,
            perf_metrics_enabled: true,
        };
        let json = serde_json::to_string(&settings).expect("serializes");
        let parsed: DeviceSettings = serde_json::from_str(&json).expect("deserializes");
        assert_eq!(parsed.theme, "light");
        assert_eq!(
            parsed.built_in_model_path.as_deref(),
            Some("C:\\models\\model.gguf")
        );
        assert!(parsed.crash_capture_enabled);
        assert!(parsed.completion_notifications_enabled);
        assert!(parsed.perf_metrics_enabled);
    }

    #[test]
    fn device_settings_json_uses_camel_case_field_names() {
        let settings = DeviceSettings {
            theme: "dark".to_string(),
            built_in_model_path: Some("model.gguf".to_string()),
            crash_capture_enabled: false,
            completion_notifications_enabled: false,
            perf_metrics_enabled: false,
        };
        let json = serde_json::to_string(&settings).expect("serializes");
        assert!(
            json.contains("\"builtInModelPath\""),
            "expected camelCase field name in: {json}"
        );
        assert!(
            json.contains("\"completionNotificationsEnabled\""),
            "expected camelCase field name in: {json}"
        );
        assert!(
            json.contains("\"perfMetricsEnabled\""),
            "expected camelCase field name in: {json}"
        );
    }

    // ── ARC-006: resolve_device_settings decision logic ──────────────────────

    #[test]
    fn resolve_prefers_an_existing_valid_file_over_the_legacy_seed() {
        let raw = r#"{"theme":"light","builtInModelPath":"D:\\models\\other.gguf"}"#;
        let settings = resolve_device_settings(Some(raw), Some("dark"));
        assert_eq!(settings.theme, "light");
        assert_eq!(
            settings.built_in_model_path.as_deref(),
            Some("D:\\models\\other.gguf")
        );
        // OPS-001: this raw JSON predates `crashCaptureEnabled` — proves a device settings file
        // saved before that field existed still parses (via #[serde(default)]) instead of being
        // treated as corrupt and falling back to the legacy-seed path.
        assert!(!settings.crash_capture_enabled);
    }

    /// ARC-006 acceptance: "Legacy localStorage/DB values migrate deterministically." On a
    /// genuine first run (no device settings file yet), a workspace's old `appearance.theme`
    /// SQLite value seeds the initial theme rather than silently resetting to the hardcoded
    /// default.
    #[test]
    fn resolve_falls_back_to_the_legacy_theme_seed_when_no_file_exists() {
        let settings = resolve_device_settings(None, Some("light"));
        assert_eq!(settings.theme, "light");
        assert_eq!(settings.built_in_model_path, None);
    }

    #[test]
    fn resolve_falls_back_to_the_legacy_theme_seed_when_the_file_is_corrupt() {
        let settings = resolve_device_settings(Some("not valid json"), Some("light"));
        assert_eq!(settings.theme, "light");
    }

    #[test]
    fn resolve_uses_the_hardcoded_default_when_neither_a_file_nor_a_legacy_seed_exists() {
        let settings = resolve_device_settings(None, None);
        assert_eq!(settings.theme, "dark");
        assert_eq!(settings.built_in_model_path, None);
    }

    #[test]
    fn resolve_ignores_a_legacy_seed_that_is_not_a_recognized_theme_value() {
        // Defensive: an unexpected/corrupted legacy SQLite value must not propagate into the
        // new store as-is — only "dark"/"light" are ever accepted.
        let settings = resolve_device_settings(None, Some("not-a-real-theme"));
        assert_eq!(settings.theme, "dark");
    }
}
