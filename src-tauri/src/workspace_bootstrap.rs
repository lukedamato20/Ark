//! ARC-001: the startup bootstrap and workspace-recovery application workflow, extracted from
//! `commands::mod`. `get_app_bootstrap` assembles the full read-model the frontend needs on
//! launch (conversations, providers, models, theme, workspace info, any startup error) from
//! several database reads plus `AppState`; `retry_workspace_open` layers real branching recovery
//! logic on top (swap the in-memory fallback database for the real one on success, record the
//! error on failure) — both are genuine orchestration, not command-handling plumbing, which is
//! why they live here rather than in `crate::workspace` (which owns filesystem/path concerns,
//! not database or `AppState` composition).
//!
//! `set_workspace`/`reset_workspace` deliberately stay as one-line delegations in
//! `commands::mod`: they already call straight into `crate::workspace` with no orchestration of
//! their own, so moving them here would add a layer without separating any concern.

use crate::chat::{ConversationListRequest, ConversationPage};
use crate::device_settings::DeviceSettings;
use crate::errors::AppError;
use crate::providers::{ModelInfo, ProviderConfig};
use crate::workspace::WorkspaceInfo;
use crate::AppState;
use serde::Serialize;
use tauri::AppHandle;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppBootstrap {
    pub conversation_page: ConversationPage,
    pub providers: Vec<ProviderConfig>,
    pub models: Vec<ModelInfo>,
    pub workspace_path: String,
    pub workspace: WorkspaceInfo,
    /// ARC-006: device-scoped (theme, built-in runtime model path) — see
    /// `crate::device_settings` and `docs/settings-catalog.md`. Replaced the old workspace-
    /// scoped `theme: String` field (backed by the SQLite `appearance.theme` setting), which was
    /// the settings-ownership bug this item exists to fix: a purely visual, per-device
    /// preference had no business being portable/synced with the workspace file.
    pub device_settings: DeviceSettings,
    /// COR-010: present when the real workspace database failed to open at startup and this
    /// response actually reflects a temporary in-memory fallback — nothing shown will be
    /// saved until the user retries or switches workspace. See `AppState::workspace_open_error`.
    pub workspace_open_error: Option<AppError>,
}

pub fn get_app_bootstrap(app: &AppHandle, state: &AppState) -> Result<AppBootstrap, AppError> {
    let workspace_info = state
        .workspace
        .lock()
        .map_err(|_| AppError::new("state_error", "Could not access workspace state."))?
        .clone();
    let db = crate::commands::lock_db(state)?;
    db.recover_stale_messages()?;
    let providers = db.list_providers()?;
    let models = db.list_all_models()?;

    // ARC-006: `appearance.theme` is the pre-ARC-006 workspace-scoped setting this device
    // settings file replaces — read here only as a one-time migration seed (see
    // `device_settings::load_device_settings`'s doc comment), never written to again. The row
    // itself is left in place rather than deleted; a setting nothing reads is harmless, and
    // deleting historical data as a side effect of an unrelated bootstrap call is not a trade
    // worth making for a few unused bytes.
    let legacy_theme_seed = db.get_setting("appearance.theme")?;
    let device_settings =
        crate::device_settings::load_device_settings(app, legacy_theme_seed.as_deref());
    // Persist immediately so the device settings file deterministically exists after the first
    // bootstrap, whether or not the user ever changes anything from the hardcoded/migrated
    // default — later reads never need to fall back to the legacy seed again.
    crate::device_settings::save_device_settings(app, &device_settings)?;

    let workspace_open_error = state
        .workspace_open_error
        .lock()
        .map_err(|_| AppError::new("state_error", "Could not access recovery state."))?
        .clone();

    Ok(AppBootstrap {
        conversation_page: db.list_conversations_page(&ConversationListRequest {
            limit: None,
            cursor: None,
            query: None,
            archived: Some(false),
            project_id: None,
        })?,
        providers,
        models,
        workspace_path: workspace_info.database_path.clone(),
        workspace: workspace_info,
        device_settings,
        workspace_open_error,
    })
}

/// COR-010: re-attempts opening the real workspace database, replacing the in-memory
/// fallback if it succeeds. This is the "retry" recovery action for a transient failure
/// (e.g. another Ark instance was holding the lock and has since closed) — no full app
/// restart required. If it still fails, the error is recorded and surfaced the same way as
/// the initial startup failure, and the fallback in-memory database remains in place so the
/// app stays usable.
pub fn retry_workspace_open(app: &AppHandle, state: &AppState) -> Result<AppBootstrap, AppError> {
    let (workspace, workspace_resolution_error) =
        crate::workspace::resolve_workspace_for_startup(app)?;
    *state
        .workspace
        .lock()
        .map_err(|_| AppError::new("state_error", "Could not access workspace state."))? =
        workspace.info();

    if let Some(error) = workspace_resolution_error {
        *state
            .workspace_open_error
            .lock()
            .map_err(|_| AppError::new("state_error", "Could not access recovery state."))? =
            Some(error);
        return get_app_bootstrap(app, state);
    }

    // ARC-004: writer and read replica are opened as a pair and swapped in together — see
    // `crate::open_database_pair` — so `db`/`read_db` can never end up pointing at different
    // underlying files/fallback states.
    match crate::open_database_pair(&workspace.database_path()) {
        Ok((new_db, new_read_db)) => {
            {
                let mut db_guard = crate::commands::lock_db(state)?;
                *db_guard = new_db;
            }
            {
                let mut read_db_guard = crate::commands::lock_read_db(state)?;
                *read_db_guard = new_read_db;
            }
            *state
                .workspace_open_error
                .lock()
                .map_err(|_| AppError::new("state_error", "Could not access recovery state."))? =
                None;
        }
        Err(error) => {
            *state
                .workspace_open_error
                .lock()
                .map_err(|_| AppError::new("state_error", "Could not access recovery state."))? =
                Some(error);
        }
    }

    get_app_bootstrap(app, state)
}
