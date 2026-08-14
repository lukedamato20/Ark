use crate::errors::AppError;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct Workspace {
    root: PathBuf,
    default_root: PathBuf,
    config_path: PathBuf,
    is_portable: bool,
}

impl Workspace {
    pub fn database_path(&self) -> PathBuf {
        self.root.join("ark.sqlite3")
    }

    pub fn info(&self) -> WorkspaceInfo {
        WorkspaceInfo {
            root_path: self.root.display().to_string(),
            database_path: self.database_path().display().to_string(),
            default_root_path: self.default_root.display().to_string(),
            config_path: self.config_path.display().to_string(),
            is_portable: self.is_portable,
            requires_restart: false,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceInfo {
    pub root_path: String,
    pub database_path: String,
    pub default_root_path: String,
    pub config_path: String,
    pub is_portable: bool,
    pub requires_restart: bool,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct WorkspaceConfig {
    workspace_root: Option<String>,
}

/// Resolves enough workspace metadata to render recovery UI even when the configured root is
/// missing, unwritable, or its configuration write was interrupted. Unlike
/// `resolve_default_workspace`, expected storage failures are returned alongside the selected
/// path instead of aborting Tauri setup and preventing the window from opening.
pub fn resolve_workspace_for_startup(
    app: &AppHandle,
) -> Result<(Workspace, Option<AppError>), AppError> {
    let default_root = default_workspace_root(app)?;
    let config_path = workspace_config_path(app)?;
    let (configured_root, config_error) = read_workspace_config_recoverably(&config_path);
    let root = configured_root.unwrap_or_else(|| default_root.clone());
    let is_portable = root != default_root;
    let workspace = Workspace {
        is_portable,
        root,
        default_root,
        config_path,
    };

    if let Some(error) = config_error {
        return Ok((workspace, Some(error)));
    }
    if workspace.is_portable && !workspace.root.exists() {
        return Ok((
            workspace,
            Some(AppError::new(
                "workspace_missing",
                "The configured workspace folder no longer exists. Restore or reconnect it, or choose a different workspace.",
            )),
        ));
    }
    if let Err(error) = prepare_workspace_root(&workspace.root) {
        return Ok((workspace, Some(error)));
    }

    Ok((workspace, None))
}

/// FTR-001's `copy_data` seeds the new location with a verified copy of the current workspace
/// database (via `backup::copy_workspace_data`) before repointing to it — "start empty" (the
/// pre-existing behavior) when `false`. Copying happens *before* the config is written, so a
/// failed copy leaves the current workspace selection completely unchanged.
pub fn set_workspace_root(
    app: &AppHandle,
    state: &crate::AppState,
    root: &str,
    copy_data: bool,
) -> Result<WorkspaceInfo, AppError> {
    // COR-008: centralized validator (also rejects embedded NUL bytes, which the previous
    // inline check here did not).
    let validated = crate::validation::validate_workspace_path(root)?;
    let path = PathBuf::from(validated);

    prepare_workspace_root(&path)?;
    if copy_data {
        crate::backup::copy_workspace_data(state, &path)?;
    }

    let default_root = default_workspace_root(app)?;
    let config_path = workspace_config_path(app)?;
    write_workspace_config(
        &config_path,
        WorkspaceConfig {
            workspace_root: Some(path.display().to_string()),
        },
    )?;

    Ok(WorkspaceInfo {
        root_path: path.display().to_string(),
        database_path: path.join("ark.sqlite3").display().to_string(),
        default_root_path: default_root.display().to_string(),
        config_path: config_path.display().to_string(),
        is_portable: path != default_root,
        requires_restart: true,
    })
}

pub fn reset_workspace_root(app: &AppHandle) -> Result<WorkspaceInfo, AppError> {
    let default_root = default_workspace_root(app)?;
    prepare_workspace_root(&default_root)?;

    let config_path = workspace_config_path(app)?;
    write_workspace_config(
        &config_path,
        WorkspaceConfig {
            workspace_root: None,
        },
    )?;

    Ok(WorkspaceInfo {
        root_path: default_root.display().to_string(),
        database_path: default_root.join("ark.sqlite3").display().to_string(),
        default_root_path: default_root.display().to_string(),
        config_path: config_path.display().to_string(),
        is_portable: false,
        requires_restart: true,
    })
}

fn default_workspace_root(app: &AppHandle) -> Result<PathBuf, AppError> {
    Ok(app
        .path()
        .app_data_dir()
        .map_err(|error| {
            AppError::new(
                "workspace_error",
                format!("Could not resolve app data directory: {error}"),
            )
        })?
        .join("workspace"))
}

fn workspace_config_path(app: &AppHandle) -> Result<PathBuf, AppError> {
    let config_dir = app.path().app_config_dir().map_err(|error| {
        AppError::new(
            "workspace_error",
            format!("Could not resolve app config directory: {error}"),
        )
    })?;
    Ok(config_dir.join("workspace.json"))
}

fn read_workspace_config(path: &Path) -> Result<Option<PathBuf>, AppError> {
    if !path.exists() {
        return Ok(None);
    }

    let content = fs::read_to_string(path).map_err(AppError::from)?;
    let config: WorkspaceConfig = serde_json::from_str(&content).map_err(|error| {
        AppError::new(
            "workspace_change_interrupted",
            format!(
                "The workspace selection file is incomplete or invalid ({error}). Ark left it untouched; choose a workspace to replace it safely."
            ),
        )
    })?;

    Ok(config
        .workspace_root
        .filter(|value| !value.trim().is_empty())
        .map(PathBuf::from))
}

fn config_previous_path(path: &Path) -> PathBuf {
    path.with_extension("json.previous")
}

fn config_next_path(path: &Path) -> PathBuf {
    path.with_extension("json.next")
}

fn read_workspace_config_recoverably(path: &Path) -> (Option<PathBuf>, Option<AppError>) {
    let previous = config_previous_path(path);
    let next = config_next_path(path);

    if !path.exists() && previous.exists() {
        return match read_workspace_config(&previous) {
            Ok(root) => (
                root,
                Some(AppError::new(
                    "workspace_change_interrupted",
                    "A workspace change was interrupted before the new selection was committed. Ark preserved the previous selection and did not modify either file.",
                )),
            ),
            Err(error) => (
                None,
                Some(AppError::new(
                    "workspace_change_interrupted",
                    format!(
                        "The active workspace selection is missing and the preserved previous selection could not be read ({}). Ark left both files untouched.",
                        error.message
                    ),
                )),
            ),
        };
    }

    match read_workspace_config(path) {
        Ok(root) if previous.exists() || next.exists() => (
            root,
            Some(AppError::new(
                "workspace_change_interrupted",
                "A complete workspace selection exists, but files from an interrupted change remain. Ark did not delete them; retry or choose a workspace after preserving anything you need.",
            )),
        ),
        Ok(root) => (root, None),
        Err(error) => (None, Some(error)),
    }
}

fn write_workspace_config(path: &Path, config: WorkspaceConfig) -> Result<(), AppError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let content = serde_json::to_string_pretty(&config).map_err(|error| {
        AppError::new(
            "workspace_error",
            format!("Could not serialize workspace config: {error}"),
        )
    })?;
    let next = config_next_path(path);
    let previous = config_previous_path(path);
    if next.exists() || previous.exists() {
        return Err(AppError::new(
            "workspace_change_interrupted",
            "Ark found files from an interrupted workspace change and will not overwrite them automatically.",
        ));
    }

    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&next)?;
    file.write_all(content.as_bytes())?;
    file.sync_all()?;
    drop(file);

    if path.exists() {
        fs::rename(path, &previous)?;
    }
    if let Err(error) = fs::rename(&next, path) {
        if previous.exists() {
            fs::rename(&previous, path).map_err(|restore_error| {
                AppError::new(
                    "workspace_change_interrupted",
                    format!(
                        "Could not commit the new workspace selection ({error}) or restore the previous selection ({restore_error}). Both files were preserved."
                    ),
                )
            })?;
        }
        return Err(AppError::new(
            "workspace_change_interrupted",
            format!("Could not commit the new workspace selection: {error}"),
        ));
    }
    if previous.exists() {
        fs::remove_file(&previous).map_err(|error| {
            AppError::new(
                "workspace_cleanup_failed",
                format!(
                    "The new workspace selection was committed, but the previous selection file could not be removed: {error}"
                ),
            )
        })?;
    }
    Ok(())
}

fn prepare_workspace_root(root: &Path) -> Result<(), AppError> {
    if root.exists() && !root.is_dir() {
        return Err(AppError::invalid_input(
            "Workspace path must be a directory.",
        ));
    }

    fs::create_dir_all(root)?;
    // Probe writability before hardening, not after: on Unix, chmod only requires ownership,
    // not existing write access, so hardening first would silently re-open a directory whose
    // owner (or a prior process) deliberately restricted it, defeating this exact check. Probing
    // first means a genuinely restricted directory is correctly rejected; hardening afterward
    // still leaves a successfully-probed directory at the intended least-privilege permissions.
    let probe_name = format!(".ark-probe-{}", Uuid::new_v4());
    probe_workspace_root(root, &probe_name)?;
    crate::file_permissions::harden_directory(root)?;
    Ok(())
}

fn probe_workspace_root(root: &Path, probe_name: &str) -> Result<(), AppError> {
    let probe = root.join(probe_name);
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&probe)
        .map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                return AppError::new(
                    "workspace_missing",
                    "The workspace folder disappeared while Ark was checking it.",
                );
            }
            let classified = AppError::from(error);
            AppError::new(
                if classified.code == "io_error" {
                    "workspace_error".to_string()
                } else {
                    classified.code
                },
                format!("Workspace is not writable: {}", classified.message),
            )
        })?;
    crate::file_permissions::harden_file(&probe)?;
    // A crash between create and removal can leave only a uniquely named zero-byte probe. It
    // never collides with the next run because names are UUIDs, and may be removed manually.
    // A live cleanup failure is surfaced instead of silently leaving a file behind.
    fs::remove_file(&probe).map_err(|error| {
        AppError::new(
            "workspace_cleanup_failed",
            format!(
                "Workspace is writable, but Ark could not remove its probe '{}': {error}",
                probe.display()
            ),
        )
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("ark-workspace-test-{name}-{}", Uuid::new_v4()))
    }

    #[test]
    fn creates_a_fresh_directory_and_leaves_no_probe_behind() {
        let root = temp_dir("fresh");
        assert!(!root.exists());

        prepare_workspace_root(&root).expect("prepares a fresh directory");

        assert!(root.is_dir());
        let leftover_probes: Vec<_> = fs::read_dir(&root)
            .expect("read created dir")
            .filter_map(|entry| entry.ok())
            .filter(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".ark-probe-")
            })
            .collect();
        assert!(
            leftover_probes.is_empty(),
            "the probe file must be cleaned up"
        );

        let _ = fs::remove_dir_all(&root);
    }

    /// COR-007's actual bug: the old implementation used a fixed `.ark-write-test` filename,
    /// so probing an existing workspace directory could silently overwrite and then delete a
    /// real file the user happened to have named that. The UUID-based probe with
    /// `create_new(true)` makes that structurally impossible — this test proves an existing
    /// file in the directory survives untouched.
    #[test]
    fn never_touches_existing_files_in_the_directory() {
        let root = temp_dir("existing-files");
        fs::create_dir_all(&root).expect("create dir");
        let user_file = root.join("my-important-notes.txt");
        fs::write(&user_file, b"do not touch me").expect("seed a user file");

        prepare_workspace_root(&root).expect("prepares an existing directory with user files");

        let contents = fs::read(&user_file).expect("user file must still exist");
        assert_eq!(
            contents, b"do not touch me",
            "user file content must be untouched"
        );

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn create_new_collision_fails_without_modifying_or_removing_the_existing_file() {
        let root = temp_dir("probe-collision");
        fs::create_dir_all(&root).expect("create dir");
        let probe_name = ".ark-probe-00000000-0000-0000-0000-000000000000";
        let collision = root.join(probe_name);
        fs::write(&collision, b"existing content").expect("seed collision");

        let error = probe_workspace_root(&root, probe_name).expect_err("create_new must fail");
        assert_eq!(error.code, "workspace_error");
        assert_eq!(
            fs::read(&collision).expect("collision survives"),
            b"existing content"
        );

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_directory_that_disappears_before_the_probe_returns_a_typed_error() {
        let root = temp_dir("disappearing");
        fs::create_dir_all(&root).expect("create dir");
        fs::remove_dir(&root).expect("remove empty dir");

        let error = probe_workspace_root(&root, ".ark-probe-test")
            .expect_err("probing a disappeared directory must fail");
        assert_eq!(error.code, "workspace_missing");
        assert!(!root.exists());
    }

    #[cfg(unix)]
    #[test]
    fn read_only_directory_is_rejected_without_leaving_a_probe() {
        use std::os::unix::fs::PermissionsExt;

        let root = temp_dir("read-only");
        fs::create_dir_all(&root).expect("create dir");
        fs::set_permissions(&root, fs::Permissions::from_mode(0o500)).expect("make read-only");
        let error = prepare_workspace_root(&root).expect_err("read-only directory must fail");
        assert_eq!(error.code, "workspace_read_only");
        assert!(fs::read_dir(&root)
            .expect("read directory")
            .next()
            .is_none());

        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).expect("restore permissions");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn is_safe_to_call_repeatedly_on_the_same_directory() {
        let root = temp_dir("repeated");
        for _ in 0..5 {
            prepare_workspace_root(&root).expect("repeated calls succeed");
        }
        assert!(root.is_dir());

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn workspace_config_replacement_is_complete_and_leaves_no_journal_files() {
        let root = temp_dir("config-atomic");
        fs::create_dir_all(&root).expect("create config dir");
        let config_path = root.join("workspace.json");

        write_workspace_config(
            &config_path,
            WorkspaceConfig {
                workspace_root: Some("C:\\first".to_string()),
            },
        )
        .expect("first config write");
        write_workspace_config(
            &config_path,
            WorkspaceConfig {
                workspace_root: Some("C:\\second".to_string()),
            },
        )
        .expect("replacement config write");

        assert_eq!(
            read_workspace_config(&config_path).expect("read config"),
            Some(PathBuf::from("C:\\second"))
        );
        assert!(!config_next_path(&config_path).exists());
        assert!(!config_previous_path(&config_path).exists());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn interrupted_workspace_change_preserves_and_reports_the_previous_selection() {
        let root = temp_dir("config-interrupted");
        fs::create_dir_all(&root).expect("create config dir");
        let config_path = root.join("workspace.json");
        let previous = config_previous_path(&config_path);
        fs::write(
            &previous,
            serde_json::to_vec(&WorkspaceConfig {
                workspace_root: Some("C:\\preserved".to_string()),
            })
            .expect("serialize fixture"),
        )
        .expect("seed interrupted previous file");

        let (selected, error) = read_workspace_config_recoverably(&config_path);
        assert_eq!(selected, Some(PathBuf::from("C:\\preserved")));
        assert_eq!(
            error.expect("recovery error").code,
            "workspace_change_interrupted"
        );
        assert!(
            previous.exists(),
            "recovery inspection must not repair/delete"
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn invalid_workspace_config_is_reported_without_modifying_the_original() {
        let root = temp_dir("config-invalid");
        fs::create_dir_all(&root).expect("create config dir");
        let config_path = root.join("workspace.json");
        let original = b"{ incomplete";
        fs::write(&config_path, original).expect("seed invalid config");

        let (selected, error) = read_workspace_config_recoverably(&config_path);
        assert!(selected.is_none());
        assert_eq!(
            error.expect("recovery error").code,
            "workspace_change_interrupted"
        );
        assert_eq!(fs::read(&config_path).expect("read original"), original);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn config_writer_refuses_to_overwrite_interrupted_change_artifacts() {
        let root = temp_dir("config-refuse-artifacts");
        fs::create_dir_all(&root).expect("create config dir");
        let config_path = root.join("workspace.json");
        fs::write(&config_path, b"{\"workspaceRoot\":null}").expect("seed current config");
        fs::write(config_next_path(&config_path), b"pending").expect("seed next artifact");

        let error = write_workspace_config(
            &config_path,
            WorkspaceConfig {
                workspace_root: Some("C:\\replacement".to_string()),
            },
        )
        .expect_err("must not overwrite interrupted state");
        assert_eq!(error.code, "workspace_change_interrupted");
        assert_eq!(
            fs::read(&config_path).expect("current remains"),
            b"{\"workspaceRoot\":null}"
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn rejects_a_path_that_already_exists_as_a_file() {
        let root = temp_dir("not-a-dir");
        if let Some(parent) = root.parent() {
            fs::create_dir_all(parent).ok();
        }
        fs::write(&root, b"i am a file, not a directory").expect("seed a colliding file");

        let error = prepare_workspace_root(&root).expect_err("must reject a non-directory path");
        assert_eq!(error.code, "invalid_input");

        let _ = fs::remove_file(&root);
    }
}
