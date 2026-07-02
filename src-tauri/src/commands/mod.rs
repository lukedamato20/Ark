use crate::chat::{BranchAlternative, ChatMessage, Message, SendChatRequest, SendChatResult, StreamEvent};
use crate::config::DEFAULT_PROVIDER_ID;
use crate::db::{now, Database};
use crate::errors::AppError;
use crate::export::{
    conversation_to_markdown, validate_conversation_export, ConversationExport, CONVERSATION_EXPORT_SCHEMA_VERSION,
};
use crate::providers::{ModelInfo, ProviderChatRequest, ProviderConfig, ProviderHealth, ProviderRuntime};
use crate::workspace::WorkspaceInfo;
use crate::AppState;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;
use sysinfo::{Disks, System};
use tauri::{AppHandle, Emitter, Manager, State};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppBootstrap {
    pub conversations: Vec<crate::chat::Conversation>,
    pub providers: Vec<ProviderConfig>,
    pub models: Vec<ModelInfo>,
    pub workspace_path: String,
    pub workspace: WorkspaceInfo,
    pub theme: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenameConversationRequest {
    pub id: String,
    pub title: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditUserMessageRequest {
    pub conversation_id: String,
    pub message_id: String,
    pub content: String,
    pub provider_id: String,
    pub model: String,
    pub temperature: Option<f64>,
    pub max_tokens: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegenerateAssistantMessageRequest {
    pub conversation_id: String,
    pub message_id: String,
    pub provider_id: String,
    pub model: String,
    pub temperature: Option<f64>,
    pub max_tokens: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssistantBranchRequest {
    pub conversation_id: String,
    pub message_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProviderRequest {
    pub provider_id: String,
    pub base_url: String,
    pub default_model_id: Option<String>,
    pub temperature: Option<f64>,
    pub max_tokens: Option<i64>,
    pub streaming_enabled: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RefreshModelsResult {
    pub health: ProviderHealth,
    pub models: Vec<ModelInfo>,
    pub provider: ProviderConfig,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticsResult {
    pub os: String,
    pub cpu: String,
    pub cpu_cores: usize,
    pub total_memory_bytes: u64,
    pub available_memory_bytes: u64,
    pub total_disk_bytes: u64,
    pub available_disk_bytes: u64,
    pub gpu: String,
    pub provider_health: ProviderHealth,
    pub model_available: bool,
    pub benchmark: Option<BenchmarkResult>,
    pub guidance: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BenchmarkResult {
    pub time_to_first_token_ms: Option<u128>,
    pub total_time_ms: u128,
    pub approximate_tokens_per_second: Option<f64>,
    pub output_preview: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetThemeRequest {
    pub theme: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetWorkspaceRequest {
    pub root_path: String,
}

#[tauri::command]
pub fn get_app_bootstrap(app: AppHandle, state: State<'_, AppState>) -> Result<AppBootstrap, AppError> {
    let workspace = crate::workspace::resolve_default_workspace(&app)?;
    let workspace_info = workspace.info();
    let db = lock_db(&state)?;
    let providers = db.list_providers()?;
    let models = db.list_all_models()?;
    let theme = db.get_setting("appearance.theme")?.unwrap_or_else(|| "dark".to_string());

    Ok(AppBootstrap {
        conversations: db.list_conversations()?,
        providers,
        models,
        workspace_path: workspace.database_path().display().to_string(),
        workspace: workspace_info,
        theme,
    })
}

#[tauri::command]
pub fn list_conversations(state: State<'_, AppState>) -> Result<Vec<crate::chat::Conversation>, AppError> {
    lock_db(&state)?.list_conversations()
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
    lock_db(&state)?.rename_conversation(&request.id, &request.title)
}

#[tauri::command]
pub fn delete_conversation(state: State<'_, AppState>, id: String) -> Result<(), AppError> {
    lock_db(&state)?.delete_conversation(&id)
}

#[tauri::command]
pub fn get_conversation_messages(
    state: State<'_, AppState>,
    conversation_id: String,
) -> Result<Vec<Message>, AppError> {
    lock_db(&state)?.get_active_messages(&conversation_id)
}

#[tauri::command]
pub fn get_assistant_alternatives(
    state: State<'_, AppState>,
    request: AssistantBranchRequest,
) -> Result<Vec<BranchAlternative>, AppError> {
    lock_db(&state)?.get_assistant_alternatives(&request.conversation_id, &request.message_id)
}

#[tauri::command]
pub fn switch_active_branch(
    state: State<'_, AppState>,
    request: AssistantBranchRequest,
) -> Result<Vec<Message>, AppError> {
    lock_db(&state)?.switch_active_branch(&request.conversation_id, &request.message_id)
}

#[tauri::command]
pub fn update_provider(
    state: State<'_, AppState>,
    request: UpdateProviderRequest,
) -> Result<ProviderConfig, AppError> {
    lock_db(&state)?.update_provider(
        &request.provider_id,
        &request.base_url,
        request.default_model_id.as_deref(),
        request.temperature,
        request.max_tokens,
        request.streaming_enabled,
    )
}

#[tauri::command]
pub fn set_theme(state: State<'_, AppState>, request: SetThemeRequest) -> Result<String, AppError> {
    if request.theme != "dark" && request.theme != "light" {
        return Err(AppError::invalid_input("Theme must be dark or light."));
    }

    lock_db(&state)?.set_setting("appearance.theme", &request.theme)?;
    Ok(request.theme)
}

#[tauri::command]
pub fn set_workspace(app: AppHandle, request: SetWorkspaceRequest) -> Result<WorkspaceInfo, AppError> {
    crate::workspace::set_workspace_root(&app, &request.root_path)
}

#[tauri::command]
pub fn reset_workspace(app: AppHandle) -> Result<WorkspaceInfo, AppError> {
    crate::workspace::reset_workspace_root(&app)
}

#[tauri::command]
pub async fn refresh_models(
    state: State<'_, AppState>,
    provider_id: String,
) -> Result<RefreshModelsResult, AppError> {
    let provider = {
        let db = lock_db(&state)?;
        db.get_provider(&provider_id)?
    };

    let runtime = ProviderRuntime::from_config(provider.clone())?;
    let health = runtime.health().await;

    if !health.is_reachable {
        return Ok(RefreshModelsResult {
            health,
            models: Vec::new(),
            provider,
        });
    }

    let models = runtime.list_models(&now()).await?;

    let provider = {
        let db = lock_db(&state)?;
        db.upsert_models(&provider_id, &models)?;
        db.get_provider(&provider_id)?
    };

    Ok(RefreshModelsResult {
        health,
        models,
        provider,
    })
}

#[tauri::command]
pub fn send_chat_message(
    app: AppHandle,
    state: State<'_, AppState>,
    request: SendChatRequest,
) -> Result<SendChatResult, AppError> {
    let content = request.content.trim();
    if content.is_empty() {
        return Err(AppError::invalid_input("Message cannot be empty."));
    }

    let (provider, provider_request, result) = {
        let db = lock_db(&state)?;
        let conversation = db.get_conversation(&request.conversation_id)?;
        let provider = db.get_provider(&request.provider_id)?;
        let active_messages = db.get_active_messages(&request.conversation_id)?;
        let parent_id = conversation.current_message_id.as_deref();

        let user_message = db.append_message(
            &request.conversation_id,
            parent_id,
            None,
            "user",
            content,
            "complete",
            Some(&request.provider_id),
            Some(&request.model),
        )?;
        db.maybe_title_conversation(&request.conversation_id, content)?;

        let assistant_message = db.append_message(
            &request.conversation_id,
            Some(&user_message.id),
            None,
            "assistant",
            "",
            "streaming",
            Some(&request.provider_id),
            Some(&request.model),
        )?;
        db.set_conversation_current_message(
            &request.conversation_id,
            &assistant_message.id,
            &request.provider_id,
            &request.model,
        )?;

        let mut provider_messages: Vec<ChatMessage> = active_messages
            .into_iter()
            .filter(|message| message.role == "user" || message.role == "assistant" || message.role == "system")
            .map(|message| ChatMessage {
                role: message.role,
                content: message.content,
            })
            .collect();
        provider_messages.push(ChatMessage {
            role: "user".to_string(),
            content: content.to_string(),
        });

        let provider_request = ProviderChatRequest {
            model: request.model.clone(),
            messages: provider_messages,
            temperature: request.temperature.or(provider.default_temperature),
            max_tokens: request.max_tokens.or(provider.default_max_tokens),
        };

        let result = SendChatResult {
            conversation_id: request.conversation_id.clone(),
            user_message_id: user_message.id,
            assistant_message_id: assistant_message.id,
        };

        emit_stream_start(&app, &result);

        (provider, provider_request, result)
    };

    spawn_provider_stream(
        app,
        &state,
        provider,
        provider_request,
        result.conversation_id.clone(),
        result.assistant_message_id.clone(),
    )?;

    Ok(result)
}

#[tauri::command]
pub fn edit_user_message(
    app: AppHandle,
    state: State<'_, AppState>,
    request: EditUserMessageRequest,
) -> Result<SendChatResult, AppError> {
    let content = request.content.trim();
    if content.is_empty() {
        return Err(AppError::invalid_input("Message cannot be empty."));
    }

    let (provider, provider_request, result) = {
        let db = lock_db(&state)?;
        let original_message = db.get_message(&request.message_id)?;
        if original_message.conversation_id != request.conversation_id || original_message.role != "user" {
            return Err(AppError::invalid_input("Only user messages in this conversation can be edited."));
        }

        let provider = db.get_provider(&request.provider_id)?;
        let parent_id = original_message.parent_message_id.as_deref();
        let history = if let Some(parent_message_id) = parent_id {
            db.get_message_path(parent_message_id)?
        } else {
            Vec::new()
        };

        let user_message = db.append_message(
            &request.conversation_id,
            parent_id,
            Some(&original_message.id),
            "user",
            content,
            "complete",
            Some(&request.provider_id),
            Some(&request.model),
        )?;

        let assistant_message = db.append_message(
            &request.conversation_id,
            Some(&user_message.id),
            None,
            "assistant",
            "",
            "streaming",
            Some(&request.provider_id),
            Some(&request.model),
        )?;

        db.set_conversation_current_message(
            &request.conversation_id,
            &assistant_message.id,
            &request.provider_id,
            &request.model,
        )?;

        let mut provider_messages: Vec<ChatMessage> = history
            .into_iter()
            .filter(|message| message.role == "user" || message.role == "assistant" || message.role == "system")
            .map(|message| ChatMessage {
                role: message.role,
                content: message.content,
            })
            .collect();
        provider_messages.push(ChatMessage {
            role: "user".to_string(),
            content: content.to_string(),
        });

        let provider_request = ProviderChatRequest {
            model: request.model.clone(),
            messages: provider_messages,
            temperature: request.temperature.or(provider.default_temperature),
            max_tokens: request.max_tokens.or(provider.default_max_tokens),
        };

        let result = SendChatResult {
            conversation_id: request.conversation_id.clone(),
            user_message_id: user_message.id,
            assistant_message_id: assistant_message.id,
        };

        emit_stream_start(&app, &result);
        (provider, provider_request, result)
    };

    spawn_provider_stream(
        app,
        &state,
        provider,
        provider_request,
        result.conversation_id.clone(),
        result.assistant_message_id.clone(),
    )?;

    Ok(result)
}

#[tauri::command]
pub fn regenerate_assistant_message(
    app: AppHandle,
    state: State<'_, AppState>,
    request: RegenerateAssistantMessageRequest,
) -> Result<SendChatResult, AppError> {
    let (provider, provider_request, result) = {
        let db = lock_db(&state)?;
        let original_message = db.get_message(&request.message_id)?;
        if original_message.conversation_id != request.conversation_id || original_message.role != "assistant" {
            return Err(AppError::invalid_input(
                "Only assistant messages in this conversation can be regenerated.",
            ));
        }

        let parent_message_id = original_message
            .parent_message_id
            .as_deref()
            .ok_or_else(|| AppError::invalid_input("Assistant message has no parent user message."))?;
        let parent_message = db.get_message(parent_message_id)?;
        if parent_message.role != "user" {
            return Err(AppError::invalid_input("Assistant regeneration requires a parent user message."));
        }

        let provider = db.get_provider(&request.provider_id)?;
        let history = db.get_message_path(parent_message_id)?;

        let assistant_message = db.append_message(
            &request.conversation_id,
            Some(parent_message_id),
            Some(&original_message.id),
            "assistant",
            "",
            "streaming",
            Some(&request.provider_id),
            Some(&request.model),
        )?;

        db.set_conversation_current_message(
            &request.conversation_id,
            &assistant_message.id,
            &request.provider_id,
            &request.model,
        )?;

        let provider_messages: Vec<ChatMessage> = history
            .into_iter()
            .filter(|message| message.role == "user" || message.role == "assistant" || message.role == "system")
            .map(|message| ChatMessage {
                role: message.role,
                content: message.content,
            })
            .collect();

        let provider_request = ProviderChatRequest {
            model: request.model.clone(),
            messages: provider_messages,
            temperature: request.temperature.or(provider.default_temperature),
            max_tokens: request.max_tokens.or(provider.default_max_tokens),
        };

        let result = SendChatResult {
            conversation_id: request.conversation_id.clone(),
            user_message_id: parent_message.id,
            assistant_message_id: assistant_message.id,
        };

        emit_stream_start(&app, &result);
        (provider, provider_request, result)
    };

    spawn_provider_stream(
        app,
        &state,
        provider,
        provider_request,
        result.conversation_id.clone(),
        result.assistant_message_id.clone(),
    )?;

    Ok(result)
}

#[tauri::command]
pub fn cancel_stream(state: State<'_, AppState>, message_id: String) -> Result<(), AppError> {
    let active_streams = state
        .active_streams
        .lock()
        .map_err(|_| AppError::new("state_error", "Could not access active streams."))?;

    if let Some(flag) = active_streams.get(&message_id) {
        flag.store(true, Ordering::Relaxed);
    }

    Ok(())
}

#[tauri::command]
pub async fn run_diagnostics(
    state: State<'_, AppState>,
    provider_id: String,
    model: Option<String>,
) -> Result<DiagnosticsResult, AppError> {
    let mut system = System::new_all();
    system.refresh_all();
    let disks = Disks::new_with_refreshed_list();
    let total_disk_bytes = disks.iter().map(|disk| disk.total_space()).sum();
    let available_disk_bytes = disks.iter().map(|disk| disk.available_space()).sum();

    let provider = {
        let db = lock_db(&state)?;
        db.get_provider(&provider_id)?
    };
    let runtime = ProviderRuntime::from_config(provider.clone())?;
    let provider_health = runtime.health().await;

    let selected_model = model.or(provider.default_model_id.clone());
    let local_models = {
        let db = lock_db(&state)?;
        db.list_models(&provider_id)?
    };
    let model_available = selected_model
        .as_deref()
        .map(|name| local_models.iter().any(|model| model.name == name && model.is_available))
        .unwrap_or(false);

    let benchmark = if provider_health.is_reachable {
        if let Some(model_name) = selected_model.clone() {
            run_benchmark(&runtime, model_name).await.ok()
        } else {
            None
        }
    } else {
        None
    };

    let guidance = performance_guidance(&provider.name, provider_health.is_reachable, model_available, benchmark.as_ref());

    Ok(DiagnosticsResult {
        os: format!(
            "{} {}",
            System::name().unwrap_or_else(|| "Unknown OS".to_string()),
            System::long_os_version().unwrap_or_default()
        )
        .trim()
        .to_string(),
        cpu: system
            .cpus()
            .first()
            .map(|cpu| cpu.brand().to_string())
            .unwrap_or_else(|| "Unknown CPU".to_string()),
        cpu_cores: system.cpus().len(),
        total_memory_bytes: system.total_memory(),
        available_memory_bytes: system.available_memory(),
        total_disk_bytes,
        available_disk_bytes,
        gpu: "GPU/accelerator detection is not available in the MVP diagnostics.".to_string(),
        provider_health,
        model_available,
        benchmark,
        guidance,
    })
}

#[tauri::command]
pub fn export_conversation_markdown(
    state: State<'_, AppState>,
    conversation_id: String,
) -> Result<String, AppError> {
    let db = lock_db(&state)?;
    let conversation = db.get_conversation(&conversation_id)?;
    let active_messages = db.get_active_messages(&conversation_id)?;
    let all_messages = db.get_all_conversation_messages(&conversation_id)?;
    let provider = conversation
        .provider_id
        .as_deref()
        .and_then(|provider_id| db.get_provider(provider_id).ok());

    Ok(conversation_to_markdown(
        &conversation,
        &active_messages,
        provider.as_ref(),
        all_messages.len() > active_messages.len(),
    ))
}

#[tauri::command]
pub fn export_conversation_json(
    state: State<'_, AppState>,
    conversation_id: String,
) -> Result<String, AppError> {
    let db = lock_db(&state)?;
    let conversation = db.get_conversation(&conversation_id)?;
    let provider = conversation
        .provider_id
        .as_deref()
        .and_then(|provider_id| db.get_provider(provider_id).ok());
    let export = ConversationExport {
        schema_version: CONVERSATION_EXPORT_SCHEMA_VERSION,
        exported_at: now(),
        messages: db.get_all_conversation_messages(&conversation_id)?,
        conversation,
        provider,
    };

    serde_json::to_string_pretty(&export)
        .map_err(|error| AppError::new("export_error", format!("Could not serialize export: {error}")))
}

#[tauri::command]
pub fn import_conversation_json(
    state: State<'_, AppState>,
    json: String,
) -> Result<crate::chat::Conversation, AppError> {
    let export: ConversationExport = serde_json::from_str(&json)
        .map_err(|error| AppError::invalid_input(format!("Invalid conversation JSON: {error}")))?;

    if export.schema_version != CONVERSATION_EXPORT_SCHEMA_VERSION {
        return Err(AppError::invalid_input("Unsupported conversation export schema version."));
    }
    validate_conversation_export(&export)?;

    let db = lock_db(&state)?;
    let imported = db.create_conversation(Some(format!("{} (imported)", export.conversation.title)))?;
    let mut id_map: HashMap<String, String> = HashMap::new();
    let mut imported_current_message_id: Option<String> = None;

    for message in export.messages {
        let parent = message
            .parent_message_id
            .as_ref()
            .and_then(|id| id_map.get(id).map(String::as_str));
        let revision = message
            .revision_of_message_id
            .as_ref()
            .and_then(|id| id_map.get(id).map(String::as_str));

        let new_message = db.append_message(
            &imported.id,
            parent,
            revision,
            &message.role,
            &message.content,
            &message.status,
            message.provider_id.as_deref(),
            message.model_id.as_deref(),
        )?;
        let metadata_json = serde_json::json!({
            "importedOriginalMessageId": &message.id,
            "importedOriginalConversationId": &export.conversation.id,
        })
        .to_string();
        db.set_message_metadata_json(&new_message.id, &metadata_json)?;

        if export.conversation.current_message_id.as_deref() == Some(message.id.as_str()) {
            imported_current_message_id = Some(new_message.id.clone());
        }

        id_map.insert(message.id, new_message.id);
    }

    if let Some(current_message_id) = imported_current_message_id {
        db.set_conversation_current_message(
            &imported.id,
            &current_message_id,
            imported.provider_id.as_deref().unwrap_or(DEFAULT_PROVIDER_ID),
            imported.model_id.as_deref().unwrap_or(""),
        )?;
    }

    db.get_conversation(&imported.id)
}

fn mark_stream_cancelled(app: &AppHandle, conversation_id: &str, message_id: &str) {
    if let Ok(db) = lock_db(&app.state::<AppState>()) {
        db.finish_message(message_id, "cancelled", Some("Generation was cancelled."), None, None)
            .ok();
    }

    app.emit(
        "chat:stream-cancelled",
        StreamEvent {
            conversation_id: conversation_id.to_string(),
            message_id: message_id.to_string(),
            delta: None,
            content: None,
            status: "cancelled".to_string(),
            error: Some("Generation was cancelled.".to_string()),
        },
    )
    .ok();
}

fn emit_stream_start(app: &AppHandle, result: &SendChatResult) {
    app.emit(
        "chat:stream-start",
        StreamEvent {
            conversation_id: result.conversation_id.clone(),
            message_id: result.assistant_message_id.clone(),
            delta: None,
            content: Some(String::new()),
            status: "streaming".to_string(),
            error: None,
        },
    )
    .ok();
}

fn spawn_provider_stream(
    app: AppHandle,
    state: &State<'_, AppState>,
    provider: ProviderConfig,
    provider_request: ProviderChatRequest,
    conversation_id: String,
    assistant_message_id: String,
) -> Result<(), AppError> {
    let cancel_flag = Arc::new(AtomicBool::new(false));
    {
        let mut active_streams = state
            .active_streams
            .lock()
            .map_err(|_| AppError::new("state_error", "Could not access active streams."))?;
        active_streams.insert(assistant_message_id.clone(), cancel_flag.clone());
    }

    let app_for_task = app.clone();

    tauri::async_runtime::spawn(async move {
        let runtime = match ProviderRuntime::from_config(provider) {
            Ok(runtime) => runtime,
            Err(error) => {
                mark_stream_failed(&app_for_task, &conversation_id, &assistant_message_id, error, false);
                return;
            }
        };

        let stream_result = runtime
            .stream_chat(provider_request, |delta| {
                if cancel_flag.load(Ordering::Relaxed) {
                    return Err(AppError::new("cancelled", "Generation was cancelled."));
                }

                let state = app_for_task.state::<AppState>();
                let content = {
                    let db = lock_db(&state)?;
                    db.append_to_message_content(&assistant_message_id, delta)?
                };

                app_for_task
                    .emit(
                        "chat:stream-delta",
                        StreamEvent {
                            conversation_id: conversation_id.clone(),
                            message_id: assistant_message_id.clone(),
                            delta: Some(delta.to_string()),
                            content: Some(content),
                            status: "streaming".to_string(),
                            error: None,
                        },
                    )
                    .ok();

                Ok(())
            })
            .await;

        let was_cancelled = cancel_flag.load(Ordering::Relaxed);
        match stream_result {
            Ok(usage) if !was_cancelled => {
                if let Ok(db) = lock_db(&app_for_task.state::<AppState>()) {
                    db.finish_message(
                        &assistant_message_id,
                        "complete",
                        None,
                        usage.input_tokens,
                        usage.output_tokens,
                    )
                    .ok();
                }

                app_for_task
                    .emit(
                        "chat:stream-complete",
                        StreamEvent {
                            conversation_id: conversation_id.clone(),
                            message_id: assistant_message_id.clone(),
                            delta: None,
                            content: None,
                            status: "complete".to_string(),
                            error: None,
                        },
                    )
                    .ok();
            }
            Ok(_) => {
                mark_stream_cancelled(&app_for_task, &conversation_id, &assistant_message_id);
            }
            Err(error) if was_cancelled || error.code == "cancelled" => {
                mark_stream_cancelled(&app_for_task, &conversation_id, &assistant_message_id);
            }
            Err(error) => {
                mark_stream_failed(&app_for_task, &conversation_id, &assistant_message_id, error, true);
            }
        }

        {
            let state = app_for_task.state::<AppState>();
            if let Ok(mut active_streams) = state.active_streams.lock() {
                active_streams.remove(&assistant_message_id);
            };
        }
    });

    Ok(())
}

fn mark_stream_failed(
    app: &AppHandle,
    conversation_id: &str,
    message_id: &str,
    error: AppError,
    emit_error: bool,
) {
    if let Ok(db) = lock_db(&app.state::<AppState>()) {
        db.finish_message(message_id, "failed", Some(&error.message), None, None)
            .ok();
    }

    if emit_error {
        app.emit(
            "chat:stream-error",
            StreamEvent {
                conversation_id: conversation_id.to_string(),
                message_id: message_id.to_string(),
                delta: None,
                content: None,
                status: "failed".to_string(),
                error: Some(error.message),
            },
        )
        .ok();
    }
}

async fn run_benchmark(
    runtime: &ProviderRuntime,
    model: String,
) -> Result<BenchmarkResult, AppError> {
    let start = Instant::now();
    let mut first_token_ms = None;
    let mut output = String::new();

    runtime
        .stream_chat(
            ProviderChatRequest {
                model,
                messages: vec![ChatMessage {
                    role: "user".to_string(),
                    content: "Reply with one short sentence about local AI readiness.".to_string(),
                }],
                temperature: Some(0.2),
                max_tokens: Some(64),
            },
            |delta| {
                if first_token_ms.is_none() {
                    first_token_ms = Some(start.elapsed().as_millis());
                }
                output.push_str(delta);
                Ok(())
            },
        )
        .await?;

    let total_time_ms = start.elapsed().as_millis();
    let token_estimate = output.split_whitespace().count().max(1) as f64;
    let seconds = (total_time_ms as f64 / 1000.0).max(0.001);

    Ok(BenchmarkResult {
        time_to_first_token_ms: first_token_ms,
        total_time_ms,
        approximate_tokens_per_second: Some(token_estimate / seconds),
        output_preview: output.chars().take(160).collect(),
    })
}

fn performance_guidance(
    provider_name: &str,
    provider_reachable: bool,
    model_available: bool,
    benchmark: Option<&BenchmarkResult>,
) -> String {
    if !provider_reachable {
        return format!("{provider_name} is not reachable. Start it to run local models.");
    }

    if !model_available {
        return format!("The selected model is not available. Install a model via {provider_name}, then refresh.");
    }

    let Some(benchmark) = benchmark else {
        return "Ark could not complete the benchmark. Chat may still work, but performance is unknown.".to_string();
    };

    let tokens_per_second = benchmark.approximate_tokens_per_second.unwrap_or(0.0);
    if tokens_per_second >= 25.0 {
        "Good for small and medium local models.".to_string()
    } else if tokens_per_second >= 8.0 {
        "Usable for small local models. Larger models may feel slow.".to_string()
    } else {
        "Expect slower responses. Prefer smaller quantized models on this device.".to_string()
    }
}

fn lock_db<'a>(state: &'a State<'_, AppState>) -> Result<std::sync::MutexGuard<'a, Database>, AppError> {
    state
        .db
        .lock()
        .map_err(|_| AppError::new("state_error", "Could not access local database."))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn benchmark(tps: f64) -> BenchmarkResult {
        BenchmarkResult {
            time_to_first_token_ms: Some(100),
            total_time_ms: 1000,
            approximate_tokens_per_second: Some(tps),
            output_preview: "test".to_string(),
        }
    }

    #[test]
    fn guidance_when_provider_unreachable() {
        let result = performance_guidance("Ollama", false, false, None);
        assert!(result.contains("Ollama"), "should name the provider");
        assert!(result.contains("not reachable") || result.contains("Start"), "should direct user to start");
    }

    #[test]
    fn guidance_when_model_unavailable() {
        let result = performance_guidance("Local inference host", true, false, None);
        assert!(result.contains("Local inference host"), "should name the provider");
        assert!(result.to_lowercase().contains("model") || result.to_lowercase().contains("install"));
    }

    #[test]
    fn guidance_when_benchmark_missing() {
        let result = performance_guidance("Ollama", true, true, None);
        assert!(result.to_lowercase().contains("benchmark") || result.to_lowercase().contains("performance"));
    }

    #[test]
    fn guidance_fast_performance() {
        let result = performance_guidance("Ollama", true, true, Some(&benchmark(30.0)));
        assert!(result.to_lowercase().contains("good"));
    }

    #[test]
    fn guidance_medium_performance() {
        let result = performance_guidance("Ollama", true, true, Some(&benchmark(12.0)));
        assert!(result.to_lowercase().contains("usable") || result.to_lowercase().contains("small"));
    }

    #[test]
    fn guidance_slow_performance() {
        let result = performance_guidance("Ollama", true, true, Some(&benchmark(3.0)));
        assert!(result.to_lowercase().contains("slow") || result.to_lowercase().contains("smaller"));
    }

    #[test]
    fn guidance_boundary_25_tps_is_fast() {
        let result = performance_guidance("Ollama", true, true, Some(&benchmark(25.0)));
        assert!(result.to_lowercase().contains("good"));
    }

    #[test]
    fn guidance_boundary_8_tps_is_usable() {
        let result = performance_guidance("Ollama", true, true, Some(&benchmark(8.0)));
        assert!(result.to_lowercase().contains("usable") || result.to_lowercase().contains("small"));
    }

    #[test]
    fn guidance_boundary_below_8_tps_is_slow() {
        let result = performance_guidance("Ollama", true, true, Some(&benchmark(7.9)));
        assert!(result.to_lowercase().contains("slow") || result.to_lowercase().contains("smaller"));
    }
}

// ── Built-in runtime (bundled llama-server) ──────────────────────────────────

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuiltInRuntimeStatus {
    pub running: bool,
    pub port: Option<u16>,
    pub model_path: Option<String>,
}

#[tauri::command]
pub async fn get_built_in_runtime_status(state: State<'_, AppState>) -> Result<BuiltInRuntimeStatus, AppError> {
    let mut sidecar = state.sidecar.lock().unwrap();
    Ok(BuiltInRuntimeStatus {
        running: sidecar.is_running(),
        port: sidecar.port,
        model_path: sidecar.model_path.clone(),
    })
}

#[tauri::command]
pub async fn stop_built_in_runtime(state: State<'_, AppState>) -> Result<(), AppError> {
    let mut sidecar = state.sidecar.lock().unwrap();
    sidecar.stop();
    Ok(())
}

#[tauri::command]
pub async fn start_built_in_runtime(
    model_path: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<BuiltInRuntimeStatus, AppError> {
    use crate::config::{BUILT_IN_DEFAULT_PORT, BUILT_IN_PROVIDER_ID};
    use crate::sidecar::{find_free_port, llama_server_binary, spawn_llama_server, wait_for_ready};

    {
        let mut sidecar = state.sidecar.lock().unwrap();
        sidecar.stop();
    }

    let binary = llama_server_binary(&app);

    let port = find_free_port(BUILT_IN_DEFAULT_PORT)
        .ok_or_else(|| AppError::provider("No free port found for built-in runtime (tried 11435–11534)."))?;

    let child = spawn_llama_server(&binary, &model_path, port)?;

    {
        let mut sidecar = state.sidecar.lock().unwrap();
        sidecar.process = Some(child);
        sidecar.port = Some(port);
        sidecar.model_path = Some(model_path.clone());
    }

    if !wait_for_ready(port).await {
        let mut sidecar = state.sidecar.lock().unwrap();
        sidecar.stop();
        return Err(AppError::provider(
            "Built-in runtime didn't become ready within 30 seconds. Verify the model file path and format (GGUF required).",
        ));
    }

    {
        let db = state.db.lock().unwrap();
        let base_url = format!("http://127.0.0.1:{port}");
        let _ = db.update_provider_base_url(BUILT_IN_PROVIDER_ID, &base_url);
    }

    Ok(BuiltInRuntimeStatus { running: true, port: Some(port), model_path: Some(model_path) })
}
