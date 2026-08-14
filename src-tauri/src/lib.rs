mod chat;
mod commands;
mod config;
#[cfg(test)]
mod contract;
mod data_protection;
mod db;
mod device_settings;
mod diagnostics;
mod errors;
mod export;
mod file_permissions;
mod generation;
mod import_export;
mod provider_management;
mod providers;
mod secret_store;
mod security;
mod sidecar;
mod supply_chain;
mod tool_policy;
mod validation;
mod workspace;
mod workspace_bootstrap;

use commands::{
    cancel_import, cancel_stream, create_conversation, delete_conversation, delete_ollama_model,
    delete_provider_secret, disable_workspace_encryption, discard_interrupted_message,
    edit_user_message, enable_workspace_encryption, export_conversation_json,
    export_conversation_markdown, get_app_bootstrap, get_assistant_alternatives,
    get_built_in_runtime_status, get_conversation_messages, get_provider_secret_metadata,
    get_secret_store_status, get_workspace_protection_status, import_conversation_json,
    keep_partial_message, list_conversations, preview_conversation_import, pull_ollama_model,
    refresh_models, regenerate_assistant_message, rename_conversation, reset_workspace,
    restore_workspace_recovery_key, retry_workspace_open, rotate_workspace_encryption,
    run_diagnostics, send_chat_message, set_workspace, start_built_in_runtime,
    start_pending_stream, stop_built_in_runtime, switch_active_branch, update_device_settings,
    update_provider, upsert_provider_secret,
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
    /// SEC-006: gates all database commands while a copy/verify/swap protection migration owns
    /// both connections. Compare-and-swap prevents concurrent protection operations.
    pub(crate) storage_maintenance: AtomicBool,
    pub sidecar: Arc<Mutex<SidecarState>>,
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

            app.manage(AppState {
                db: Mutex::new(db),
                workspace: Mutex::new(workspace_info),
                read_db: Mutex::new(read_db),
                workspace_open_error: Mutex::new(workspace_open_error),
                active_streams: Mutex::new(HashMap::new()),
                pending_streams: Mutex::new(HashMap::new()),
                active_imports: Mutex::new(HashMap::new()),
                storage_maintenance: AtomicBool::new(false),
                sidecar: Arc::new(Mutex::new(SidecarState::new())),
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_app_bootstrap,
            list_conversations,
            create_conversation,
            rename_conversation,
            delete_conversation,
            get_conversation_messages,
            get_assistant_alternatives,
            switch_active_branch,
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
            delete_ollama_model
        ])
        .run(tauri::generate_context!())
        .unwrap_or_else(|error| {
            eprintln!("failed to run Ark: {error}");
        });
}
