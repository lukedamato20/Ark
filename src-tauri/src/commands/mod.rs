use crate::chat::{
    BranchAlternative, ConversationListRequest, ConversationPage, Message, SendChatRequest,
    SendChatResult,
};
use crate::db::Database;
use crate::errors::AppError;
use crate::providers::ProviderConfig;
use crate::workspace::WorkspaceInfo;
use crate::AppState;
use serde::Deserialize;
use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};

pub use crate::workspace_bootstrap::AppBootstrap;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenameConversationRequest {
    pub id: String,
    pub title: String,
}

/// FTR-004: each field independently `Option` — `None`/blank clears that override tier back to
/// "inherit the provider default," matching `validation::validate_system_prompt`'s normalization.
/// The frontend always sends its complete, already-merged current draft (same convention as
/// `DeviceSettings`), so there is no partial-update/merge logic here.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateConversationSettingsRequest {
    pub id: String,
    pub system_prompt: Option<String>,
    pub temperature: Option<f64>,
    pub max_tokens: Option<i64>,
}

pub use crate::generation::{EditUserMessageRequest, RegenerateAssistantMessageRequest};

/// FTR-003: `name` is always sent; every other field independently `None`/blank clears that
/// project-level default, matching `UpdateConversationSettingsRequest`'s convention — the
/// frontend always sends its complete current draft, not a partial patch.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProjectRequest {
    pub id: String,
    pub name: String,
    pub instructions: Option<String>,
    pub default_provider_id: Option<String>,
    pub default_model_id: Option<String>,
    pub default_temperature: Option<f64>,
    pub default_max_tokens: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssistantBranchRequest {
    pub conversation_id: String,
    pub message_id: String,
}

pub use crate::provider_management::{
    BuiltInRuntimeStatus, DeleteOllamaModelRequest, PullOllamaModelRequest, RefreshModelsResult,
    UpdateProviderRequest,
};

pub use crate::data_protection::{WorkspaceProtectionChange, WorkspaceProtectionStatus};
pub use crate::device_settings::DeviceSettings;
pub use crate::secret_store::{SecretMetadata, SecretStoreStatus};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetWorkspaceRequest {
    pub root_path: String,
    /// FTR-001: when `true`, seeds the new location with a verified copy of the current
    /// workspace database before repointing to it — "start empty" (the pre-existing default
    /// behavior) is `false`/omitted. There is deliberately no "move" option; see
    /// `backup.rs`'s module doc comment for why deleting the original isn't implemented.
    pub copy_data: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreWorkspaceRecoveryKeyRequest {
    pub recovery_key: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportConversationRequest {
    pub import_id: String,
    pub json: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportProgressEvent {
    pub import_id: String,
    pub completed_messages: usize,
    pub total_messages: usize,
}

// ARC-001: assembling the startup read-model and recovering a failed workspace database are
// application workflows — the implementation lives in `crate::workspace_bootstrap`.
#[tauri::command]
pub fn get_app_bootstrap(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<AppBootstrap, AppError> {
    crate::workspace_bootstrap::get_app_bootstrap(&app, &state)
}

#[tauri::command]
pub fn retry_workspace_open(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<AppBootstrap, AppError> {
    crate::workspace_bootstrap::retry_workspace_open(&app, &state)
}

#[tauri::command]
pub fn list_conversations(
    state: State<'_, AppState>,
    request: ConversationListRequest,
) -> Result<ConversationPage, AppError> {
    // ARC-004: read-hot, called every time the sidebar refreshes — routed through the read
    // replica so it stays responsive while a streaming generation holds the writer lock for a
    // checkpoint write.
    lock_read_db(&state)?.list_conversations_page(&request)
}

#[tauri::command]
pub fn create_conversation(
    state: State<'_, AppState>,
    title: Option<String>,
) -> Result<crate::chat::Conversation, AppError> {
    lock_db(&state)?.create_conversation(title)
}

#[tauri::command]
pub fn rename_conversation(
    state: State<'_, AppState>,
    request: RenameConversationRequest,
) -> Result<crate::chat::Conversation, AppError> {
    let id = crate::validation::validate_entity_id(&request.id, "Conversation ID")?;
    lock_db(&state)?.rename_conversation(id, &request.title)
}

#[tauri::command]
pub fn delete_conversation(state: State<'_, AppState>, id: String) -> Result<(), AppError> {
    let id = crate::validation::validate_entity_id(&id, "Conversation ID")?;
    lock_db(&state)?.delete_conversation(id)
}

#[tauri::command]
pub fn update_conversation_settings(
    state: State<'_, AppState>,
    request: UpdateConversationSettingsRequest,
) -> Result<crate::chat::Conversation, AppError> {
    let id = crate::validation::validate_entity_id(&request.id, "Conversation ID")?.to_string();
    let system_prompt = crate::validation::validate_system_prompt(request.system_prompt)?;
    let temperature = crate::validation::validate_temperature(request.temperature)?;
    let max_tokens = crate::validation::validate_max_tokens(request.max_tokens)?;
    lock_db(&state)?.update_conversation_settings(
        &id,
        system_prompt.as_deref(),
        temperature,
        max_tokens,
    )
}

#[tauri::command]
pub fn set_conversation_archived(
    state: State<'_, AppState>,
    id: String,
    archived: bool,
) -> Result<crate::chat::Conversation, AppError> {
    let id = crate::validation::validate_entity_id(&id, "Conversation ID")?;
    lock_db(&state)?.set_conversation_archived(id, archived)
}

#[tauri::command]
pub fn set_conversation_pinned(
    state: State<'_, AppState>,
    id: String,
    pinned: bool,
) -> Result<crate::chat::Conversation, AppError> {
    let id = crate::validation::validate_entity_id(&id, "Conversation ID")?;
    lock_db(&state)?.set_conversation_pinned(id, pinned)
}

#[tauri::command]
pub fn set_conversation_project(
    state: State<'_, AppState>,
    id: String,
    project_id: Option<String>,
) -> Result<crate::chat::Conversation, AppError> {
    let id = crate::validation::validate_entity_id(&id, "Conversation ID")?.to_string();
    let project_id = project_id
        .as_deref()
        .map(|value| crate::validation::validate_entity_id(value, "Project ID"))
        .transpose()?
        .map(str::to_string);
    lock_db(&state)?.set_conversation_project(&id, project_id.as_deref())
}

#[tauri::command]
pub fn list_projects(
    state: State<'_, AppState>,
) -> Result<Vec<crate::projects::Project>, AppError> {
    lock_read_db(&state)?.list_projects()
}

#[tauri::command]
pub fn create_project(
    state: State<'_, AppState>,
    name: String,
) -> Result<crate::projects::Project, AppError> {
    lock_db(&state)?.create_project(&name)
}

#[tauri::command]
pub fn update_project(
    state: State<'_, AppState>,
    request: UpdateProjectRequest,
) -> Result<crate::projects::Project, AppError> {
    let id = crate::validation::validate_entity_id(&request.id, "Project ID")?.to_string();
    let instructions = crate::validation::validate_system_prompt(request.instructions)?;
    let temperature = crate::validation::validate_temperature(request.default_temperature)?;
    let max_tokens = crate::validation::validate_max_tokens(request.default_max_tokens)?;
    lock_db(&state)?.update_project(
        &id,
        crate::projects::UpdateProjectChanges {
            name: &request.name,
            instructions: instructions.as_deref(),
            default_provider_id: request.default_provider_id.as_deref(),
            default_model_id: request.default_model_id.as_deref(),
            default_temperature: temperature,
            default_max_tokens: max_tokens,
        },
    )
}

#[tauri::command]
pub fn set_project_archived(
    state: State<'_, AppState>,
    id: String,
    archived: bool,
) -> Result<crate::projects::Project, AppError> {
    let id = crate::validation::validate_entity_id(&id, "Project ID")?;
    lock_db(&state)?.set_project_archived(id, archived)
}

#[tauri::command]
pub fn preview_project_deletion(
    state: State<'_, AppState>,
    id: String,
) -> Result<crate::projects::ProjectDeletionPreview, AppError> {
    let id = crate::validation::validate_entity_id(&id, "Project ID")?;
    lock_read_db(&state)?.preview_project_deletion(id)
}

#[tauri::command]
pub fn delete_project(state: State<'_, AppState>, id: String) -> Result<(), AppError> {
    let id = crate::validation::validate_entity_id(&id, "Project ID")?;
    lock_db(&state)?.delete_project(id)
}

#[tauri::command]
pub fn get_conversation_messages(
    state: State<'_, AppState>,
    conversation_id: String,
) -> Result<Vec<Message>, AppError> {
    // ARC-004: read-hot, called every time the user switches conversations — see
    // `list_conversations` above for why this goes through the read replica.
    let conversation_id =
        crate::validation::validate_entity_id(&conversation_id, "Conversation ID")?;
    lock_read_db(&state)?.get_active_messages(conversation_id)
}

/// FTR-005: fetches a single message's full content — `get_assistant_alternatives` only returns
/// a 140-character preview per sibling (`Database::message_preview`), which is enough for the
/// switcher list but not for the side-by-side comparison view, which needs the complete response.
#[tauri::command]
pub fn get_message(state: State<'_, AppState>, id: String) -> Result<Message, AppError> {
    let id = crate::validation::validate_entity_id(&id, "Message ID")?;
    lock_read_db(&state)?.get_message(id)
}

#[tauri::command]
pub fn get_assistant_alternatives(
    state: State<'_, AppState>,
    request: AssistantBranchRequest,
) -> Result<Vec<BranchAlternative>, AppError> {
    let conversation_id =
        crate::validation::validate_entity_id(&request.conversation_id, "Conversation ID")?;
    let message_id = crate::validation::validate_entity_id(&request.message_id, "Message ID")?;
    lock_db(&state)?.get_assistant_alternatives(conversation_id, message_id)
}

#[tauri::command]
pub fn switch_active_branch(
    state: State<'_, AppState>,
    request: AssistantBranchRequest,
) -> Result<Vec<Message>, AppError> {
    let conversation_id =
        crate::validation::validate_entity_id(&request.conversation_id, "Conversation ID")?;
    let message_id = crate::validation::validate_entity_id(&request.message_id, "Message ID")?;
    lock_db(&state)?.switch_active_branch(conversation_id, message_id)
}

#[tauri::command]
pub fn set_branch_name(
    state: State<'_, AppState>,
    message_id: String,
    name: Option<String>,
) -> Result<Message, AppError> {
    let message_id = crate::validation::validate_entity_id(&message_id, "Message ID")?.to_string();
    let name = crate::validation::validate_branch_name(name)?;
    lock_db(&state)?.set_message_branch_name(&message_id, name.as_deref())
}

/// COR-001 recovery action: accept an interrupted assistant message's partial content as final.
#[tauri::command]
pub fn keep_partial_message(
    state: State<'_, AppState>,
    message_id: String,
) -> Result<Message, AppError> {
    let message_id = crate::validation::validate_entity_id(&message_id, "Message ID")?;
    lock_db(&state)?.keep_partial_message(message_id)
}

/// COR-001 recovery action: move the active branch away from an interrupted assistant message
/// without deleting it, preferring a completed sibling response.
#[tauri::command]
pub fn discard_interrupted_message(
    state: State<'_, AppState>,
    request: AssistantBranchRequest,
) -> Result<Vec<Message>, AppError> {
    let conversation_id =
        crate::validation::validate_entity_id(&request.conversation_id, "Conversation ID")?;
    let message_id = crate::validation::validate_entity_id(&request.message_id, "Message ID")?;
    lock_db(&state)?.discard_interrupted_message(conversation_id, message_id)
}

// ARC-001: provider management (config updates, model refresh, Ollama pull/delete, built-in
// runtime lifecycle) is an application workflow — the implementation lives in
// `crate::provider_management`. These commands remain transport adapters only.
#[tauri::command]
pub fn update_provider(
    state: State<'_, AppState>,
    request: UpdateProviderRequest,
) -> Result<ProviderConfig, AppError> {
    crate::provider_management::update_provider(&state, request)
}

// SEC-005: these adapters expose only availability, masked metadata, and opaque identifiers.
// Secret values enter Rust once on upsert and are moved directly into the OS credential store;
// no command can read a raw value back across IPC.
#[tauri::command]
pub async fn get_secret_store_status() -> SecretStoreStatus {
    crate::secret_store::get_status().await
}

#[tauri::command]
pub async fn upsert_provider_secret(
    state: State<'_, AppState>,
    provider_id: String,
    secret: String,
) -> Result<SecretMetadata, AppError> {
    crate::secret_store::upsert_provider_secret(&state, provider_id, secret).await
}

#[tauri::command]
pub async fn get_provider_secret_metadata(
    state: State<'_, AppState>,
    provider_id: String,
) -> Result<Option<SecretMetadata>, AppError> {
    crate::secret_store::get_provider_secret_metadata(&state, provider_id).await
}

#[tauri::command]
pub async fn delete_provider_secret(
    state: State<'_, AppState>,
    provider_id: String,
) -> Result<(), AppError> {
    crate::secret_store::delete_provider_secret(&state, provider_id).await
}

// SEC-006: protection commands expose mode/availability plus a one-time recovery key after
// enable/rotation. Raw database keys are never readable through IPC.
#[tauri::command]
pub fn get_workspace_protection_status(
    state: State<'_, AppState>,
) -> Result<WorkspaceProtectionStatus, AppError> {
    crate::data_protection::get_status(&state)
}

#[tauri::command]
pub fn enable_workspace_encryption(
    state: State<'_, AppState>,
) -> Result<WorkspaceProtectionChange, AppError> {
    crate::data_protection::enable_encryption(&state)
}

#[tauri::command]
pub fn rotate_workspace_encryption(
    state: State<'_, AppState>,
) -> Result<WorkspaceProtectionChange, AppError> {
    crate::data_protection::rotate_key(&state)
}

#[tauri::command]
pub fn disable_workspace_encryption(
    state: State<'_, AppState>,
) -> Result<WorkspaceProtectionStatus, AppError> {
    crate::data_protection::disable_encryption(&state)
}

#[tauri::command]
pub fn restore_workspace_recovery_key(
    state: State<'_, AppState>,
    request: RestoreWorkspaceRecoveryKeyRequest,
) -> Result<WorkspaceProtectionStatus, AppError> {
    crate::data_protection::restore_recovery_key(&state, request.recovery_key)
}

// ARC-006: theme and the built-in runtime's model path are device-scoped settings, not
// workspace-scoped ones — the implementation lives in `crate::device_settings`.
#[tauri::command]
pub fn update_device_settings(
    app: AppHandle,
    settings: DeviceSettings,
) -> Result<DeviceSettings, AppError> {
    crate::device_settings::update_device_settings(&app, settings)
}

#[tauri::command]
pub fn set_workspace(
    app: AppHandle,
    state: State<'_, AppState>,
    request: SetWorkspaceRequest,
) -> Result<WorkspaceInfo, AppError> {
    crate::workspace::set_workspace_root(
        &app,
        &state,
        &request.root_path,
        request.copy_data.unwrap_or(false),
    )
}

#[tauri::command]
pub fn reset_workspace(app: AppHandle) -> Result<WorkspaceInfo, AppError> {
    crate::workspace::reset_workspace_root(&app)
}

#[tauri::command]
pub fn create_workspace_backup(
    state: State<'_, AppState>,
    destination_dir: String,
) -> Result<crate::backup::BackupResult, AppError> {
    crate::backup::create_backup(&state, destination_dir)
}

#[tauri::command]
pub fn preview_workspace_restore(
    state: State<'_, AppState>,
    backup_path: String,
) -> Result<crate::backup::RestorePreview, AppError> {
    crate::backup::preview_restore(&state, backup_path)
}

#[tauri::command]
pub fn restore_workspace_backup(
    state: State<'_, AppState>,
    backup_path: String,
    target_root: String,
) -> Result<(), AppError> {
    crate::backup::restore_backup(&state, backup_path, target_root)
}

/// OPS-001: builds the reviewable diagnostics bundle text. The frontend always shows this exact
/// text to the user before calling `save_diagnostics_bundle` — see `diagnostics_bundle.rs`'s
/// module doc for why the save command takes the reviewed text verbatim rather than
/// re-assembling it.
#[tauri::command]
pub fn export_diagnostics_bundle(
    state: State<'_, AppState>,
) -> Result<crate::diagnostics_bundle::DiagnosticsBundle, AppError> {
    crate::diagnostics_bundle::build_diagnostics_bundle(&state)
}

#[tauri::command]
pub fn save_diagnostics_bundle(
    destination_path: String,
    bundle_text: String,
) -> Result<(), AppError> {
    crate::diagnostics_bundle::save_diagnostics_bundle(&destination_path, &bundle_text)
}

#[tauri::command]
pub async fn refresh_models(
    state: State<'_, AppState>,
    provider_id: String,
) -> Result<RefreshModelsResult, AppError> {
    crate::provider_management::refresh_models(&state, provider_id).await
}

// ARC-001: sending/editing/regenerating a chat message and cancelling a stream are the core
// conversation/generation application workflow — the implementation lives in
// `crate::generation` alongside the streaming supervision it hands off to. These commands
// remain transport adapters only.
#[tauri::command]
pub fn send_chat_message(
    state: State<'_, AppState>,
    request: SendChatRequest,
) -> Result<SendChatResult, AppError> {
    crate::generation::send_chat_message(&state, request)
}

#[tauri::command]
pub fn edit_user_message(
    state: State<'_, AppState>,
    request: EditUserMessageRequest,
) -> Result<SendChatResult, AppError> {
    crate::generation::edit_user_message(&state, request)
}

#[tauri::command]
pub fn regenerate_assistant_message(
    state: State<'_, AppState>,
    request: RegenerateAssistantMessageRequest,
) -> Result<SendChatResult, AppError> {
    crate::generation::regenerate_assistant_message(&state, request)
}

#[tauri::command]
pub fn start_pending_stream(
    app: AppHandle,
    state: State<'_, AppState>,
    message_id: String,
) -> Result<(), AppError> {
    crate::generation::start_pending_stream(app, &state, message_id)
}

#[tauri::command]
pub fn cancel_stream(
    app: AppHandle,
    state: State<'_, AppState>,
    message_id: String,
) -> Result<(), AppError> {
    crate::generation::cancel_stream(app, &state, message_id)
}

// ARC-001: system diagnostics (hardware probing, provider health check, benchmark run) is an
// application workflow — the implementation lives in `crate::diagnostics`. This command remains
// a transport adapter: decode the request, delegate, return the result.
#[tauri::command]
pub async fn run_diagnostics(
    state: State<'_, AppState>,
    provider_id: String,
    model: Option<String>,
    include_runtime_logs: Option<bool>,
) -> Result<crate::diagnostics::DiagnosticsResult, AppError> {
    crate::diagnostics::run_diagnostics(
        &state,
        provider_id,
        model,
        include_runtime_logs.unwrap_or(false),
    )
    .await
}

// ARC-001: export/import is an application workflow, not command-handling logic — the
// implementation lives in `crate::import_export` as plain functions over `&Database` (fully
// unit-testable with no Tauri runtime). These commands are transport adapters only: lock the
// database, delegate, return the result.

pub use crate::import_export::ImportConversationResult;

#[tauri::command]
pub fn export_conversation_markdown(
    state: State<'_, AppState>,
    conversation_id: String,
) -> Result<String, AppError> {
    let conversation_id =
        crate::validation::validate_entity_id(&conversation_id, "Conversation ID")?;
    let db = lock_db(&state)?;
    crate::import_export::export_conversation_markdown(&db, conversation_id)
}

#[tauri::command]
pub fn export_conversation_json(
    state: State<'_, AppState>,
    conversation_id: String,
) -> Result<String, AppError> {
    let conversation_id =
        crate::validation::validate_entity_id(&conversation_id, "Conversation ID")?;
    let db = lock_db(&state)?;
    crate::import_export::export_conversation_json(&db, conversation_id)
}

#[tauri::command]
pub fn import_conversation_json(
    app: AppHandle,
    state: State<'_, AppState>,
    request: ImportConversationRequest,
) -> Result<ImportConversationResult, AppError> {
    let import_id =
        crate::validation::validate_entity_id(&request.import_id, "Import ID")?.to_string();
    let cancellation = Arc::new(AtomicBool::new(false));
    {
        let mut active_imports = state
            .active_imports
            .lock()
            .map_err(|_| AppError::new("state_error", "Could not access active imports."))?;
        if active_imports.contains_key(&import_id) {
            return Err(AppError::new(
                "import_conflict",
                "An import with this ID is already running.",
            ));
        }
        active_imports.insert(import_id.clone(), cancellation.clone());
    }

    let result = (|| {
        let db = lock_db(&state)?;
        crate::import_export::import_conversation_json_with_control(
            &db,
            &request.json,
            || cancellation.load(Ordering::Acquire),
            |completed_messages, total_messages| {
                if completed_messages == total_messages || completed_messages % 100 == 0 {
                    app.emit(
                        "import:progress",
                        ImportProgressEvent {
                            import_id: import_id.clone(),
                            completed_messages,
                            total_messages,
                        },
                    )
                    .map_err(|error| {
                        AppError::new(
                            "event_error",
                            format!("Could not report import progress: {error}"),
                        )
                    })?;
                }
                Ok(())
            },
        )
    })();

    state
        .active_imports
        .lock()
        .map_err(|_| AppError::new("state_error", "Could not access active imports."))?
        .remove(&import_id);
    result
}

#[tauri::command]
pub fn preview_conversation_import(
    state: State<'_, AppState>,
    json: String,
) -> Result<crate::import_export::ImportConversationPreview, AppError> {
    let db = lock_db(&state)?;
    crate::import_export::preview_conversation_import(&db, &json)
}

#[tauri::command]
pub fn cancel_import(state: State<'_, AppState>, import_id: String) -> Result<(), AppError> {
    let import_id = crate::validation::validate_entity_id(&import_id, "Import ID")?;
    if let Some(cancellation) = state
        .active_imports
        .lock()
        .map_err(|_| AppError::new("state_error", "Could not access active imports."))?
        .get(import_id)
    {
        cancellation.store(true, Ordering::Release);
    }
    Ok(())
}

#[tauri::command]
pub async fn pull_ollama_model(
    app: AppHandle,
    state: State<'_, AppState>,
    request: PullOllamaModelRequest,
) -> Result<(), AppError> {
    crate::provider_management::pull_ollama_model(&app, &state, request).await
}

#[tauri::command]
pub async fn delete_ollama_model(
    state: State<'_, AppState>,
    request: DeleteOllamaModelRequest,
) -> Result<(), AppError> {
    crate::provider_management::delete_ollama_model(&state, request).await
}

#[tauri::command]
pub fn cancel_ollama_pull(state: State<'_, AppState>, provider_id: String) -> Result<(), AppError> {
    let provider_id = crate::validation::validate_entity_id(&provider_id, "Provider ID")?;
    crate::provider_management::cancel_ollama_pull(&state, provider_id)
}

// ARC-001: these two take `&AppState` — the plain data port — rather than Tauri's `State<T>`
// wrapper, specifically so every application-service function that depends on them (in
// `generation`, `diagnostics`, `provider_management`, `workspace_bootstrap`) can be constructed
// and unit-tested with a real `AppState` and no running Tauri app. Call sites passing
// `&state: &State<'_, AppState>` coerce automatically via `State`'s `Deref<Target = AppState>`.
pub(crate) fn lock_db(state: &AppState) -> Result<std::sync::MutexGuard<'_, Database>, AppError> {
    if state
        .storage_maintenance
        .load(std::sync::atomic::Ordering::Acquire)
    {
        return Err(AppError::new(
            "workspace_maintenance_busy",
            "Workspace protection is being changed. Retry when the operation completes.",
        ));
    }
    state
        .db
        .lock()
        .map_err(|_| AppError::new("state_error", "Could not access local database."))
}

/// ARC-004: locks the read-replica connection (see `AppState::read_db`) instead of the writer.
/// Used by read-hot command handlers (`list_conversations`, `get_conversation_messages`) that
/// must stay responsive while a streaming generation holds/releases the writer lock for
/// checkpoint writes — WAL mode means this connection is never blocked by that.
pub(crate) fn lock_read_db(
    state: &AppState,
) -> Result<std::sync::MutexGuard<'_, Database>, AppError> {
    if state
        .storage_maintenance
        .load(std::sync::atomic::Ordering::Acquire)
    {
        return Err(AppError::new(
            "workspace_maintenance_busy",
            "Workspace protection is being changed. Retry when the operation completes.",
        ));
    }
    state
        .read_db
        .lock()
        .map_err(|_| AppError::new("state_error", "Could not access local database."))
}

/// ARC-004: "No production mutex unwrap remains on database/process state" applies to the
/// sidecar (child-process) mutex too, not just the database ones above — a poisoned lock here
/// must surface as a typed error to the command handler, never panic the whole app.
pub(crate) fn lock_sidecar(
    state: &AppState,
) -> Result<std::sync::MutexGuard<'_, crate::sidecar::SidecarState>, AppError> {
    state.sidecar.lock().map_err(|_| {
        AppError::new(
            "state_error",
            "Could not access the built-in runtime state.",
        )
    })
}

// ── Built-in runtime (bundled llama-server) ──────────────────────────────────

#[tauri::command]
pub async fn get_built_in_runtime_status(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<BuiltInRuntimeStatus, AppError> {
    crate::provider_management::get_built_in_runtime_status(&app, &state).await
}

#[tauri::command]
pub async fn stop_built_in_runtime(state: State<'_, AppState>) -> Result<(), AppError> {
    crate::provider_management::stop_built_in_runtime(&state).await
}

#[tauri::command]
pub async fn start_built_in_runtime(
    model_path: String,
    model_source: String,
    model_license: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<BuiltInRuntimeStatus, AppError> {
    crate::provider_management::start_built_in_runtime(
        model_path,
        model_source,
        model_license,
        &app,
        &state,
    )
    .await
}
