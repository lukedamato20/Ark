use crate::chat::{
    BranchAlternative, ConversationListRequest, ConversationMessagePage, ConversationPage, Message,
    SendChatRequest, SendChatResult,
};
use crate::db::Database;
use crate::errors::AppError;
use crate::providers::ProviderConfig;
use crate::workspace::WorkspaceInfo;
use crate::AppState;
use chrono::{Duration as ChronoDuration, Utc};
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
    pub response_style: Option<String>,
    pub tone: Option<String>,
}

pub use crate::generation::{EditUserMessageRequest, RegenerateAssistantMessageRequest};

/// CMP-003: an explicit, user-chosen grant from the Tools settings panel — distinct from the
/// short, fixed-TTL grant `tools::authorize_note_write` creates automatically when a previewed
/// write is approved inline.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GrantToolCapabilityRequest {
    pub tool_id: String,
    pub ttl_minutes: i64,
}

/// CMP-003: previews a notes write before it runs. `content` is required for `create`/`update`,
/// ignored for `delete`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewNoteWriteRequest {
    pub action: crate::tools::NoteWriteAction,
    pub content: Option<String>,
}

/// CMP-003: `approve` mirrors `UpdateProviderChanges::acknowledge_remote_risk`'s established
/// shape — `false` on a normal attempt; the frontend resubmits with `true` only after the user
/// has seen `preview_note_write`'s output and explicitly confirmed it.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateNoteRequest {
    pub conversation_id: String,
    pub content: String,
    pub approve: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateNoteRequest {
    pub id: String,
    pub content: String,
    pub approve: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteNoteRequest {
    pub id: String,
    pub approve: bool,
}

/// CMP-004: previews a web search before it runs.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewWebSearchRequest {
    pub query: String,
}

/// CMP-004: `approve` mirrors `CreateNoteRequest`'s established shape.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchWebRequest {
    pub query: String,
    pub approve: bool,
}

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
    pub response_style: Option<String>,
    pub tone: Option<String>,
}

/// CODE-004: every read-only coding command names the Project whose persisted Repository binding
/// supplies the filesystem authority. No command accepts an arbitrary root path.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeProjectRequest {
    pub project_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeListDirectoryRequest {
    pub project_id: String,
    pub path: String,
    pub max_entries: Option<usize>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeReadFileRequest {
    pub project_id: String,
    pub path: String,
    pub start_line: Option<usize>,
    pub max_lines: Option<usize>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeSearchRequest {
    pub project_id: String,
    pub query: String,
    pub path: Option<String>,
    pub case_sensitive: Option<bool>,
    pub max_results: Option<usize>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeRunSearchRequest {
    pub run_id: String,
    pub query: String,
    pub path: Option<String>,
    pub case_sensitive: Option<bool>,
    pub max_results: Option<usize>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeRepositoryMapRequest {
    pub project_id: String,
    pub max_entries: Option<usize>,
}

/// CODE-005: previews an `edit_file` write. `edits` are the same typed search/replace blocks the
/// approved execution request must echo back unchanged (via `CodeExecuteEditFileRequest`).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodePreviewEditFileRequest {
    pub project_id: String,
    pub path: String,
    pub edits: Vec<crate::code_write_tools::EditBlock>,
}

/// CODE-005: the frontend must echo `edits`/`call_hash`/`preview_hash`/`precondition_hash`
/// unchanged from the `EditFilePreview` the user approved — `execute_edit_file` re-derives all
/// three hashes from current Repository state and refuses if any no longer match.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeExecuteEditFileRequest {
    pub project_id: String,
    pub path: String,
    pub edits: Vec<crate::code_write_tools::EditBlock>,
    pub call_hash: String,
    pub preview_hash: String,
    pub precondition_hash: String,
}

/// Approval of a proposal produced inside the conversation loop. The client echoes the exact
/// persisted hashes it displayed; it never sends edit content separately.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeApproveEditRequest {
    pub session_id: String,
    pub run_id: String,
    pub invocation_id: String,
    pub call_hash: String,
    pub preview_hash: String,
    pub precondition_hash: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeRejectEditRequest {
    pub session_id: String,
    pub run_id: String,
    pub invocation_id: String,
    pub call_hash: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateCodeSessionRequest {
    pub project_id: String,
    pub title: String,
    pub idempotency_key: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateCodeRunRequest {
    pub session_id: String,
    pub parent_run_id: Option<String>,
    pub provider_id: String,
    pub model_id: String,
    /// CODE-007: what Ark Code should investigate. Required — a run with nothing to do cannot
    /// meaningfully plan its first step.
    pub task: String,
    pub max_steps: Option<u32>,
    pub max_active_ms: Option<u64>,
    pub max_tokens: Option<u64>,
    pub max_cost_microunits: Option<u64>,
    pub idempotency_key: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveCodeCommandDefinitionRequest {
    pub id: Option<String>,
    pub label: String,
    pub program: String,
    pub arguments: Vec<String>,
    pub timeout_seconds: u32,
    pub enabled: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CodeSessionRequestFingerprint<'a> {
    project_id: &'a str,
    title: &'a str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CodeRunRequestFingerprint<'a> {
    session_id: &'a str,
    parent_run_id: Option<&'a str>,
    provider_id: &'a str,
    model_id: &'a str,
    task: &'a str,
    repository_identity_hash: &'a str,
    max_steps: u32,
    max_active_ms: u64,
    max_tokens: u64,
    max_cost_microunits: Option<u64>,
}

/// FTR-003: `name` and `instructions` are always sent (unlike a project's optional
/// `instructions`, a persona's prompt content is required); `Database::create_persona` rejects a
/// blank `instructions` the same way it rejects a blank `name`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatePersonaRequest {
    pub name: String,
    pub instructions: String,
    pub default_temperature: Option<f64>,
    pub default_max_tokens: Option<i64>,
    pub response_style: Option<String>,
    pub tone: Option<String>,
}

/// FTR-003: mirrors `UpdateProjectRequest`'s "always send the complete current draft" convention.
/// `Database::update_persona` decides internally whether this actually creates a new immutable
/// version (only if `instructions`/the defaults changed) or just renames in place.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdatePersonaRequest {
    pub id: String,
    pub name: String,
    pub instructions: String,
    pub default_temperature: Option<f64>,
    pub default_max_tokens: Option<i64>,
    pub response_style: Option<String>,
    pub tone: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssistantBranchRequest {
    pub conversation_id: String,
    pub message_id: String,
}

pub use crate::provider_management::{
    BuiltInRuntimeStatus, CreateRemoteProviderRequest, DeleteOllamaModelRequest,
    PullOllamaModelRequest, RefreshModelsResult, UpdateProviderRequest,
};

pub use crate::data_protection::{WorkspaceProtectionChange, WorkspaceProtectionStatus};
pub use crate::device_settings::DeviceSettings;
pub use crate::managed_models::{
    ManagedModelDownloadRequest, ManagedModelOperation, ManagedModelPreflight, ManagedModelStatus,
    StartManagedModelRequest,
};
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
pub fn list_pinned_conversations(
    state: State<'_, AppState>,
    limit: u32,
) -> Result<Vec<crate::chat::Conversation>, AppError> {
    lock_read_db(&state)?.list_pinned_conversations(limit)
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
    let response_style = crate::validation::validate_response_style(request.response_style)?;
    let tone = crate::validation::validate_tone(request.tone)?;
    lock_db(&state)?.update_conversation_settings(
        &id,
        system_prompt.as_deref(),
        temperature,
        max_tokens,
        response_style.as_deref(),
        tone.as_deref(),
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
    let response_style = crate::validation::validate_response_style(request.response_style)?;
    let tone = crate::validation::validate_tone(request.tone)?;
    lock_db(&state)?.update_project(
        &id,
        crate::projects::UpdateProjectChanges {
            name: &request.name,
            instructions: instructions.as_deref(),
            default_provider_id: request.default_provider_id.as_deref(),
            default_model_id: request.default_model_id.as_deref(),
            default_temperature: temperature,
            default_max_tokens: max_tokens,
            response_style: response_style.as_deref(),
            tone: tone.as_deref(),
        },
    )
}

/// CODE-003: binds, switches, or removes the code Repository for an existing Project without
/// mutating Ark's storage Workspace. A blank value is normalized to removal for consistency
/// with the other optional Project settings.
#[tauri::command]
pub fn set_project_repository(
    state: State<'_, AppState>,
    id: String,
    repository_path: Option<String>,
) -> Result<crate::projects::Project, AppError> {
    let id = crate::validation::validate_entity_id(&id, "Project ID")?.to_string();
    // Reject a nonexistent Project before touching the filesystem with a writability probe.
    lock_read_db(&state)?.get_project(&id)?;

    let repository_path = repository_path
        .as_deref()
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .map(|path| {
            let workspace_root = state
                .workspace
                .lock()
                .map_err(|_| AppError::new("state_error", "Could not access Workspace state."))?
                .root_path
                .clone();
            let canonical = crate::repository::validate_repository_root(
                path,
                std::path::Path::new(&workspace_root),
            )?;
            canonical.to_str().map(str::to_string).ok_or_else(|| {
                AppError::invalid_input("Repository path must contain valid Unicode.")
            })
        })
        .transpose()?;

    lock_db(&state)?.set_project_repository(&id, repository_path.as_deref())
}

/// CODE-004's registry is intentionally separate from `list_tools`: Ark Chat never discovers or
/// grants Repository-tier tools through its Tools panel.
#[tauri::command]
pub fn list_ark_code_tools() -> Vec<crate::tools::ToolDefinition> {
    crate::code_tools::ark_code_tools()
}

#[tauri::command]
pub fn code_list_directory(
    state: State<'_, AppState>,
    request: CodeListDirectoryRequest,
) -> Result<crate::code_tools::RepositoryDirectoryListing, AppError> {
    let context = code_repository_context(&state, &request.project_id)?;
    crate::code_tools::list_directory(&context, &request.path, request.max_entries)
}

#[tauri::command]
pub fn code_read_file(
    state: State<'_, AppState>,
    request: CodeReadFileRequest,
) -> Result<crate::code_tools::RepositoryFileRead, AppError> {
    let context = code_repository_context(&state, &request.project_id)?;
    crate::code_tools::read_file(
        &context,
        &request.path,
        request.start_line,
        request.max_lines,
    )
}

#[tauri::command]
pub fn code_search(
    state: State<'_, AppState>,
    request: CodeSearchRequest,
) -> Result<crate::code_tools::RepositorySearchResult, AppError> {
    let context = code_repository_context(&state, &request.project_id)?;
    crate::code_tools::search(
        &context,
        &request.query,
        request.path.as_deref(),
        request.case_sensitive.unwrap_or(false),
        request.max_results,
    )
}

#[tauri::command]
pub fn code_repository_map(
    state: State<'_, AppState>,
    request: CodeRepositoryMapRequest,
) -> Result<crate::code_tools::RepositoryMap, AppError> {
    let context = code_repository_context(&state, &request.project_id)?;
    crate::code_tools::repository_map(&context, request.max_entries)
}

#[tauri::command]
pub async fn code_git_status(
    state: State<'_, AppState>,
    request: CodeProjectRequest,
) -> Result<crate::code_tools::RepositoryGitStatus, AppError> {
    let context = code_repository_context(&state, &request.project_id)?;
    crate::code_tools::git_status(&context).await
}

#[tauri::command]
pub async fn code_git_diff(
    state: State<'_, AppState>,
    request: CodeProjectRequest,
) -> Result<crate::code_tools::RepositoryGitDiff, AppError> {
    let context = code_repository_context(&state, &request.project_id)?;
    crate::code_tools::git_diff(&context).await
}

fn code_run_repository_context(
    state: &AppState,
    run_id: &str,
) -> Result<crate::code_tools::RepositoryContext, AppError> {
    let run = lock_read_db(state)?.get_code_agent_run(run_id)?;
    let workspace_root = {
        let workspace = state
            .workspace
            .lock()
            .map_err(|_| AppError::new("lock_poisoned", "Workspace state lock poisoned"))?;
        std::path::PathBuf::from(&workspace.root_path)
    };
    crate::code_git_tools::validate_run_repository(
        &run.repository_path_snapshot,
        &workspace_root,
        &run.session_id,
        &run.repository_identity_hash,
    )
}

#[tauri::command]
pub async fn get_code_run_repository_support(
    state: State<'_, AppState>,
    run_id: String,
) -> Result<crate::code_tools::CodeRepositorySupport, AppError> {
    let run_id = crate::validation::validate_entity_id(&run_id, "Ark Code run ID")?;
    let context = code_run_repository_context(&state, run_id)?;
    let repository_map = crate::code_tools::repository_map(&context, Some(1_000))?;
    let git_status = crate::code_tools::git_status(&context).await?;
    let git_diff = crate::code_tools::git_diff(&context).await?;
    Ok(crate::code_tools::CodeRepositorySupport {
        repository_map,
        git_status,
        git_diff,
    })
}

#[tauri::command]
pub fn search_code_run_repository(
    state: State<'_, AppState>,
    request: CodeRunSearchRequest,
) -> Result<crate::code_tools::RepositorySearchResult, AppError> {
    let run_id = crate::validation::validate_entity_id(&request.run_id, "Ark Code run ID")?;
    let context = code_run_repository_context(&state, run_id)?;
    crate::code_tools::search(
        &context,
        &request.query,
        request.path.as_deref(),
        request.case_sensitive.unwrap_or(false),
        request.max_results,
    )
}

#[tauri::command]
pub fn code_preview_edit_file(
    state: State<'_, AppState>,
    request: CodePreviewEditFileRequest,
) -> Result<crate::code_write_tools::EditFilePreview, AppError> {
    let context = code_repository_context(&state, &request.project_id)?;
    crate::code_write_tools::preview_edit_file(&context, &request.path, request.edits)
}

#[tauri::command]
pub fn code_execute_edit_file(
    state: State<'_, AppState>,
    request: CodeExecuteEditFileRequest,
) -> Result<crate::code_write_tools::EditFileOutcome, AppError> {
    let context = code_repository_context(&state, &request.project_id)?;
    crate::code_write_tools::execute_edit_file(
        &context,
        crate::code_write_tools::ApprovedEditFile {
            path: request.path,
            edits: request.edits,
            call_hash: request.call_hash,
            preview_hash: request.preview_hash,
            precondition_hash: request.precondition_hash,
        },
    )
}

#[tauri::command]
pub async fn code_approve_edit(
    app: AppHandle,
    state: State<'_, AppState>,
    request: CodeApproveEditRequest,
) -> Result<crate::code_sessions::CodeRunDetail, AppError> {
    let session_id =
        crate::validation::validate_entity_id(&request.session_id, "Ark Code session ID")?;
    let run_id = crate::validation::validate_entity_id(&request.run_id, "Ark Code run ID")?;
    let invocation_id = crate::validation::validate_entity_id(
        &request.invocation_id,
        "Ark Code tool invocation ID",
    )?;
    let (run, invocation) = {
        let db = lock_read_db(&state)?;
        let detail = db.get_code_run_detail(run_id)?;
        if detail.run.session_id != session_id {
            return Err(AppError::not_found("Ark Code run"));
        }
        let invocation = detail
            .invocations
            .into_iter()
            .find(|item| item.id == invocation_id)
            .ok_or_else(|| AppError::not_found("Ark Code edit proposal"))?;
        (detail.run, invocation)
    };
    if invocation.tool_name == crate::code_command_tools::RUN_COMMAND_TOOL_ID {
        let arguments: crate::code_command_tools::RunCommandArguments =
            serde_json::from_str(&invocation.canonical_arguments_json).map_err(|_| {
                AppError::new(
                    "code_command_proposal_invalid",
                    "The persisted command proposal could not be decoded safely.",
                )
            })?;
        let definition =
            lock_read_db(&state)?.get_code_command_definition(&arguments.command_id)?;
        let workspace_root = {
            let workspace = state
                .workspace
                .lock()
                .map_err(|_| AppError::new("lock_poisoned", "Workspace state lock poisoned"))?;
            std::path::PathBuf::from(&workspace.root_path)
        };
        let context = crate::code_git_tools::validate_run_repository(
            &run.repository_path_snapshot,
            &workspace_root,
            session_id,
            &run.repository_identity_hash,
        )?;
        let fresh = crate::code_command_tools::preview_command(&context, arguments, definition)?;
        if request.call_hash != invocation.call_hash
            || Some(request.preview_hash.as_str()) != invocation.preview_hash.as_deref()
            || Some(request.precondition_hash.as_str()) != invocation.precondition_hash.as_deref()
            || fresh.call_hash != request.call_hash
            || fresh.preview_hash != request.preview_hash
            || fresh.precondition_hash != request.precondition_hash
            || Some(fresh.content.as_str()) != invocation.preview.as_deref()
        {
            return Err(AppError::new(
                "code_command_approval_stale",
                "The command definition or proposal changed after it was shown.",
            ));
        }
        if state
            .active_code_runs
            .lock()
            .map_err(|_| AppError::new("state_error", "Could not access active Ark Code runs."))?
            .contains_key(run_id)
        {
            return Err(AppError::new(
                "code_run_already_active",
                "This Ark Code run already has an active executor.",
            ));
        }
        let execution_lease_id = uuid::Uuid::new_v4().to_string();
        let execution_lease_expires_at = (Utc::now()
            + ChronoDuration::seconds(i64::from(
                fresh.definition.timeout_seconds.saturating_add(30),
            )))
        .to_rfc3339();
        let verification_plan_json = crate::code_sessions::serialize_json(&serde_json::json!({
            "kind": "command_v1",
            "commandId": fresh.definition.id,
            "definitionPreconditionHash": fresh.precondition_hash,
        }))?;
        lock_db(&state)?.begin_approved_code_edit(&crate::code_sessions::ApproveCodeEdit {
            run_id,
            invocation_id,
            tool_name: crate::code_command_tools::RUN_COMMAND_TOOL_ID,
            call_hash: &request.call_hash,
            preview_hash: &request.preview_hash,
            precondition_hash: &request.precondition_hash,
            execution_lease_id: &execution_lease_id,
            execution_lease_expires_at: &execution_lease_expires_at,
            verification_plan_json: &verification_plan_json,
        })?;
        let cancellation = std::sync::Arc::new(crate::code_agent::CodeRunCancellation::new());
        state
            .active_code_runs
            .lock()
            .map_err(|_| AppError::new("state_error", "Could not access active Ark Code runs."))?
            .insert(run_id.to_string(), cancellation.clone());
        let execution =
            crate::code_command_tools::execute_command(&context, &fresh, &cancellation).await;
        if let Ok(mut active) = state.active_code_runs.lock() {
            active.remove(run_id);
        }
        let (outcome, evidence_json, observation) = match execution {
            Ok(result) => (
                result.outcome,
                crate::code_sessions::serialize_json(&result)?,
                crate::code_sessions::serialize_json(&result)?,
            ),
            Err(error) => (
                crate::code_sessions::CodeRecoveryOutcome::Unknown,
                crate::code_sessions::serialize_json(&serde_json::json!({
                    "executionErrorCode": error.code,
                    "executionError": error.message,
                }))?,
                "Ark could not classify the approved verification command safely. Inspect its effects before continuing."
                    .to_string(),
            ),
        };
        let detail = lock_db(&state)?.finalize_approved_code_edit(
            &crate::code_sessions::FinalizeCodeEdit {
                run_id,
                invocation_id,
                tool_name: crate::code_command_tools::RUN_COMMAND_TOOL_ID,
                execution_lease_id: &execution_lease_id,
                outcome,
                evidence_json: &evidence_json,
                observation_content: &observation,
            },
        )?;
        return if detail.run.state == crate::code_sessions::CodeRunState::Observing {
            crate::code_agent::start_run(app, &state, session_id, run_id)
        } else {
            Ok(detail)
        };
    }
    if invocation.tool_name == crate::code_git_tools::ROLLBACK_TOOL_ID {
        let arguments: crate::code_git_tools::GitRollbackArguments =
            serde_json::from_str(&invocation.canonical_arguments_json).map_err(|_| {
                AppError::new(
                    "code_rollback_proposal_invalid",
                    "The persisted Git rollback proposal could not be decoded safely.",
                )
            })?;
        let (repository, target, checkpoint_oids) = {
            let db = lock_read_db(&state)?;
            let repository = db.get_code_session_repository(session_id)?;
            let target = db.get_code_git_checkpoint(session_id, &arguments.checkpoint_id)?;
            let checkpoint_oids = db
                .list_code_git_checkpoints(session_id)?
                .into_iter()
                .map(|checkpoint| checkpoint.commit_oid)
                .collect::<Vec<_>>();
            (repository, target, checkpoint_oids)
        };
        let workspace_root = {
            let workspace = state
                .workspace
                .lock()
                .map_err(|_| AppError::new("lock_poisoned", "Workspace state lock poisoned"))?;
            std::path::PathBuf::from(&workspace.root_path)
        };
        let context = crate::code_git_tools::validate_run_repository(
            &run.repository_path_snapshot,
            &workspace_root,
            session_id,
            &run.repository_identity_hash,
        )?;
        let fresh = crate::code_git_tools::preview_rollback(
            &context,
            arguments,
            &target.commit_oid,
            &repository.base_commit_oid,
            &checkpoint_oids,
        )
        .await?;
        if request.call_hash != invocation.call_hash
            || Some(request.preview_hash.as_str()) != invocation.preview_hash.as_deref()
            || Some(request.precondition_hash.as_str()) != invocation.precondition_hash.as_deref()
            || fresh.call_hash != request.call_hash
            || fresh.preview_hash != request.preview_hash
            || fresh.precondition_hash != request.precondition_hash
            || Some(fresh.content.as_str()) != invocation.preview.as_deref()
        {
            return Err(AppError::new(
                "code_rollback_approval_stale",
                "Repository state or the rollback proposal changed after it was shown.",
            ));
        }
        let execution_lease_id = uuid::Uuid::new_v4().to_string();
        let execution_lease_expires_at = (Utc::now() + ChronoDuration::seconds(30)).to_rfc3339();
        let verification_plan_json = crate::code_sessions::serialize_json(&serde_json::json!({
            "kind": "git_rollback_v1",
            "beforeHeadOid": fresh.before_head_oid,
            "beforeTreeOid": fresh.before_tree_oid,
            "targetCommitOid": fresh.target_commit_oid,
            "checkpointId": fresh.checkpoint_id,
        }))?;
        lock_db(&state)?.begin_approved_code_edit(&crate::code_sessions::ApproveCodeEdit {
            run_id,
            invocation_id,
            tool_name: crate::code_git_tools::ROLLBACK_TOOL_ID,
            call_hash: &request.call_hash,
            preview_hash: &request.preview_hash,
            precondition_hash: &request.precondition_hash,
            execution_lease_id: &execution_lease_id,
            execution_lease_expires_at: &execution_lease_expires_at,
            verification_plan_json: &verification_plan_json,
        })?;
        let execution = crate::code_git_tools::execute_rollback(
            &context,
            &fresh,
            &repository.base_commit_oid,
            &checkpoint_oids,
        )
        .await;
        let (outcome, evidence_json, observation) = match execution {
            Ok(result) => (
                crate::code_sessions::CodeRecoveryOutcome::Applied,
                crate::code_sessions::serialize_json(&result)?,
                format!(
                    "Ark Code restored its isolated branch to checkpoint {} and verified a clean Repository.",
                    result.checkpoint_id
                ),
            ),
            Err(error) => {
                let outcome = crate::code_git_tools::verify_rollback(
                    &context,
                    &fresh.before_head_oid,
                    &fresh.target_commit_oid,
                )
                .await;
                (
                    outcome,
                    crate::code_sessions::serialize_json(&serde_json::json!({
                        "executionErrorCode": error.code,
                        "executionError": error.message,
                        "verificationOutcome": outcome,
                    }))?,
                    "The approved rollback did not complete normally. Ark verified the isolated branch and stopped safely."
                        .to_string(),
                )
            }
        };
        let detail = lock_db(&state)?.finalize_approved_code_edit(
            &crate::code_sessions::FinalizeCodeEdit {
                run_id,
                invocation_id,
                tool_name: crate::code_git_tools::ROLLBACK_TOOL_ID,
                execution_lease_id: &execution_lease_id,
                outcome,
                evidence_json: &evidence_json,
                observation_content: &observation,
            },
        )?;
        return if detail.run.state == crate::code_sessions::CodeRunState::Observing {
            crate::code_agent::start_run(app, &state, session_id, run_id)
        } else {
            Ok(detail)
        };
    }
    if invocation.tool_name == crate::code_git_tools::CHECKPOINT_TOOL_ID {
        let arguments: crate::code_git_tools::GitCheckpointArguments =
            serde_json::from_str(&invocation.canonical_arguments_json).map_err(|_| {
                AppError::new(
                    "code_checkpoint_proposal_invalid",
                    "The persisted Git checkpoint proposal could not be decoded safely.",
                )
            })?;
        let workspace_root = {
            let workspace = state
                .workspace
                .lock()
                .map_err(|_| AppError::new("lock_poisoned", "Workspace state lock poisoned"))?;
            std::path::PathBuf::from(&workspace.root_path)
        };
        let context = crate::code_git_tools::validate_run_repository(
            &run.repository_path_snapshot,
            &workspace_root,
            session_id,
            &run.repository_identity_hash,
        )?;
        let fresh = crate::code_git_tools::preview_checkpoint(&context, arguments).await?;
        if request.call_hash != invocation.call_hash
            || Some(request.preview_hash.as_str()) != invocation.preview_hash.as_deref()
            || Some(request.precondition_hash.as_str()) != invocation.precondition_hash.as_deref()
            || fresh.call_hash != request.call_hash
            || fresh.preview_hash != request.preview_hash
            || fresh.precondition_hash != request.precondition_hash
            || Some(fresh.content.as_str()) != invocation.preview.as_deref()
        {
            return Err(AppError::new(
                "code_checkpoint_approval_stale",
                "Repository state or the checkpoint proposal changed after it was shown.",
            ));
        }
        let execution_lease_id = uuid::Uuid::new_v4().to_string();
        let execution_lease_expires_at = (Utc::now() + ChronoDuration::seconds(30)).to_rfc3339();
        let verification_plan_json = crate::code_sessions::serialize_json(&serde_json::json!({
            "kind": "git_checkpoint_v1",
            "beforeHeadOid": fresh.head_oid,
            "expectedTreeOid": fresh.tree_oid,
            "branch": format!("refs/heads/ark/session/{session_id}"),
        }))?;
        lock_db(&state)?.begin_approved_code_edit(&crate::code_sessions::ApproveCodeEdit {
            run_id,
            invocation_id,
            tool_name: crate::code_git_tools::CHECKPOINT_TOOL_ID,
            call_hash: &request.call_hash,
            preview_hash: &request.preview_hash,
            precondition_hash: &request.precondition_hash,
            execution_lease_id: &execution_lease_id,
            execution_lease_expires_at: &execution_lease_expires_at,
            verification_plan_json: &verification_plan_json,
        })?;

        let execution = crate::code_git_tools::execute_checkpoint(&context, &fresh).await;
        let (outcome, evidence_json, observation, checkpoint) = match execution {
            Ok(checkpoint) => (
                crate::code_sessions::CodeRecoveryOutcome::Applied,
                crate::code_sessions::serialize_json(&checkpoint)?,
                format!(
                    "Git checkpoint {} was committed and verified on Ark Code's isolated branch.",
                    checkpoint.commit_oid
                ),
                Some(checkpoint),
            ),
            Err(error) => {
                let verification = crate::code_git_tools::verify_checkpoint(
                    &context,
                    &fresh.head_oid,
                    &fresh.tree_oid,
                )
                .await;
                let checkpoint =
                    if verification.outcome == crate::code_sessions::CodeRecoveryOutcome::Applied {
                        verification.observed_head_oid.as_ref().map(|commit_oid| {
                            crate::code_git_tools::GitCheckpointOutcome {
                                commit_oid: commit_oid.clone(),
                                parent_commit_oid: fresh.head_oid.clone(),
                                tree_oid: fresh.tree_oid.clone(),
                                message: fresh.message.clone(),
                            }
                        })
                    } else {
                        None
                    };
                (
                    verification.outcome,
                    crate::code_sessions::serialize_json(&serde_json::json!({
                        "executionErrorCode": error.code,
                        "executionError": error.message,
                        "verification": verification,
                    }))?,
                    "The approved Git checkpoint did not complete normally. Ark verified the isolated branch and stopped safely."
                        .to_string(),
                    checkpoint,
                )
            }
        };
        if let Some(checkpoint) = checkpoint {
            lock_db(&state)?.record_code_git_checkpoint(
                &crate::code_sessions::NewCodeGitCheckpoint {
                    session_id,
                    run_id,
                    invocation_id,
                    commit_oid: &checkpoint.commit_oid,
                    parent_commit_oid: &checkpoint.parent_commit_oid,
                    tree_oid: &checkpoint.tree_oid,
                    message: &checkpoint.message,
                },
            )?;
        }
        let detail = lock_db(&state)?.finalize_approved_code_edit(
            &crate::code_sessions::FinalizeCodeEdit {
                run_id,
                invocation_id,
                tool_name: crate::code_git_tools::CHECKPOINT_TOOL_ID,
                execution_lease_id: &execution_lease_id,
                outcome,
                evidence_json: &evidence_json,
                observation_content: &observation,
            },
        )?;
        return if detail.run.state == crate::code_sessions::CodeRunState::Observing {
            crate::code_agent::start_run(app, &state, session_id, run_id)
        } else {
            Ok(detail)
        };
    }
    if invocation.tool_name != crate::code_write_tools::EDIT_FILE_TOOL_ID {
        return Err(AppError::invalid_input(
            "Only edit_file proposals can be approved here.",
        ));
    }
    let arguments: crate::code_write_tools::EditFileArguments =
        serde_json::from_str(&invocation.canonical_arguments_json).map_err(|_| {
            AppError::new(
                "code_edit_proposal_invalid",
                "The persisted edit proposal could not be decoded safely.",
            )
        })?;
    let workspace_root = {
        let workspace = state
            .workspace
            .lock()
            .map_err(|_| AppError::new("lock_poisoned", "Workspace state lock poisoned"))?;
        std::path::PathBuf::from(&workspace.root_path)
    };
    let context = crate::code_git_tools::validate_run_repository(
        &run.repository_path_snapshot,
        &workspace_root,
        session_id,
        &run.repository_identity_hash,
    )?;
    let fresh = crate::code_write_tools::preview_edit_file(
        &context,
        &arguments.path,
        arguments.edits.clone(),
    )?;
    if request.call_hash != invocation.call_hash
        || Some(request.preview_hash.as_str()) != invocation.preview_hash.as_deref()
        || Some(request.precondition_hash.as_str()) != invocation.precondition_hash.as_deref()
        || fresh.call_hash != request.call_hash
        || fresh.preview_hash != request.preview_hash
        || fresh.precondition_hash != request.precondition_hash
        || Some(fresh.diff.as_str()) != invocation.preview.as_deref()
    {
        return Err(AppError::new(
            "code_edit_approval_stale",
            "The file or proposal changed after this diff was shown. Request a new proposal.",
        ));
    }

    let execution_lease_id = uuid::Uuid::new_v4().to_string();
    let execution_lease_expires_at = (Utc::now() + ChronoDuration::seconds(30)).to_rfc3339();
    let verification_plan_json = crate::code_sessions::serialize_json(&serde_json::json!({
        "kind": "file_hash_v1",
        "path": &fresh.path,
        "beforeHash": &fresh.before_hash,
        "expectedAfterHash": &fresh.expected_after_hash,
    }))?;
    lock_db(&state)?.begin_approved_code_edit(&crate::code_sessions::ApproveCodeEdit {
        run_id,
        invocation_id,
        tool_name: crate::code_write_tools::EDIT_FILE_TOOL_ID,
        call_hash: &request.call_hash,
        preview_hash: &request.preview_hash,
        precondition_hash: &request.precondition_hash,
        execution_lease_id: &execution_lease_id,
        execution_lease_expires_at: &execution_lease_expires_at,
        verification_plan_json: &verification_plan_json,
    })?;

    let execution = crate::code_write_tools::execute_edit_file(
        &context,
        crate::code_write_tools::ApprovedEditFile {
            path: arguments.path,
            edits: arguments.edits,
            call_hash: request.call_hash,
            preview_hash: request.preview_hash,
            precondition_hash: request.precondition_hash,
        },
    );
    let (outcome, evidence_json, observation) = match execution {
        Ok(outcome) => {
            let evidence = crate::code_sessions::serialize_json(&outcome)?;
            let observation = crate::code_sessions::serialize_json(&serde_json::json!({
                "path": outcome.path,
                "outcome": outcome.outcome,
                "observedAfterHash": outcome.observed_after_hash,
            }))?;
            (outcome.outcome, evidence, observation)
        }
        Err(error) => match crate::code_write_tools::verify_edit_file_outcome(
            &context,
            &fresh.path,
            &fresh.before_hash,
            &fresh.expected_after_hash,
        ) {
            Ok(verified) => (
                verified.outcome,
                crate::code_sessions::serialize_json(&serde_json::json!({
                    "executionErrorCode": error.code,
                    "executionError": error.message,
                    "verification": verified,
                }))?,
                "The approved edit attempt did not complete normally. Ark verified the current file state and stopped before continuing."
                    .to_string(),
            ),
            Err(verification_error) => (
                crate::code_sessions::CodeRecoveryOutcome::Unknown,
                crate::code_sessions::serialize_json(&serde_json::json!({
                    "executionErrorCode": error.code,
                    "executionError": error.message,
                    "verificationErrorCode": verification_error.code,
                    "verificationError": verification_error.message,
                }))?,
                "The approved edit could not be verified safely. Inspect the Repository before continuing."
                    .to_string(),
            ),
        },
    };
    let detail =
        lock_db(&state)?.finalize_approved_code_edit(&crate::code_sessions::FinalizeCodeEdit {
            run_id,
            invocation_id,
            tool_name: crate::code_write_tools::EDIT_FILE_TOOL_ID,
            execution_lease_id: &execution_lease_id,
            outcome,
            evidence_json: &evidence_json,
            observation_content: &observation,
        })?;
    if detail.run.state == crate::code_sessions::CodeRunState::Observing {
        crate::code_agent::start_run(app, &state, session_id, run_id)
    } else {
        Ok(detail)
    }
}

#[tauri::command]
pub fn code_reject_edit(
    state: State<'_, AppState>,
    request: CodeRejectEditRequest,
) -> Result<crate::code_sessions::CodeRunDetail, AppError> {
    let session_id =
        crate::validation::validate_entity_id(&request.session_id, "Ark Code session ID")?;
    let run_id = crate::validation::validate_entity_id(&request.run_id, "Ark Code run ID")?;
    let invocation_id = crate::validation::validate_entity_id(
        &request.invocation_id,
        "Ark Code tool invocation ID",
    )?;
    let detail = lock_read_db(&state)?.get_code_run_detail(run_id)?;
    if detail.run.session_id != session_id {
        return Err(AppError::not_found("Ark Code run"));
    }
    lock_db(&state)?.deny_code_edit(run_id, invocation_id, &request.call_hash)
}

#[tauri::command]
pub fn create_code_session(
    state: State<'_, AppState>,
    request: CreateCodeSessionRequest,
) -> Result<crate::code_sessions::CodeSession, AppError> {
    let project_id = crate::validation::validate_entity_id(&request.project_id, "Project ID")?;
    let title = crate::code_sessions::validate_session_title(&request.title)?;
    let request_hash = crate::code_sessions::request_hash(&CodeSessionRequestFingerprint {
        project_id,
        title: &title,
    })?;
    lock_db(&state)?.create_code_session(
        project_id,
        &title,
        &request.idempotency_key,
        &request_hash,
    )
}

#[tauri::command]
pub fn list_code_sessions(
    state: State<'_, AppState>,
    include_archived: Option<bool>,
) -> Result<Vec<crate::code_sessions::CodeSession>, AppError> {
    lock_read_db(&state)?.list_code_sessions(include_archived.unwrap_or(false))
}

#[tauri::command]
pub fn list_code_command_definitions(
    state: State<'_, AppState>,
) -> Result<Vec<crate::code_sessions::CodeCommandDefinition>, AppError> {
    lock_read_db(&state)?.list_code_command_definitions()
}

#[tauri::command]
pub fn save_code_command_definition(
    state: State<'_, AppState>,
    request: SaveCodeCommandDefinitionRequest,
) -> Result<crate::code_sessions::CodeCommandDefinition, AppError> {
    lock_db(&state)?.save_code_command_definition(
        &crate::code_sessions::SaveCodeCommandDefinition {
            id: request.id.as_deref(),
            label: &request.label,
            program: &request.program,
            arguments: &request.arguments,
            timeout_seconds: request.timeout_seconds,
            enabled: request.enabled,
        },
    )
}

#[tauri::command]
pub fn delete_code_command_definition(
    state: State<'_, AppState>,
    id: String,
) -> Result<(), AppError> {
    let id = crate::validation::validate_entity_id(&id, "Command definition ID")?;
    lock_db(&state)?.delete_code_command_definition(id)
}

#[tauri::command]
pub fn get_code_session(
    state: State<'_, AppState>,
    id: String,
) -> Result<crate::code_sessions::CodeSessionDetail, AppError> {
    let id = crate::validation::validate_entity_id(&id, "Ark Code session ID")?;
    lock_read_db(&state)?.get_code_session_detail(id)
}

#[tauri::command]
pub async fn create_code_run(
    state: State<'_, AppState>,
    request: CreateCodeRunRequest,
) -> Result<crate::code_sessions::CodeAgentRun, AppError> {
    let session_id =
        crate::validation::validate_entity_id(&request.session_id, "Ark Code session ID")?;
    let parent_run_id = request
        .parent_run_id
        .as_deref()
        .map(|id| crate::validation::validate_entity_id(id, "Parent Ark Code run ID"))
        .transpose()?;
    let provider_id = crate::validation::validate_entity_id(&request.provider_id, "Provider ID")?;
    let model_id = request.model_id.trim();
    if model_id.is_empty() || model_id.chars().count() > 512 {
        return Err(AppError::invalid_input(
            "Ark Code model ID must be between 1 and 512 characters.",
        ));
    }
    let task = crate::code_sessions::validate_task(&request.task)?;

    let (session, project, provider, models, session_repository) = {
        let db = lock_read_db(&state)?;
        let session = db.get_code_session(session_id)?;
        let project = db.get_project(&session.project_id)?;
        let provider = db.get_provider(provider_id)?;
        let models = db.list_models(provider_id)?;
        let session_repository = db.find_code_session_repository(session_id)?;
        (session, project, provider, models, session_repository)
    };
    if !provider.is_enabled {
        return Err(AppError::new(
            "provider_disabled",
            "Enable the selected provider before starting Ark Code.",
        ));
    }
    let model = models
        .iter()
        .find(|model| model.name == model_id && model.is_available)
        .ok_or_else(|| {
            AppError::new(
                "provider_model_unavailable",
                "Refresh models and select an available model before starting Ark Code.",
            )
        })?;
    if model.tool_calling_mode == crate::providers::ToolCallingMode::Unsupported {
        return Err(AppError::new(
            "model_tools_unsupported",
            "This model does not support Ark Code tool calling. Choose a native or prompted-tool model.",
        ));
    }
    let model_context_window = model
        .context_window
        .and_then(|value| u64::try_from(value).ok())
        .filter(|value| *value >= 512)
        .ok_or_else(|| {
            AppError::new(
                "model_context_window_unknown",
                "Ark Code needs the selected model's real context window. Refresh model metadata or choose a model that reports it.",
            )
        })?;

    let workspace_root = {
        let workspace = state
            .workspace
            .lock()
            .map_err(|_| AppError::new("lock_poisoned", "Workspace state lock poisoned"))?;
        std::path::PathBuf::from(&workspace.root_path)
    };
    let (context, branch_name, base_commit_oid) = if let Some(repository) = session_repository {
        let context = crate::code_git_tools::validate_run_repository(
            &repository.root_path,
            &workspace_root,
            &session.id,
            &repository.repository_identity_hash,
        )?;
        let branch = crate::code_git_tools::run_managed_git(
            &context,
            &["branch", "--show-current"],
            std::time::Duration::from_secs(10),
        )
        .await?;
        if branch.trim() != repository.branch_name {
            return Err(AppError::new(
                "code_repository_branch_changed",
                "Ark Code's managed Repository is no longer on its dedicated session branch.",
            ));
        }
        (context, repository.branch_name, repository.base_commit_oid)
    } else {
        let source_context = crate::code_tools::RepositoryContext::from_project(&project)?;
        let context = crate::code_git_tools::provision_session_repository(
            &source_context,
            &workspace_root,
            &session.id,
        )
        .await?;
        let branch_name = format!("ark/session/{}", session.id);
        let base_commit_oid = crate::code_git_tools::run_managed_git(
            &context,
            &["rev-parse", "--verify", "HEAD"],
            std::time::Duration::from_secs(10),
        )
        .await?
        .trim()
        .to_string();
        (context, branch_name, base_commit_oid)
    };
    let (repository_path, repository_identity_hash) =
        crate::code_sessions::repository_snapshot(context.root())?;
    lock_db(&state)?.ensure_code_session_repository(
        &crate::code_sessions::NewCodeSessionRepository {
            session_id: &session.id,
            root_path: &repository_path,
            repository_identity_hash: &repository_identity_hash,
            branch_name: &branch_name,
            base_commit_oid: &base_commit_oid,
        },
    )?;
    let max_steps = request
        .max_steps
        .unwrap_or(crate::code_sessions::DEFAULT_MAX_STEPS);
    let max_active_ms = request
        .max_active_ms
        .unwrap_or(crate::code_sessions::DEFAULT_MAX_ACTIVE_MS);
    let max_tokens = request.max_tokens.unwrap_or(model_context_window);
    crate::code_sessions::validate_run_budgets(max_steps, max_active_ms, max_tokens)?;
    let request_hash = crate::code_sessions::request_hash(&CodeRunRequestFingerprint {
        session_id: &session.id,
        parent_run_id,
        provider_id,
        model_id,
        task: &task,
        repository_identity_hash: &repository_identity_hash,
        max_steps,
        max_active_ms,
        max_tokens,
        max_cost_microunits: request.max_cost_microunits,
    })?;
    lock_db(&state)?.create_code_agent_run(&crate::code_sessions::NewCodeRun {
        session_id: &session.id,
        parent_run_id,
        provider_id,
        model_id,
        task: &task,
        repository_path_snapshot: &repository_path,
        repository_identity_hash: &repository_identity_hash,
        max_steps,
        max_active_ms,
        max_tokens,
        max_cost_microunits: request.max_cost_microunits,
        idempotency_key: &request.idempotency_key,
        request_hash: &request_hash,
    })
}

#[tauri::command]
pub async fn initialize_project_git_repository(
    state: State<'_, AppState>,
    project_id: String,
) -> Result<(), AppError> {
    let project_id = crate::validation::validate_entity_id(&project_id, "Project ID")?;
    let project = lock_read_db(&state)?.get_project(project_id)?;
    let context = crate::code_tools::RepositoryContext::from_project(&project)?;
    crate::code_git_tools::initialize_project_repository(&context).await
}

/// CODE-007 development seam: drives exactly one model turn of an existing
/// `queued`/`observing` run. Production UI uses `start_code_agent_run` instead.
#[tauri::command]
pub async fn run_code_agent_step(
    state: State<'_, AppState>,
    session_id: String,
    run_id: String,
) -> Result<crate::code_sessions::CodeRunDetail, AppError> {
    let session_id = crate::validation::validate_entity_id(&session_id, "Ark Code session ID")?;
    let run_id = crate::validation::validate_entity_id(&run_id, "Ark Code run ID")?;
    crate::code_agent::run_step(&state, session_id, run_id).await
}

/// CODE-007 production path: starts the durable automatic loop and returns immediately. The
/// single-step command above remains available only for deterministic tests/development tooling.
#[tauri::command]
pub fn start_code_agent_run(
    app: AppHandle,
    state: State<'_, AppState>,
    session_id: String,
    run_id: String,
) -> Result<crate::code_sessions::CodeRunDetail, AppError> {
    let session_id = crate::validation::validate_entity_id(&session_id, "Ark Code session ID")?;
    let run_id = crate::validation::validate_entity_id(&run_id, "Ark Code run ID")?;
    crate::code_agent::start_run(app, &state, session_id, run_id)
}

#[tauri::command]
pub fn cancel_code_agent_run(
    app: AppHandle,
    state: State<'_, AppState>,
    session_id: String,
    run_id: String,
) -> Result<crate::code_sessions::CodeRunDetail, AppError> {
    let session_id = crate::validation::validate_entity_id(&session_id, "Ark Code session ID")?;
    let run_id = crate::validation::validate_entity_id(&run_id, "Ark Code run ID")?;
    crate::code_agent::cancel_run(&app, &state, session_id, run_id)
}

#[tauri::command]
pub fn get_code_run_detail(
    state: State<'_, AppState>,
    run_id: String,
) -> Result<crate::code_sessions::CodeRunDetail, AppError> {
    let run_id = crate::validation::validate_entity_id(&run_id, "Ark Code run ID")?;
    lock_read_db(&state)?.get_code_run_detail(run_id)
}

fn code_repository_context(
    state: &AppState,
    project_id: &str,
) -> Result<crate::code_tools::RepositoryContext, AppError> {
    let project_id = crate::validation::validate_entity_id(project_id, "Project ID")?;
    let project = lock_read_db(state)?.get_project(project_id)?;
    crate::code_tools::RepositoryContext::from_project(&project)
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
pub fn set_conversation_persona(
    state: State<'_, AppState>,
    id: String,
    persona_id: Option<String>,
) -> Result<crate::chat::Conversation, AppError> {
    let id = crate::validation::validate_entity_id(&id, "Conversation ID")?.to_string();
    let persona_id = persona_id
        .as_deref()
        .map(|value| crate::validation::validate_entity_id(value, "Persona ID"))
        .transpose()?
        .map(str::to_string);
    lock_db(&state)?.set_conversation_persona(&id, persona_id.as_deref())
}

#[tauri::command]
pub fn list_personas(
    state: State<'_, AppState>,
) -> Result<Vec<crate::personas::Persona>, AppError> {
    lock_read_db(&state)?.list_personas()
}

#[tauri::command]
pub fn create_persona(
    state: State<'_, AppState>,
    request: CreatePersonaRequest,
) -> Result<crate::personas::Persona, AppError> {
    let instructions = crate::validation::validate_persona_instructions(&request.instructions)?;
    let temperature = crate::validation::validate_temperature(request.default_temperature)?;
    let max_tokens = crate::validation::validate_max_tokens(request.default_max_tokens)?;
    let response_style = crate::validation::validate_response_style(request.response_style)?;
    let tone = crate::validation::validate_tone(request.tone)?;
    lock_db(&state)?.create_persona(
        &request.name,
        &instructions,
        temperature,
        max_tokens,
        response_style.as_deref(),
        tone.as_deref(),
    )
}

#[tauri::command]
pub fn update_persona(
    state: State<'_, AppState>,
    request: UpdatePersonaRequest,
) -> Result<crate::personas::Persona, AppError> {
    let id = crate::validation::validate_entity_id(&request.id, "Persona ID")?.to_string();
    let instructions = crate::validation::validate_persona_instructions(&request.instructions)?;
    let temperature = crate::validation::validate_temperature(request.default_temperature)?;
    let max_tokens = crate::validation::validate_max_tokens(request.default_max_tokens)?;
    let response_style = crate::validation::validate_response_style(request.response_style)?;
    let tone = crate::validation::validate_tone(request.tone)?;
    lock_db(&state)?.update_persona(
        &id,
        &request.name,
        &instructions,
        temperature,
        max_tokens,
        response_style.as_deref(),
        tone.as_deref(),
    )
}

#[tauri::command]
pub fn list_persona_versions(
    state: State<'_, AppState>,
    id: String,
) -> Result<Vec<crate::personas::PersonaVersionSummary>, AppError> {
    let id = crate::validation::validate_entity_id(&id, "Persona ID")?;
    lock_read_db(&state)?.list_persona_versions(id)
}

#[tauri::command]
pub fn export_persona_json(state: State<'_, AppState>, id: String) -> Result<String, AppError> {
    let id = crate::validation::validate_entity_id(&id, "Persona ID")?;
    let db = lock_read_db(&state)?;
    crate::personas::export_persona_json(&db, id)
}

#[tauri::command]
pub fn import_persona_json(
    state: State<'_, AppState>,
    json: String,
) -> Result<crate::personas::Persona, AppError> {
    let db = lock_db(&state)?;
    crate::personas::import_persona_json(&db, &json)
}

#[tauri::command]
pub fn set_persona_archived(
    state: State<'_, AppState>,
    id: String,
    archived: bool,
) -> Result<crate::personas::Persona, AppError> {
    let id = crate::validation::validate_entity_id(&id, "Persona ID")?;
    lock_db(&state)?.set_persona_archived(id, archived)
}

#[tauri::command]
pub fn preview_persona_deletion(
    state: State<'_, AppState>,
    id: String,
) -> Result<crate::personas::PersonaDeletionPreview, AppError> {
    let id = crate::validation::validate_entity_id(&id, "Persona ID")?;
    lock_read_db(&state)?.preview_persona_deletion(id)
}

#[tauri::command]
pub fn delete_persona(state: State<'_, AppState>, id: String) -> Result<(), AppError> {
    let id = crate::validation::validate_entity_id(&id, "Persona ID")?;
    lock_db(&state)?.delete_persona(id)
}

/// CMP-001: stages a new text attachment against `conversation_id` — the message it will
/// eventually be sent with doesn't exist yet, matching the "preview/remove before send"
/// acceptance criterion. `validate_attachment` is the content-sniffing/size boundary: a
/// `.txt`-named file whose bytes don't actually decode as plausible text is rejected here
/// regardless of what its name claims.
#[tauri::command]
pub fn attach_text_file(
    state: State<'_, AppState>,
    conversation_id: String,
    file_name: String,
    content: String,
) -> Result<crate::attachments::Attachment, AppError> {
    let conversation_id =
        crate::validation::validate_entity_id(&conversation_id, "Conversation ID")?.to_string();
    let (file_name, content) = crate::validation::validate_attachment(&file_name, &content)?;
    lock_db(&state)?.create_attachment(&conversation_id, &file_name, &content)
}

#[tauri::command]
pub fn list_conversation_attachments(
    state: State<'_, AppState>,
    conversation_id: String,
) -> Result<Vec<crate::attachments::Attachment>, AppError> {
    let conversation_id =
        crate::validation::validate_entity_id(&conversation_id, "Conversation ID")?;
    lock_read_db(&state)?.list_conversation_attachments(conversation_id)
}

#[tauri::command]
pub fn get_attachment_content(state: State<'_, AppState>, id: String) -> Result<String, AppError> {
    let id = crate::validation::validate_entity_id(&id, "Attachment ID")?;
    lock_read_db(&state)?.get_attachment_content(id)
}

/// CMP-001: only ever succeeds while the attachment is still staged (`messageId` still `null`) —
/// see `Database::delete_attachment`'s own doc comment for why one already linked to a sent
/// message is not offered for deletion.
#[tauri::command]
pub fn delete_attachment(state: State<'_, AppState>, id: String) -> Result<(), AppError> {
    let id = crate::validation::validate_entity_id(&id, "Attachment ID")?;
    lock_db(&state)?.delete_attachment(id)
}

/// CMP-003: the first real consumer of `tool_policy`/`tools`. See `tools.rs`'s own module doc for
/// what this deliberately does and does not cover — one built-in, user-triggered, chat-safe tool
/// ("notes"), not a real MCP protocol client or LLM-autonomous agent loop.
#[tauri::command]
pub fn list_tools(state: State<'_, AppState>) -> Result<Vec<crate::tools::ToolStatus>, AppError> {
    let db = lock_read_db(&state)?;
    let now_ts = crate::db::now();
    crate::tools::built_in_tools()
        .into_iter()
        .map(|definition| {
            let active_grant = db
                .get_active_grant_for_tool(&definition.id)?
                .filter(|grant| grant.is_valid_at(&now_ts));
            Ok(crate::tools::ToolStatus {
                definition,
                active_grant,
            })
        })
        .collect()
}

#[tauri::command]
pub fn grant_tool_capability(
    state: State<'_, AppState>,
    request: GrantToolCapabilityRequest,
) -> Result<crate::tools::ToolCapabilityGrant, AppError> {
    let tool_id = crate::validation::validate_entity_id(&request.tool_id, "Tool ID")?.to_string();
    let ttl_minutes = crate::validation::validate_grant_ttl_minutes(request.ttl_minutes)?;
    let tool = crate::tools::built_in_tools()
        .into_iter()
        .find(|tool| tool.id == tool_id)
        .ok_or_else(|| AppError::not_found("Tool"))?;
    lock_db(&state)?.create_capability_grant(&tool_id, &tool.scope, ttl_minutes)
}

#[tauri::command]
pub fn revoke_tool_capability(state: State<'_, AppState>, id: String) -> Result<(), AppError> {
    let id = crate::validation::validate_entity_id(&id, "Grant ID")?;
    lock_db(&state)?.revoke_capability_grant(id)
}

#[tauri::command]
pub fn list_tool_audit_events(
    state: State<'_, AppState>,
) -> Result<Vec<crate::tool_policy::AuditEvent>, AppError> {
    lock_read_db(&state)?.list_audit_events()
}

/// SEC-009's tamper-evidence property, made checkable from the UI: recomputes the persisted
/// chain's hashes from scratch and confirms they match what is stored. `true` means the trail is
/// genuinely unmodified since it was written, not just present.
#[tauri::command]
pub fn verify_tool_audit_trail(state: State<'_, AppState>) -> Result<bool, AppError> {
    let events = lock_read_db(&state)?.list_audit_events()?;
    Ok(crate::tool_policy::verify_audit_chain(&events))
}

#[tauri::command]
pub fn list_conversation_notes(
    state: State<'_, AppState>,
    conversation_id: String,
) -> Result<Vec<crate::tools::ConversationNote>, AppError> {
    let conversation_id =
        crate::validation::validate_entity_id(&conversation_id, "Conversation ID")?;
    lock_read_db(&state)?.list_conversation_notes(conversation_id)
}

#[tauri::command]
pub fn preview_note_write(
    request: PreviewNoteWriteRequest,
) -> Result<crate::tool_policy::SideEffectPreview, AppError> {
    let content = match request.action {
        crate::tools::NoteWriteAction::Delete => None,
        crate::tools::NoteWriteAction::Create | crate::tools::NoteWriteAction::Update => {
            Some(crate::validation::validate_note_content(
                request.content.as_deref().unwrap_or_default(),
            )?)
        }
    };
    Ok(crate::tools::preview_note_write(
        request.action,
        content.as_deref(),
    ))
}

fn approval_required_error() -> AppError {
    AppError::new(
        "approval_required",
        "This action needs approval — preview it and grant access first.",
    )
}

#[tauri::command]
pub fn create_note(
    state: State<'_, AppState>,
    request: CreateNoteRequest,
) -> Result<crate::tools::ConversationNote, AppError> {
    let conversation_id =
        crate::validation::validate_entity_id(&request.conversation_id, "Conversation ID")?
            .to_string();
    let content = crate::validation::validate_note_content(&request.content)?;
    let db = lock_db(&state)?;
    match crate::tools::authorize_note_write(&db, request.approve)? {
        crate::tools::NoteWriteAttempt::ApprovalRequired => Err(approval_required_error()),
        crate::tools::NoteWriteAttempt::Applied => {
            let note = db.create_note(&conversation_id, &content)?;
            db.record_tool_invocation("notes", "created a note")?;
            Ok(note)
        }
    }
}

#[tauri::command]
pub fn update_note(
    state: State<'_, AppState>,
    request: UpdateNoteRequest,
) -> Result<crate::tools::ConversationNote, AppError> {
    let id = crate::validation::validate_entity_id(&request.id, "Note ID")?.to_string();
    let content = crate::validation::validate_note_content(&request.content)?;
    let db = lock_db(&state)?;
    match crate::tools::authorize_note_write(&db, request.approve)? {
        crate::tools::NoteWriteAttempt::ApprovalRequired => Err(approval_required_error()),
        crate::tools::NoteWriteAttempt::Applied => {
            let note = db.update_note(&id, &content)?;
            db.record_tool_invocation("notes", "updated a note")?;
            Ok(note)
        }
    }
}

#[tauri::command]
pub fn delete_note(state: State<'_, AppState>, request: DeleteNoteRequest) -> Result<(), AppError> {
    let id = crate::validation::validate_entity_id(&request.id, "Note ID")?.to_string();
    let db = lock_db(&state)?;
    match crate::tools::authorize_note_write(&db, request.approve)? {
        crate::tools::NoteWriteAttempt::ApprovalRequired => Err(approval_required_error()),
        crate::tools::NoteWriteAttempt::Applied => {
            db.delete_note(&id)?;
            db.record_tool_invocation("notes", "deleted a note")?;
            Ok(())
        }
    }
}

#[tauri::command]
pub fn preview_web_search(
    request: PreviewWebSearchRequest,
) -> Result<crate::tool_policy::SideEffectPreview, AppError> {
    let query = crate::validation::validate_search_query(&request.query)?;
    Ok(crate::web_search::preview_web_search(&query))
}

#[tauri::command]
pub async fn search_web(
    state: State<'_, AppState>,
    request: SearchWebRequest,
) -> Result<crate::web_search::WebSearchResult, AppError> {
    let query = crate::validation::validate_search_query(&request.query)?;
    match crate::web_search::search_web(&state, query, request.approve).await? {
        crate::web_search::WebSearchOutcome::ApprovalRequired => Err(approval_required_error()),
        crate::web_search::WebSearchOutcome::Applied(result) => Ok(result),
    }
}

#[tauri::command]
pub async fn upsert_tool_secret(
    state: State<'_, AppState>,
    tool_id: String,
    secret: String,
) -> Result<SecretMetadata, AppError> {
    crate::secret_store::upsert_tool_secret(&state, tool_id, secret).await
}

#[tauri::command]
pub async fn get_tool_secret_metadata(
    state: State<'_, AppState>,
    tool_id: String,
) -> Result<Option<SecretMetadata>, AppError> {
    crate::secret_store::get_tool_secret_metadata(&state, tool_id).await
}

#[tauri::command]
pub async fn delete_tool_secret(
    state: State<'_, AppState>,
    tool_id: String,
) -> Result<(), AppError> {
    crate::secret_store::delete_tool_secret(&state, tool_id).await
}

/// PERF-003: `depth_limit` bounds how far back the active path is walked (see
/// `Database::get_active_messages_page`) — the frontend always passes an explicit page size
/// (an initial load, or a larger one after "Load earlier messages"), rather than this command
/// defaulting to the unbounded `get_active_messages` every other Rust caller still uses.
#[tauri::command]
pub fn get_conversation_messages(
    state: State<'_, AppState>,
    conversation_id: String,
    depth_limit: i64,
) -> Result<ConversationMessagePage, AppError> {
    // ARC-004: read-hot, called every time the user switches conversations — see
    // `list_conversations` above for why this goes through the read replica.
    let conversation_id =
        crate::validation::validate_entity_id(&conversation_id, "Conversation ID")?;
    let (messages, has_more_older) =
        lock_read_db(&state)?.get_active_messages_page(conversation_id, depth_limit)?;
    Ok(ConversationMessagePage {
        messages,
        has_more_older,
    })
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
pub fn get_conversation_branch_topology(
    state: State<'_, AppState>,
    conversation_id: String,
) -> Result<Vec<crate::chat::BranchTopologyNode>, AppError> {
    let conversation_id =
        crate::validation::validate_entity_id(&conversation_id, "Conversation ID")?;
    lock_read_db(&state)?.get_branch_topology(conversation_id)
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

#[tauri::command]
pub fn create_remote_provider(
    state: State<'_, AppState>,
    request: CreateRemoteProviderRequest,
) -> Result<ProviderConfig, AppError> {
    crate::provider_management::create_remote_provider(&state, request)
}

#[tauri::command]
pub async fn delete_provider(
    state: State<'_, AppState>,
    provider_id: String,
    confirmed: bool,
) -> Result<(), AppError> {
    crate::provider_management::delete_provider(&state, provider_id, confirmed).await
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

/// FTR-003: updates the workspace-wide instruction fallback. Blank input clears the setting;
/// validation matches every other free-form system-instruction tier.
#[tauri::command]
pub fn update_application_instructions(
    state: State<'_, AppState>,
    instructions: Option<String>,
) -> Result<Option<String>, AppError> {
    let instructions = crate::validation::validate_system_prompt(instructions)?;
    let db = lock_db(&state)?;
    if let Some(value) = instructions.as_deref() {
        db.set_setting(crate::config::APPLICATION_INSTRUCTIONS_SETTING_KEY, value)?;
    } else {
        db.delete_setting(crate::config::APPLICATION_INSTRUCTIONS_SETTING_KEY)?;
    }
    Ok(instructions)
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
    app: AppHandle,
    state: State<'_, AppState>,
    provider_id: String,
) -> Result<RefreshModelsResult, AppError> {
    crate::provider_management::refresh_models(&app, &state, provider_id).await
}

#[tauri::command]
pub fn cancel_provider_refresh(
    state: State<'_, AppState>,
    provider_id: String,
) -> Result<(), AppError> {
    crate::provider_management::cancel_provider_refresh(&state, provider_id)
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

/// PERF-001: the one frontend-originated performance metric — how long the frontend's own
/// bootstrap took, from navigation start to the shell becoming interactive (see
/// `useArkController.ts::bootstrap`'s `finally` block). `name` is checked against a fixed
/// allowlist rather than accepted as free text: unlike every other metric in this pass (all
/// recorded directly by Rust code that controls what it names), this one crosses the IPC
/// boundary from a caller that could in principle pass anything, so it gets the same "centralize
/// native input validation" treatment (COR-008) as any other command argument.
const ALLOWED_FRONTEND_METRIC_NAMES: &[&str] = &["cached_shell_ms"];

#[tauri::command]
pub fn record_frontend_perf_metric(
    app: AppHandle,
    state: State<'_, AppState>,
    name: String,
    value_ms: f64,
) -> Result<(), AppError> {
    if !ALLOWED_FRONTEND_METRIC_NAMES.contains(&name.as_str()) {
        return Err(AppError::invalid_input("Unknown performance metric name."));
    }
    crate::perf_metrics::record_if_enabled(
        &app,
        &state,
        "perf.frontend",
        None,
        &[(name.as_str(), value_ms.round().to_string())],
    );
    Ok(())
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

/// FTR-008: `project_id: None` exports every conversation in the workspace.
#[tauri::command]
pub fn export_workspace_json(
    state: State<'_, AppState>,
    project_id: Option<String>,
) -> Result<String, AppError> {
    let project_id = project_id
        .as_deref()
        .map(|value| crate::validation::validate_entity_id(value, "Project ID"))
        .transpose()?;
    let db = lock_db(&state)?;
    crate::import_export::export_workspace_json(&db, project_id)
}

#[tauri::command]
pub fn export_workspace_markdown(
    state: State<'_, AppState>,
    project_id: Option<String>,
) -> Result<String, AppError> {
    let project_id = project_id
        .as_deref()
        .map(|value| crate::validation::validate_entity_id(value, "Project ID"))
        .transpose()?;
    let db = lock_db(&state)?;
    crate::import_export::export_workspace_markdown(&db, project_id)
}

#[tauri::command]
pub fn preview_workspace_import(
    state: State<'_, AppState>,
    json: String,
) -> Result<crate::import_export::WorkspaceImportPreview, AppError> {
    let db = lock_db(&state)?;
    crate::import_export::preview_workspace_import(&db, &json)
}

#[tauri::command]
pub fn import_workspace_json(
    state: State<'_, AppState>,
    json: String,
    include_conversation_ids: Vec<String>,
) -> Result<crate::import_export::WorkspaceImportResult, AppError> {
    let db = lock_db(&state)?;
    crate::import_export::import_workspace_json(
        &db,
        &json,
        &include_conversation_ids.into_iter().collect(),
    )
}

#[tauri::command]
pub fn get_companion_api_status(
    app: AppHandle,
) -> Result<crate::companion_api::CompanionApiStatus, AppError> {
    crate::companion_api::get_status(&app)
}

#[tauri::command]
pub async fn set_companion_api_enabled(
    app: AppHandle,
    enabled: bool,
) -> Result<crate::companion_api::CompanionApiStatus, AppError> {
    crate::companion_api::set_enabled(&app, enabled).await
}

#[tauri::command]
pub async fn regenerate_companion_api_token(
    app: AppHandle,
) -> Result<crate::companion_api::CompanionApiTokenReveal, AppError> {
    crate::companion_api::regenerate_token(&app).await
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

#[tauri::command]
pub fn check_disk_space(
    state: State<'_, AppState>,
) -> Result<crate::provider_management::DiskSpaceInfo, AppError> {
    crate::provider_management::check_disk_space(&state)
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

#[tauri::command]
pub fn list_managed_models(app: AppHandle) -> Result<Vec<ManagedModelStatus>, AppError> {
    crate::managed_models::list_managed_models(&app)
}

#[tauri::command]
pub fn preflight_managed_model(
    model_id: String,
    operation: ManagedModelOperation,
    app: AppHandle,
) -> Result<ManagedModelPreflight, AppError> {
    crate::managed_models::preflight_managed_model(&app, &model_id, operation)
}

#[tauri::command]
pub fn get_hardware_fit_evidence() -> crate::managed_models::HardwareFitEvidence {
    crate::managed_models::local_hardware_fit_evidence()
}

#[tauri::command]
pub async fn download_managed_model(
    request: ManagedModelDownloadRequest,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<ManagedModelStatus, AppError> {
    crate::managed_models::download_managed_model(&app, &state, request).await
}

#[tauri::command]
pub fn cancel_managed_model_download(
    model_id: String,
    state: State<'_, AppState>,
) -> Result<(), AppError> {
    crate::managed_models::cancel_managed_model_download(&state, &model_id)
}

#[tauri::command]
pub fn delete_managed_model(
    model_id: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), AppError> {
    crate::managed_models::delete_managed_model(&app, &state, &model_id)
}

#[tauri::command]
pub async fn start_managed_model(
    request: StartManagedModelRequest,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<BuiltInRuntimeStatus, AppError> {
    let (model, path) = crate::managed_models::authorize_start(&app, &request)?;
    crate::provider_management::start_built_in_runtime_with_expected_digest(
        path.display().to_string(),
        model.source_repository,
        model.license,
        Some(&model.sha256),
        &app,
        &state,
    )
    .await
}
