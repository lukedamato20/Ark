mod chat;
mod commands;
mod config;
mod db;
mod errors;
mod export;
mod providers;
mod sidecar;
mod workspace;

use commands::{
    cancel_stream, create_conversation, delete_conversation, edit_user_message,
    export_conversation_json, export_conversation_markdown, get_app_bootstrap,
    get_assistant_alternatives, get_built_in_runtime_status, get_conversation_messages,
    import_conversation_json, list_conversations, refresh_models,
    regenerate_assistant_message, rename_conversation, reset_workspace, run_diagnostics,
    send_chat_message, set_theme, set_workspace, start_built_in_runtime, stop_built_in_runtime,
    switch_active_branch, update_provider,
};
use db::Database;
use sidecar::SidecarState;
use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use tauri::Manager;

pub struct AppState {
    pub db: Mutex<Database>,
    pub active_streams: Mutex<HashMap<String, Arc<AtomicBool>>>,
    pub sidecar: Mutex<SidecarState>,
}

impl Drop for AppState {
    fn drop(&mut self) {
        if let Ok(mut s) = self.sidecar.lock() {
            s.stop();
        }
    }
}

pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let workspace = workspace::resolve_default_workspace(app.handle())?;
            let db = Database::open(workspace.database_path())?;

            app.manage(AppState {
                db: Mutex::new(db),
                active_streams: Mutex::new(HashMap::new()),
                sidecar: Mutex::new(SidecarState::new()),
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
            send_chat_message,
            edit_user_message,
            regenerate_assistant_message,
            cancel_stream,
            refresh_models,
            update_provider,
            set_theme,
            set_workspace,
            reset_workspace,
            run_diagnostics,
            export_conversation_markdown,
            export_conversation_json,
            import_conversation_json,
            start_built_in_runtime,
            stop_built_in_runtime,
            get_built_in_runtime_status
        ])
        .run(tauri::generate_context!())
        .unwrap_or_else(|error| {
            eprintln!("failed to run Ark: {error}");
        });
}
