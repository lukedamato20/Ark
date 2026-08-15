mod backup;
mod chat;
mod commands;
mod config;
#[cfg(test)]
mod contract;
mod data_protection;
mod db;
mod device_settings;
mod diagnostics;
mod diagnostics_bundle;
mod errors;
mod export;
mod file_permissions;
mod generation;
mod import_export;
mod observability;
mod projects;
mod provider_management;
mod providers;
mod proxy;
mod redaction;
mod secret_store;
mod security;
mod sidecar;
mod supply_chain;
mod tool_policy;
mod validation;
mod workspace;
mod workspace_bootstrap;

use commands::{
    cancel_import, cancel_ollama_pull, cancel_stream, create_conversation, create_project,
    create_workspace_backup, delete_conversation, delete_ollama_model, delete_project,
    delete_provider_secret, disable_workspace_encryption, discard_interrupted_message,
    edit_user_message, enable_workspace_encryption, export_conversation_json,
    export_conversation_markdown, export_diagnostics_bundle, get_app_bootstrap,
    get_assistant_alternatives, get_built_in_runtime_status, get_conversation_messages,
    get_message, get_provider_secret_metadata, get_secret_store_status,
    get_workspace_protection_status, import_conversation_json, keep_partial_message,
    list_conversations, list_projects, preview_conversation_import, preview_project_deletion,
    preview_workspace_restore, pull_ollama_model, refresh_models, regenerate_assistant_message,
    rename_conversation, reset_workspace, restore_workspace_backup, restore_workspace_recovery_key,
    retry_workspace_open, rotate_workspace_encryption, run_diagnostics, save_diagnostics_bundle,
    send_chat_message, set_branch_name, set_conversation_archived, set_conversation_pinned,
    set_conversation_project, set_project_archived, set_workspace, start_built_in_runtime,
    start_pending_stream, stop_built_in_runtime, switch_active_branch,
    update_conversation_settings, update_device_settings, update_project, update_provider,
    upsert_provider_secret,
};
use db::Database;
use errors::AppError;
use sidecar::SidecarState;
use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use tauri::Manager;

pub struct AppState {
    pub db: Mutex<Database>,
    /// The selected workspace metadata is retained even while the database connections point
    /// at the in-memory recovery fallback. Bootstrap must not re-run a failing config/path
    /// resolver, otherwise the recovery UI itself would become unreachable.
    pub workspace: Mutex<workspace::WorkspaceInfo>,
    /// ARC-004: a second connection to the same database, opened read-only (see
    /// `Database::open_read_replica`). WAL mode means this never blocks, and is never blocked
    /// by, a write transaction in progress on `db` — used by read-hot command handlers that
    /// must stay responsive while a streaming generation is checkpointing writes. See
    /// `commands::lock_read_db`. Kept in lockstep with `db`: both are (re)opened together
    /// against the same path everywhere `db` is opened (initial `.setup()`, COR-010's in-memory
    /// fallback, and `retry_workspace_open`).
    pub read_db: Mutex<Database>,
    /// COR-010: set when the real workspace database failed to open at startup and `db`
    /// currently holds a temporary in-memory fallback instead (see `run`'s `.setup()`).
    /// Surfaced to the frontend via `get_app_bootstrap` so the user gets a typed recovery
    /// screen — pick a different workspace, or retry once the underlying issue (e.g. another
    /// Ark instance holding the lock) is resolved — rather than the app failing to launch at
    /// all, which is what happened before this existed.
    pub workspace_open_error: Mutex<Option<AppError>>,
    pub(crate) active_streams: Mutex<HashMap<String, Arc<generation::StreamCancellation>>>,
    pub(crate) pending_streams: Mutex<HashMap<String, generation::PendingStream>>,
    pub(crate) active_imports: Mutex<HashMap<String, Arc<AtomicBool>>>,
    /// FTR-006: keyed by provider ID, matching the UI's single-pull-per-provider reality — see
    /// `provider_management::pull_ollama_model`'s own doc comment.
    pub(crate) active_ollama_pulls: Mutex<HashMap<String, Arc<AtomicBool>>>,
    /// SEC-006: gates all database commands while a copy/verify/swap protection migration owns
    /// both connections. Compare-and-swap prevents concurrent protection operations.
    pub(crate) storage_maintenance: AtomicBool,
    pub sidecar: Arc<Mutex<SidecarState>>,
    /// OPS-001: bounded, redacted, best-effort-persisted local diagnostics log. `Arc` for the
    /// same reason as `sidecar` — the panic hook installed in `run()` needs a handle to it that
    /// outlives any single command invocation.
    pub(crate) observability_log: Arc<Mutex<observability::DiagnosticsLog>>,
}

impl Drop for AppState {
    fn drop(&mut self) {
        // ARC-004: best-effort — a failed/skipped checkpoint never corrupts the database (see
        // `Database::checkpoint`'s doc comment), so this is reported, not treated as fatal, and
        // must never panic during shutdown.
        if let Ok(db) = self.db.lock() {
            if let Err(error) = db.checkpoint() {
                eprintln!("Failed to checkpoint workspace database on shutdown: {error}");
            }
        }
        if let Ok(mut s) = self.sidecar.lock() {
            if let Err(failure) = s.stop() {
                eprintln!(
                    "Failed to stop managed runtime during shutdown: {}",
                    failure.message
                );
            }
        }
    }
}

/// ARC-004: opens the writer/read-replica connection pair together against the same path — the
/// one place that pairing is expressed, used both at startup (`.setup()` below) and by
/// `workspace_bootstrap::retry_workspace_open`, so the two connections can never drift onto
/// different files/fallback states.
pub(crate) fn open_database_pair(path: &std::path::Path) -> Result<(Database, Database), AppError> {
    let key = data_protection::key_for_database_open(path)?;
    open_database_pair_with_key(path, key.as_deref().map(String::as_str))
}

pub(crate) fn open_database_pair_with_key(
    path: &std::path::Path,
    key: Option<&str>,
) -> Result<(Database, Database), AppError> {
    let db = Database::open_with_key(path, key)?;
    let read_db = Database::open_read_replica_with_key(path, key)?;
    Ok((db, read_db))
}

/// OPS-001: opt-in, off-by-default local crash capture. Chains onto (never replaces) the
/// default panic hook, so normal stderr panic output is unaffected. Deliberately bypasses
/// `AppState.observability_log`'s `Mutex` entirely and writes straight to the log file via
/// `observability::record_crash_directly_to_file` — see that function's doc comment for why:
/// if the panic happened inside a call already holding that mutex on this same thread, taking
/// it again here would deadlock instead of failing safely. Re-reads `DeviceSettings` from disk
/// at panic time (not a value captured at startup) so toggling the setting takes effect on the
/// very next panic without requiring a restart; the read is a small, tolerant file read with no
/// locks involved, consistent with `device_settings::load_device_settings`'s existing "corrupt
/// or missing is just treated as absent" behavior. Nothing this hook does ever transmits
/// anything off the device — the diagnostics bundle export is a separate, reviewed, manually
/// user-initiated action.
fn install_crash_hook(app_handle: tauri::AppHandle, log_file_path: Option<std::path::PathBuf>) {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        default_hook(panic_info);
        let Some(path) = &log_file_path else {
            return;
        };
        let settings = device_settings::load_device_settings(&app_handle, None);
        if !settings.crash_capture_enabled {
            return;
        }
        observability::record_crash_directly_to_file(path, &panic_info.to_string());
    }));
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let (workspace, workspace_resolution_error) =
                workspace::resolve_workspace_for_startup(app.handle())?;
            let workspace_info = workspace.info();

            // COR-010: never let a broken workspace database (corrupt file, locked by another
            // instance, disk full, read-only permissions) prevent Ark from launching at all.
            // Fall back to a harmless in-memory database so the window still opens and every
            // command still works — Settings/diagnostics are fully reachable — while the
            // frontend is told about the real failure and offered Retry / choose-a-different-
            // workspace, matching this task's "storage failures never leave an indefinite
            // loading screen" outcome.
            let (db, read_db, workspace_open_error) = if let Some(error) = workspace_resolution_error
            {
                let (fallback, fallback_read) =
                    open_database_pair(std::path::Path::new(":memory:"))?;
                (fallback, fallback_read, Some(error))
            } else {
                match open_database_pair(&workspace.database_path()) {
                    Ok((db, read_db)) => (db, read_db, None),
                    Err(error) => {
                        eprintln!(
                            "Failed to open workspace database ({error}); falling back to a temporary in-memory database."
                        );
                        let (fallback, fallback_read) =
                            open_database_pair(std::path::Path::new(":memory:"))?;
                        (fallback, fallback_read, Some(error))
                    }
                }
            };

            // OPS-001: a bounded, redacted, best-effort-persisted local diagnostics log — see
            // `observability.rs`'s module doc. Unattached (in-memory only) if the config
            // directory can't be resolved, which never blocks startup.
            let mut diagnostics_log = observability::DiagnosticsLog::new();
            let diagnostics_log_path = app
                .path()
                .app_config_dir()
                .ok()
                .map(|dir| dir.join("logs").join("ark.log"));
            if let Some(path) = diagnostics_log_path.clone() {
                diagnostics_log.attach_file(path);
            }
            if let Some(error) = &workspace_open_error {
                diagnostics_log.record(
                    observability::LogLevel::Warn,
                    "startup",
                    None,
                    &format!("workspace open failed, using in-memory fallback: {}", error.code),
                );
            }

            install_crash_hook(app.handle().clone(), diagnostics_log_path);

            app.manage(AppState {
                db: Mutex::new(db),
                workspace: Mutex::new(workspace_info),
                read_db: Mutex::new(read_db),
                workspace_open_error: Mutex::new(workspace_open_error),
                active_streams: Mutex::new(HashMap::new()),
                pending_streams: Mutex::new(HashMap::new()),
                active_imports: Mutex::new(HashMap::new()),
                active_ollama_pulls: Mutex::new(HashMap::new()),
                storage_maintenance: AtomicBool::new(false),
                sidecar: Arc::new(Mutex::new(SidecarState::new())),
                observability_log: Arc::new(Mutex::new(diagnostics_log)),
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_app_bootstrap,
            list_conversations,
            create_conversation,
            rename_conversation,
            update_conversation_settings,
            set_conversation_archived,
            set_conversation_pinned,
            set_conversation_project,
            list_projects,
            create_project,
            update_project,
            set_project_archived,
            preview_project_deletion,
            delete_project,
            delete_conversation,
            get_conversation_messages,
            get_message,
            get_assistant_alternatives,
            switch_active_branch,
            set_branch_name,
            keep_partial_message,
            discard_interrupted_message,
            send_chat_message,
            edit_user_message,
            regenerate_assistant_message,
            start_pending_stream,
            cancel_stream,
            refresh_models,
            update_provider,
            get_secret_store_status,
            upsert_provider_secret,
            get_provider_secret_metadata,
            delete_provider_secret,
            get_workspace_protection_status,
            enable_workspace_encryption,
            rotate_workspace_encryption,
            disable_workspace_encryption,
            restore_workspace_recovery_key,
            update_device_settings,
            set_workspace,
            reset_workspace,
            retry_workspace_open,
            create_workspace_backup,
            preview_workspace_restore,
            restore_workspace_backup,
            export_diagnostics_bundle,
            save_diagnostics_bundle,
            run_diagnostics,
            export_conversation_markdown,
            export_conversation_json,
            import_conversation_json,
            preview_conversation_import,
            cancel_import,
            start_built_in_runtime,
            stop_built_in_runtime,
            get_built_in_runtime_status,
            pull_ollama_model,
            delete_ollama_model,
            cancel_ollama_pull
        ])
        .run(tauri::generate_context!())
        .unwrap_or_else(|error| {
            eprintln!("failed to run Ark: {error}");
        });
}
