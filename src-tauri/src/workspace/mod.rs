use crate::errors::AppError;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};

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

pub fn resolve_default_workspace(app: &AppHandle) -> Result<Workspace, AppError> {
    let default_root = default_workspace_root(app)?;
    let config_path = workspace_config_path(app)?;
    let configured_root = read_workspace_config(&config_path)?;
    let root = configured_root.unwrap_or_else(|| default_root.clone());
    prepare_workspace_root(&root)?;

    Ok(Workspace {
        is_portable: root != default_root,
        root,
        default_root,
        config_path,
    })
}

pub fn set_workspace_root(app: &AppHandle, root: &str) -> Result<WorkspaceInfo, AppError> {
    let path = PathBuf::from(root.trim());
    if root.trim().is_empty() {
        return Err(AppError::invalid_input("Workspace path cannot be empty."));
    }
    if !path.is_absolute() {
        return Err(AppError::invalid_input("Workspace path must be absolute."));
    }

    prepare_workspace_root(&path)?;

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
        .map_err(|error| AppError::new("workspace_error", format!("Could not resolve app data directory: {error}")))?
        .join("workspace"))
}

fn workspace_config_path(app: &AppHandle) -> Result<PathBuf, AppError> {
    let config_dir = app
        .path()
        .app_config_dir()
        .map_err(|error| AppError::new("workspace_error", format!("Could not resolve app config directory: {error}")))?;
    fs::create_dir_all(&config_dir)?;
    Ok(config_dir.join("workspace.json"))
}

fn read_workspace_config(path: &Path) -> Result<Option<PathBuf>, AppError> {
    if !path.exists() {
        return Ok(None);
    }

    let content = fs::read_to_string(path)?;
    let config: WorkspaceConfig = serde_json::from_str(&content)
        .map_err(|error| AppError::new("workspace_error", format!("Invalid workspace config: {error}")))?;

    Ok(config
        .workspace_root
        .filter(|value| !value.trim().is_empty())
        .map(PathBuf::from))
}

fn write_workspace_config(path: &Path, config: WorkspaceConfig) -> Result<(), AppError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let content = serde_json::to_string_pretty(&config)
        .map_err(|error| AppError::new("workspace_error", format!("Could not serialize workspace config: {error}")))?;
    fs::write(path, content)?;
    Ok(())
}

fn prepare_workspace_root(root: &Path) -> Result<(), AppError> {
    if root.exists() && !root.is_dir() {
        return Err(AppError::invalid_input("Workspace path must be a directory."));
    }

    fs::create_dir_all(root)?;
    let probe = root.join(".ark-write-test");
    fs::write(&probe, b"ok")?;
    fs::remove_file(probe)?;
    Ok(())
}
