//! ARC-001: the conversation/generation application workflow, extracted from `commands::mod` —
//! sending a new message, editing a user message, regenerating an assistant message, cancelling
//! a stream, and the streaming supervision underneath all of them. Tauri commands in
//! `commands::mod` remain thin adapters: decode the request, delegate here, return the result.
//! This is a pure code-motion extraction: no behavior changed, and the full existing test suite
//! (streaming, cancellation, interruption) continues to pass unchanged, which is itself the
//! acceptance evidence that no regression was introduced.

use crate::chat::{
    ChatMessage, Conversation, Message, SendChatRequest, SendChatResult, StreamEvent,
};
use crate::db::Database;
use crate::errors::AppError;
use crate::projects::Project;
use crate::providers::{ProviderChatRequest, ProviderConfig, ProviderRegistry};
use crate::AppState;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tauri::{AppHandle, Emitter, Manager};

/// FTR-004/FTR-003: where an effective generation setting actually came from — recorded in the
/// assistant message's provenance so "what settings produced this response" is answerable
/// later without re-deriving it from the conversation/project/provider rows as they exist *now*
/// (which may have since changed).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum SettingSource {
    /// An explicit per-request override — today only ever sent by a caller that supplies a
    /// value distinct from every tier below (the frontend does not currently expose one, but
    /// the field/tier has existed since COR-003 and remains the highest-precedence override).
    Request,
    Conversation,
    /// FTR-003: the conversation's assigned project's own default — sits between the
    /// conversation's own override and the provider's default, so a project can steer every
    /// conversation inside it without conversations having to repeat the same override.
    Project,
    ProviderDefault,
}

/// FTR-004/FTR-003: the four-tier precedence — request override, then this conversation's own
/// setting, then its assigned project's default (if any), then the provider's current default —
/// resolved once per generation call, not three times with divergent logic at each of
/// `send_chat_message`/`edit_user_message`/`regenerate_assistant_message`.
fn resolve_setting<T>(
    request_value: Option<T>,
    conversation_value: Option<T>,
    project_value: Option<T>,
    provider_value: Option<T>,
) -> (Option<T>, Option<SettingSource>) {
    if let Some(value) = request_value {
        return (Some(value), Some(SettingSource::Request));
    }
    if let Some(value) = conversation_value {
        return (Some(value), Some(SettingSource::Conversation));
    }
    if let Some(value) = project_value {
        return (Some(value), Some(SettingSource::Project));
    }
    if let Some(value) = provider_value {
        return (Some(value), Some(SettingSource::ProviderDefault));
    }
    (None, None)
}

/// FTR-003: the system prompt has no per-request override tier (unlike temperature/max_tokens),
/// so this is a separate two-tier resolution rather than a degenerate call to `resolve_setting`
/// with an always-`None` request tier — a conversation's own override, then its project's
/// instructions.
fn resolve_system_prompt(
    conversation_value: Option<&str>,
    project_value: Option<&str>,
) -> (Option<String>, Option<SettingSource>) {
    if let Some(value) = conversation_value {
        return (Some(value.to_string()), Some(SettingSource::Conversation));
    }
    if let Some(value) = project_value {
        return (Some(value.to_string()), Some(SettingSource::Project));
    }
    (None, None)
}

/// FTR-003: fetches the conversation's assigned project, if any. A conversation whose
/// `project_id` no longer resolves (only reachable if something bypassed
/// `Database::set_conversation_project`'s own existence check) is treated as unassigned rather
/// than failing the generation — a stale reference on the send path shouldn't block a message.
fn resolve_conversation_project(db: &Database, conversation: &Conversation) -> Option<Project> {
    let project_id = conversation.project_id.as_deref()?;
    db.get_project(project_id).ok()
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct GenerationProvenance {
    provider_id: String,
    model: String,
    /// FTR-003: the project this conversation was assigned to at generation time, if any — kept
    /// alongside `system_prompt_source`/`*_source` below so a later reader can tell *which*
    /// project contributed a `Project`-sourced setting without re-deriving it from the
    /// conversation row as it exists *now*, which may have since been reassigned.
    project_id: Option<String>,
    temperature: Option<f64>,
    temperature_source: Option<SettingSource>,
    max_tokens: Option<i64>,
    max_tokens_source: Option<SettingSource>,
    /// Which tier (if any) supplied the system prompt actually injected — not the prompt text
    /// itself, which stays out of provenance metadata the same way message content itself is
    /// never duplicated into it.
    system_prompt_source: Option<SettingSource>,
}

/// FTR-004 acceptance: "effective settings ... stored in response provenance." Best-effort — a
/// serialization/write failure here must never fail the generation itself (the message and its
/// content remain fully usable without provenance), so this silently drops the error rather
/// than propagating an `AppError` that would abort an otherwise-successful send/edit/regenerate.
fn record_generation_provenance(
    db: &Database,
    assistant_message_id: &str,
    provenance: &GenerationProvenance,
) {
    let Ok(json) = serde_json::to_string(provenance) else {
        return;
    };
    let _ = db.set_message_metadata_json(assistant_message_id, &json);
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

/// COR-002: provider work is queued in memory after the durable message transaction commits,
/// then consumed by the explicit `start_pending_stream` command. That second IPC call happens
/// only after the frontend has installed the returned durable IDs in its normalized store, so
/// an immediate provider cannot beat its placeholder. A queued plan is single-use and contains
/// no user-facing authority: the database remains the durable lifecycle source of truth.
pub(crate) struct PendingStream {
    provider: ProviderConfig,
    bearer_token: Option<String>,
    provider_request: ProviderChatRequest,
    conversation_id: String,
    assistant_message_id: String,
}

/// A durable cancellation request is committed separately; this control handles the best-effort
/// transport side. `Notify::notify_one` retains a permit when cancellation arrives before the
/// task begins selecting, closing the former registration/start window without polling.
pub(crate) struct StreamCancellation {
    requested: AtomicBool,
    notified: tokio::sync::Notify,
}

impl StreamCancellation {
    fn new() -> Self {
        Self {
            requested: AtomicBool::new(false),
            notified: tokio::sync::Notify::new(),
        }
    }

    fn request(&self) {
        self.requested.store(true, Ordering::Release);
        self.notified.notify_one();
    }

    fn is_requested(&self) -> bool {
        self.requested.load(Ordering::Acquire)
    }
}

pub fn send_chat_message(
    state: &AppState,
    mut request: SendChatRequest,
) -> Result<SendChatResult, AppError> {
    request.conversation_id =
        crate::validation::validate_entity_id(&request.conversation_id, "Conversation ID")?
            .to_string();
    request.provider_id =
        crate::validation::validate_entity_id(&request.provider_id, "Provider ID")?.to_string();
    let content = request.content.trim();
    if content.is_empty() {
        return Err(AppError::invalid_input("Message cannot be empty."));
    }
    let temperature = crate::validation::validate_temperature(request.temperature)?;
    let max_tokens = crate::validation::validate_max_tokens(request.max_tokens)?;

    let (provider, provider_request, result) = {
        let db = crate::commands::lock_db(state)?;
        // COR-004: user message insert, title generation, assistant placeholder insert, and
        // the conversation's current-message pointer update must commit together or not at
        // all — a crash between any two of these would otherwise orphan a user message that
        // never appears in the active branch. No provider I/O happens inside this closure.
        db.transaction(|| {
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

            let project = resolve_conversation_project(&db, &conversation);

            let (effective_system_prompt, system_prompt_source) = resolve_system_prompt(
                conversation.system_prompt.as_deref(),
                project.as_ref().and_then(|p| p.instructions.as_deref()),
            );
            let mut provider_messages: Vec<ChatMessage> = Vec::new();
            if let Some(system_prompt) = &effective_system_prompt {
                provider_messages.push(ChatMessage {
                    role: "system".to_string(),
                    content: system_prompt.clone(),
                });
            }
            provider_messages.extend(active_messages.into_iter().filter_map(|message| {
                matches!(message.role.as_str(), "user" | "assistant" | "system").then_some(
                    ChatMessage {
                        role: message.role,
                        content: message.content,
                    },
                )
            }));
            provider_messages.push(ChatMessage {
                role: "user".to_string(),
                content: content.to_string(),
            });

            let (effective_temperature, temperature_source) = resolve_setting(
                temperature,
                conversation.temperature,
                project.as_ref().and_then(|p| p.default_temperature),
                provider.default_temperature,
            );
            let (effective_max_tokens, max_tokens_source) = resolve_setting(
                max_tokens,
                conversation.max_tokens,
                project.as_ref().and_then(|p| p.default_max_tokens),
                provider.default_max_tokens,
            );

            record_generation_provenance(
                &db,
                &assistant_message.id,
                &GenerationProvenance {
                    provider_id: request.provider_id.clone(),
                    model: request.model.clone(),
                    project_id: conversation.project_id.clone(),
                    temperature: effective_temperature,
                    temperature_source,
                    max_tokens: effective_max_tokens,
                    max_tokens_source,
                    system_prompt_source,
                },
            );

            let provider_request = ProviderChatRequest {
                model: request.model.clone(),
                messages: provider_messages,
                temperature: effective_temperature,
                max_tokens: effective_max_tokens,
                user_deadline: None,
            };

            let result = SendChatResult {
                conversation_id: request.conversation_id.clone(),
                user_message_id: user_message.id,
                assistant_message_id: assistant_message.id,
            };

            Ok((provider, provider_request, result))
        })?
    };

    queue_provider_stream(
        state,
        provider,
        provider_request,
        result.conversation_id.clone(),
        result.assistant_message_id.clone(),
    )?;

    Ok(result)
}

pub fn edit_user_message(
    state: &AppState,
    mut request: EditUserMessageRequest,
) -> Result<SendChatResult, AppError> {
    request.conversation_id =
        crate::validation::validate_entity_id(&request.conversation_id, "Conversation ID")?
            .to_string();
    request.message_id =
        crate::validation::validate_entity_id(&request.message_id, "Message ID")?.to_string();
    request.provider_id =
        crate::validation::validate_entity_id(&request.provider_id, "Provider ID")?.to_string();
    let content = request.content.trim();
    if content.is_empty() {
        return Err(AppError::invalid_input("Message cannot be empty."));
    }
    let temperature = crate::validation::validate_temperature(request.temperature)?;
    let max_tokens = crate::validation::validate_max_tokens(request.max_tokens)?;

    let (provider, provider_request, result) = {
        let db = crate::commands::lock_db(state)?;
        let original_message = db.get_message(&request.message_id)?;
        if original_message.conversation_id != request.conversation_id
            || original_message.role != "user"
        {
            return Err(AppError::invalid_input(
                "Only user messages in this conversation can be edited.",
            ));
        }

        // COR-004: the edit's new user-message revision, its assistant placeholder, and the
        // conversation pointer update must commit atomically — see send_chat_message.
        db.transaction(|| {
            let conversation = db.get_conversation(&request.conversation_id)?;
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

            let project = resolve_conversation_project(&db, &conversation);

            let (effective_system_prompt, system_prompt_source) = resolve_system_prompt(
                conversation.system_prompt.as_deref(),
                project.as_ref().and_then(|p| p.instructions.as_deref()),
            );
            let mut provider_messages: Vec<ChatMessage> = Vec::new();
            if let Some(system_prompt) = &effective_system_prompt {
                provider_messages.push(ChatMessage {
                    role: "system".to_string(),
                    content: system_prompt.clone(),
                });
            }
            provider_messages.extend(history.into_iter().filter_map(|message| {
                matches!(message.role.as_str(), "user" | "assistant" | "system").then_some(
                    ChatMessage {
                        role: message.role,
                        content: message.content,
                    },
                )
            }));
            provider_messages.push(ChatMessage {
                role: "user".to_string(),
                content: content.to_string(),
            });

            let (effective_temperature, temperature_source) = resolve_setting(
                temperature,
                conversation.temperature,
                project.as_ref().and_then(|p| p.default_temperature),
                provider.default_temperature,
            );
            let (effective_max_tokens, max_tokens_source) = resolve_setting(
                max_tokens,
                conversation.max_tokens,
                project.as_ref().and_then(|p| p.default_max_tokens),
                provider.default_max_tokens,
            );

            record_generation_provenance(
                &db,
                &assistant_message.id,
                &GenerationProvenance {
                    provider_id: request.provider_id.clone(),
                    model: request.model.clone(),
                    project_id: conversation.project_id.clone(),
                    temperature: effective_temperature,
                    temperature_source,
                    max_tokens: effective_max_tokens,
                    max_tokens_source,
                    system_prompt_source,
                },
            );

            let provider_request = ProviderChatRequest {
                model: request.model.clone(),
                messages: provider_messages,
                temperature: effective_temperature,
                max_tokens: effective_max_tokens,
                user_deadline: None,
            };

            let result = SendChatResult {
                conversation_id: request.conversation_id.clone(),
                user_message_id: user_message.id,
                assistant_message_id: assistant_message.id,
            };

            Ok((provider, provider_request, result))
        })?
    };

    queue_provider_stream(
        state,
        provider,
        provider_request,
        result.conversation_id.clone(),
        result.assistant_message_id.clone(),
    )?;

    Ok(result)
}

pub fn regenerate_assistant_message(
    state: &AppState,
    mut request: RegenerateAssistantMessageRequest,
) -> Result<SendChatResult, AppError> {
    request.conversation_id =
        crate::validation::validate_entity_id(&request.conversation_id, "Conversation ID")?
            .to_string();
    request.message_id =
        crate::validation::validate_entity_id(&request.message_id, "Message ID")?.to_string();
    request.provider_id =
        crate::validation::validate_entity_id(&request.provider_id, "Provider ID")?.to_string();
    let temperature = crate::validation::validate_temperature(request.temperature)?;
    let max_tokens = crate::validation::validate_max_tokens(request.max_tokens)?;

    let (provider, provider_request, result) = {
        let db = crate::commands::lock_db(state)?;
        let original_message = db.get_message(&request.message_id)?;
        if original_message.conversation_id != request.conversation_id
            || original_message.role != "assistant"
        {
            return Err(AppError::invalid_input(
                "Only assistant messages in this conversation can be regenerated.",
            ));
        }

        let parent_message_id = original_message
            .parent_message_id
            .as_deref()
            .ok_or_else(|| {
                AppError::invalid_input("Assistant message has no parent user message.")
            })?;
        let parent_message = db.get_message(parent_message_id)?;
        if parent_message.role != "user" {
            return Err(AppError::invalid_input(
                "Assistant regeneration requires a parent user message.",
            ));
        }

        let conversation = db.get_conversation(&request.conversation_id)?;
        let provider = db.get_provider(&request.provider_id)?;
        let project = resolve_conversation_project(&db, &conversation);
        let history = db.get_message_path(parent_message_id)?;

        // COR-004: the new assistant revision insert and the conversation pointer update
        // must commit atomically — see send_chat_message.
        let (provider_request, result) = db.transaction(|| {
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

            let (effective_system_prompt, system_prompt_source) = resolve_system_prompt(
                conversation.system_prompt.as_deref(),
                project.as_ref().and_then(|p| p.instructions.as_deref()),
            );
            let mut provider_messages: Vec<ChatMessage> = Vec::new();
            if let Some(system_prompt) = &effective_system_prompt {
                provider_messages.push(ChatMessage {
                    role: "system".to_string(),
                    content: system_prompt.clone(),
                });
            }
            provider_messages.extend(history.into_iter().filter_map(|message| {
                matches!(message.role.as_str(), "user" | "assistant" | "system").then_some(
                    ChatMessage {
                        role: message.role,
                        content: message.content,
                    },
                )
            }));

            let (effective_temperature, temperature_source) = resolve_setting(
                temperature,
                conversation.temperature,
                project.as_ref().and_then(|p| p.default_temperature),
                provider.default_temperature,
            );
            let (effective_max_tokens, max_tokens_source) = resolve_setting(
                max_tokens,
                conversation.max_tokens,
                project.as_ref().and_then(|p| p.default_max_tokens),
                provider.default_max_tokens,
            );

            record_generation_provenance(
                &db,
                &assistant_message.id,
                &GenerationProvenance {
                    provider_id: request.provider_id.clone(),
                    model: request.model.clone(),
                    project_id: conversation.project_id.clone(),
                    temperature: effective_temperature,
                    temperature_source,
                    max_tokens: effective_max_tokens,
                    max_tokens_source,
                    system_prompt_source,
                },
            );

            let provider_request = ProviderChatRequest {
                model: request.model.clone(),
                messages: provider_messages,
                temperature: effective_temperature,
                max_tokens: effective_max_tokens,
                user_deadline: None,
            };

            let result = SendChatResult {
                conversation_id: request.conversation_id.clone(),
                user_message_id: parent_message.id,
                assistant_message_id: assistant_message.id,
            };

            Ok((provider_request, result))
        })?;

        (provider, provider_request, result)
    };

    queue_provider_stream(
        state,
        provider,
        provider_request,
        result.conversation_id.clone(),
        result.assistant_message_id.clone(),
    )?;

    Ok(result)
}

/// COR-005: cancellation is durable, not just an in-memory signal. This function:
/// 1. Best-effort signals the live task (if one is registered) to stop reading from the
///    provider as soon as possible — this is the "attempt sidecar/HTTP cancellation" part.
/// 2. Synchronously commits the durable terminal state via a conditional update, so the
///    caller itself — not the eventual reaction of a background task that might be slow,
///    hung, or already gone — is what the UI can trust. This is what makes cancellation
///    restart-safe and correct even for a "missing task" (e.g. the process already exited,
///    or this call arrives after a restart where COR-001 recovery already ran).
/// 3. Idempotent: cancelling an already-terminal message (complete/failed/cancelled/
///    interrupted) matches zero rows and is a harmless, error-free no-op.
///
pub fn cancel_stream(app: AppHandle, state: &AppState, message_id: String) -> Result<(), AppError> {
    let message_id = crate::validation::validate_entity_id(&message_id, "Message ID")?.to_string();
    let (message, became_cancelled) = request_cancellation(state, &message_id)?;

    if became_cancelled {
        app.emit(
            "chat:stream-cancelled",
            StreamEvent {
                conversation_id: message.conversation_id,
                message_id,
                delta: None,
                content: None,
                status: "cancelled".to_string(),
                error: Some("Generation was cancelled by the user.".to_string()),
                revision: None,
                schema_version: crate::chat::STREAM_EVENT_SCHEMA_VERSION,
            },
        )
        .ok();
    }

    Ok(())
}

fn request_cancellation(state: &AppState, message_id: &str) -> Result<(Message, bool), AppError> {
    let message = crate::commands::lock_db(state)?.get_message(message_id)?;

    // A cancellation can arrive after the placeholder was committed but before the frontend's
    // explicit start IPC. Removing the single-use plan prevents provider I/O from ever starting.
    state
        .pending_streams
        .lock()
        .map_err(|_| AppError::new("state_error", "Could not access pending streams."))?
        .remove(message_id);

    if let Ok(active_streams) = state.active_streams.lock() {
        if let Some(flag) = active_streams.get(message_id) {
            flag.request();
        }
    }

    let became_cancelled = crate::commands::lock_db(state)?.finish_message_if_active(
        message_id,
        "cancelled",
        Some("Generation was cancelled by the user."),
        None,
        None,
    )?;

    Ok((message, became_cancelled))
}

/// COR-011: checkpoint cadence ceilings. 250ms caps checkpoint frequency at 4/sec — comfortably
/// under the "≤20 batches/sec" acceptance ceiling — while staying responsive; the byte
/// threshold guards against unbounded buffer growth if a provider emits an unusually large
/// single delta.
const STREAM_CHECKPOINT_INTERVAL: std::time::Duration = std::time::Duration::from_millis(250);
const STREAM_CHECKPOINT_MAX_BYTES: usize = 8192;

pub(crate) fn emit_stream_start(app: &AppHandle, conversation_id: &str, message_id: &str) {
    app.emit(
        "chat:stream-start",
        StreamEvent {
            conversation_id: conversation_id.to_string(),
            message_id: message_id.to_string(),
            delta: None,
            content: Some(String::new()),
            status: "streaming".to_string(),
            error: None,
            revision: None,
            schema_version: crate::chat::STREAM_EVENT_SCHEMA_VERSION,
        },
    )
    .ok();
}

fn queue_provider_stream(
    state: &AppState,
    provider: ProviderConfig,
    provider_request: ProviderChatRequest,
    conversation_id: String,
    assistant_message_id: String,
) -> Result<(), AppError> {
    let bearer_token = crate::secret_store::resolve_bearer_token(state, &provider);
    if let Err(error) =
        ProviderRegistry::create_with_bearer_token(provider.clone(), bearer_token.clone())
    {
        crate::commands::lock_db(state)?.finish_message_if_active(
            &assistant_message_id,
            "failed",
            Some(&error.message),
            None,
            None,
        )?;
        return Err(error);
    }
    let plan = PendingStream {
        provider,
        bearer_token,
        provider_request,
        conversation_id,
        assistant_message_id: assistant_message_id.clone(),
    };
    let mut pending = match state.pending_streams.lock() {
        Ok(pending) => pending,
        Err(_) => {
            let _ = crate::commands::lock_db(state)?.finish_message_if_active(
                &assistant_message_id,
                "failed",
                Some("Ark could not queue provider work."),
                None,
                None,
            );
            return Err(AppError::new(
                "state_error",
                "Could not access pending streams.",
            ));
        }
    };
    if pending.contains_key(&assistant_message_id) {
        return Err(AppError::new(
            "state_error",
            "A generation plan already exists for this message.",
        ));
    }
    pending.insert(assistant_message_id, plan);
    Ok(())
}

/// COR-002 handshake: the create/edit/regenerate command has already returned durable IDs and
/// the frontend has synchronously installed its placeholder before invoking this command. The
/// plan is removed exactly once. `spawn_provider_stream` registers cancellation and emits start
/// before scheduling provider work, preserving start → delta* → terminal event order.
pub fn start_pending_stream(
    app: AppHandle,
    state: &AppState,
    message_id: String,
) -> Result<(), AppError> {
    let message_id = crate::validation::validate_entity_id(&message_id, "Message ID")?.to_string();
    let plan = state
        .pending_streams
        .lock()
        .map_err(|_| AppError::new("state_error", "Could not access pending streams."))?
        .remove(&message_id);

    let Some(plan) = plan else {
        let is_active = state
            .active_streams
            .lock()
            .map_err(|_| AppError::new("state_error", "Could not access active streams."))?
            .contains_key(&message_id);
        if is_active {
            return Ok(());
        }
        let message = crate::commands::lock_db(state)?.get_message(&message_id)?;
        if !matches!(message.status.as_str(), "pending" | "streaming") {
            return Ok(());
        }
        return Err(AppError::new(
            "generation_not_pending",
            "This generation has no queued provider work. Retry the interrupted response.",
        ));
    };

    debug_assert_eq!(plan.assistant_message_id, message_id);
    let result = spawn_provider_stream(
        app,
        state,
        plan.provider,
        plan.bearer_token,
        plan.provider_request,
        plan.conversation_id,
        plan.assistant_message_id,
    );
    if let Err(error) = &result {
        crate::commands::lock_db(state)?.finish_message_if_active(
            &message_id,
            "failed",
            Some(&error.message),
            None,
            None,
        )?;
        // The command error is the foreground notification. No provider task exists, so there
        // is no second terminal event to emit; an authoritative reload sees `failed`.
    }
    result
}

pub(crate) fn spawn_provider_stream(
    app: AppHandle,
    state: &AppState,
    provider: ProviderConfig,
    bearer_token: Option<String>,
    provider_request: ProviderChatRequest,
    conversation_id: String,
    assistant_message_id: String,
) -> Result<(), AppError> {
    let cancellation = Arc::new(StreamCancellation::new());
    {
        let mut active_streams = state
            .active_streams
            .lock()
            .map_err(|_| AppError::new("state_error", "Could not access active streams."))?;
        active_streams.insert(assistant_message_id.clone(), cancellation.clone());
    }

    // Cancellation is registered before start is visible, and provider work is not scheduled
    // until after start is emitted. This makes the externally observable order deterministic.
    emit_stream_start(&app, &conversation_id, &assistant_message_id);

    let app_for_task = app.clone();

    tauri::async_runtime::spawn(async move {
        let runtime = match ProviderRegistry::create_with_bearer_token(provider, bearer_token) {
            Ok(runtime) => runtime,
            Err(error) => {
                mark_stream_failed(
                    &app_for_task,
                    &conversation_id,
                    &assistant_message_id,
                    error,
                    false,
                );
                return;
            }
        };

        // ARC-003: every current adapter supports streaming, but this is the actual
        // capability-driven guard, not a check duplicated from `ProviderConfig.capabilities` —
        // a future provider type that genuinely can't stream fails here with a clear, typed
        // error instead of reaching a `stream_chat` implementation that was never meant to be
        // called for it.
        if !runtime.capabilities().streaming {
            mark_stream_failed(
                &app_for_task,
                &conversation_id,
                &assistant_message_id,
                AppError::invalid_input("This provider does not support streaming chat."),
                true,
            );
            return;
        }

        // COR-011: buffer deltas in memory and checkpoint to SQLite at a bounded rate rather
        // than rewriting the full `content` column on every single delta (which is O(current
        // length) per write in SQLite and, over a long response, approaches O(n²) total work).
        // The emitted `chat:stream-delta` event also carries only the delta, not the full
        // accumulated content, so per-delta IPC payload size no longer grows with response
        // length — the frontend accumulates deltas itself and only trusts a full-content
        // payload once, on the terminal event, as a correctness backstop against any missed
        // delta (a stronger reconciliation belongs to COR-002's broader event-ordering work).
        let mut buffer = String::new();
        let mut last_checkpoint = Instant::now();
        // COR-002 (partial): monotonic per-message delta sequence, starting at 1. Lets the
        // frontend detect a missed/out-of-order delta and resync from durable state instead
        // of silently corrupting its client-accumulated content.
        let mut revision: i64 = 0;

        let stream_result = {
            let mut on_delta = |delta: &str| {
                if cancellation.is_requested() {
                    return Err(AppError::new("cancelled", "Generation was cancelled."));
                }

                buffer.push_str(delta);

                if buffer.len() >= STREAM_CHECKPOINT_MAX_BYTES
                    || last_checkpoint.elapsed() >= STREAM_CHECKPOINT_INTERVAL
                {
                    let state = app_for_task.state::<AppState>();
                    let db = crate::commands::lock_db(&state)?;
                    db.append_to_message_content(&assistant_message_id, &buffer)?;
                    buffer.clear();
                    last_checkpoint = Instant::now();
                }

                revision += 1;
                app_for_task
                    .emit(
                        "chat:stream-delta",
                        StreamEvent {
                            conversation_id: conversation_id.clone(),
                            message_id: assistant_message_id.clone(),
                            delta: Some(delta.to_string()),
                            content: None,
                            status: "streaming".to_string(),
                            error: None,
                            revision: Some(revision),
                            schema_version: crate::chat::STREAM_EVENT_SCHEMA_VERSION,
                        },
                    )
                    .ok();

                Ok(())
            };
            let stream_future = runtime.stream_chat(provider_request, &mut on_delta);
            tokio::pin!(stream_future);
            tokio::select! {
                result = &mut stream_future => result,
                _ = cancellation.notified.notified() => {
                    Err(AppError::new("cancelled", "Generation was cancelled."))
                }
            }
        };

        // Final flush: whatever the outcome, any content buffered since the last checkpoint
        // must reach durable storage before the terminal status transition below, or it would
        // be silently lost.
        if !buffer.is_empty() {
            if let Ok(db) = crate::commands::lock_db(&app_for_task.state::<AppState>()) {
                db.append_to_message_content(&assistant_message_id, &buffer)
                    .ok();
            }
        }

        let was_cancelled = cancellation.is_requested();
        match stream_result {
            Ok(usage) if !was_cancelled => {
                // Conditional finish: if `cancel_stream` raced this and already committed
                // `cancelled` durably, this becomes a no-op and we must not tell the UI the
                // generation completed — cancellation is the authoritative terminal result.
                let state = app_for_task.state::<AppState>();
                let db = crate::commands::lock_db(&state).ok();
                let became_complete = db
                    .as_ref()
                    .and_then(|db| {
                        db.finish_message_if_active(
                            &assistant_message_id,
                            "complete",
                            None,
                            usage.input_tokens,
                            usage.output_tokens,
                        )
                        .ok()
                    })
                    .unwrap_or(false);

                if became_complete {
                    let final_content = db
                        .as_ref()
                        .and_then(|db| db.get_message(&assistant_message_id).ok())
                        .map(|m| m.content);
                    app_for_task
                        .emit(
                            "chat:stream-complete",
                            StreamEvent {
                                conversation_id: conversation_id.clone(),
                                message_id: assistant_message_id.clone(),
                                delta: None,
                                content: final_content,
                                status: "complete".to_string(),
                                error: None,
                                revision: None,
                                schema_version: crate::chat::STREAM_EVENT_SCHEMA_VERSION,
                            },
                        )
                        .ok();
                }
            }
            Ok(_) => {
                mark_stream_cancelled(&app_for_task, &conversation_id, &assistant_message_id);
            }
            Err(error) if was_cancelled || error.code == "cancelled" => {
                mark_stream_cancelled(&app_for_task, &conversation_id, &assistant_message_id);
            }
            // COR-003: a stream that ended without a valid completion marker, hit a malformed
            // frame, or went idle too long is a protocol-level interruption, not a generic
            // failure — partial content is preserved and offered through the same Retry/Keep
            // partial/Discard recovery flow as a crash-interrupted generation (COR-001).
            Err(error)
                if matches!(
                    error.code.as_str(),
                    "stream_incomplete"
                        | "protocol_error"
                        | "stream_header_timeout"
                        | "stream_idle_timeout"
                        | "stream_user_deadline"
                ) =>
            {
                mark_stream_interrupted(
                    &app_for_task,
                    &conversation_id,
                    &assistant_message_id,
                    error,
                );
            }
            Err(error) => {
                mark_stream_failed(
                    &app_for_task,
                    &conversation_id,
                    &assistant_message_id,
                    error,
                    true,
                );
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

/// COR-005: uses the conditional finish so this becomes a harmless no-op — no duplicate
/// event, no clobbered status — when `cancel_stream` has already synchronously finalized
/// the message as `cancelled` before this background-task path observed the cancel flag.
fn mark_stream_cancelled(app: &AppHandle, conversation_id: &str, message_id: &str) {
    let state = app.state::<AppState>();
    let db = crate::commands::lock_db(&state).ok();
    let became_cancelled = db
        .as_ref()
        .and_then(|db| {
            db.finish_message_if_active(
                message_id,
                "cancelled",
                Some("Generation was cancelled."),
                None,
                None,
            )
            .ok()
        })
        .unwrap_or(false);

    if !became_cancelled {
        return;
    }

    // COR-011: include the final flushed content once, on this terminal event, as a
    // correctness backstop in case any delta was dropped in transit.
    let final_content = db
        .as_ref()
        .and_then(|db| db.get_message(message_id).ok())
        .map(|m| m.content);

    app.emit(
        "chat:stream-cancelled",
        StreamEvent {
            conversation_id: conversation_id.to_string(),
            message_id: message_id.to_string(),
            delta: None,
            content: final_content,
            status: "cancelled".to_string(),
            error: Some("Generation was cancelled.".to_string()),
            revision: None,
            schema_version: crate::chat::STREAM_EVENT_SCHEMA_VERSION,
        },
    )
    .ok();
}

/// COR-003: distinct from `mark_stream_cancelled` (user action) and `mark_stream_failed`
/// (generic/unrecoverable provider error) — this is specifically for a stream that ended
/// without a valid protocol completion marker, hit a malformed frame, or went idle too long.
/// Uses the conditional finish for the same racing-writer reasons as the other two.
fn mark_stream_interrupted(
    app: &AppHandle,
    conversation_id: &str,
    message_id: &str,
    error: AppError,
) {
    let state = app.state::<AppState>();
    let db = crate::commands::lock_db(&state).ok();
    let became_interrupted = db
        .as_ref()
        .and_then(|db| {
            db.finish_message_if_active(message_id, "interrupted", Some(&error.message), None, None)
                .ok()
        })
        .unwrap_or(false);

    if !became_interrupted {
        return;
    }

    let final_content = db
        .as_ref()
        .and_then(|db| db.get_message(message_id).ok())
        .map(|m| m.content);

    app.emit(
        "chat:stream-interrupted",
        StreamEvent {
            conversation_id: conversation_id.to_string(),
            message_id: message_id.to_string(),
            delta: None,
            content: final_content,
            status: "interrupted".to_string(),
            error: Some(error.message),
            revision: None,
            schema_version: crate::chat::STREAM_EVENT_SCHEMA_VERSION,
        },
    )
    .ok();
}

/// COR-005: uses the conditional finish for the same reason as `mark_stream_cancelled` — a
/// provider failure racing a user cancellation must not overwrite an already-durable
/// `cancelled` status, and must not tell the UI the generation failed once it's already
/// been resolved as cancelled.
fn mark_stream_failed(
    app: &AppHandle,
    conversation_id: &str,
    message_id: &str,
    error: AppError,
    emit_error: bool,
) {
    let state = app.state::<AppState>();
    // OPS-001: the error *code* only, never `.message` — a provider's own error text can carry
    // arbitrary response content, and this log may end up in an exported diagnostics bundle
    // handed to someone else. The stable code is what a support conversation actually needs.
    if let Ok(mut log) = state.observability_log.lock() {
        log.record(
            crate::observability::LogLevel::Error,
            "generation",
            Some(message_id),
            &format!("stream failed: {}", error.code),
        );
    }
    let db = crate::commands::lock_db(&state).ok();
    let became_failed = db
        .as_ref()
        .and_then(|db| {
            db.finish_message_if_active(message_id, "failed", Some(&error.message), None, None)
                .ok()
        })
        .unwrap_or(false);

    if emit_error && became_failed {
        let final_content = db
            .as_ref()
            .and_then(|db| db.get_message(message_id).ok())
            .map(|m| m.content);
        app.emit(
            "chat:stream-error",
            StreamEvent {
                conversation_id: conversation_id.to_string(),
                message_id: message_id.to_string(),
                delta: None,
                content: final_content,
                status: "failed".to_string(),
                error: Some(error.message),
                revision: None,
                schema_version: crate::chat::STREAM_EVENT_SCHEMA_VERSION,
            },
        )
        .ok();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::DEFAULT_PROVIDER_ID;
    use crate::db::Database;
    use crate::providers::ProviderCapabilities;
    use crate::sidecar::SidecarState;
    use std::collections::HashMap;
    use std::fs;
    use std::sync::{Arc, Barrier, Mutex};
    use uuid::Uuid;

    fn test_state() -> (AppState, std::path::PathBuf) {
        let path = std::env::temp_dir().join(format!(
            "ark-generation-ordering-test-{}.sqlite3",
            Uuid::new_v4()
        ));
        let db = Database::open(&path).expect("writer opens");
        let read_db = Database::open_read_replica(&path).expect("read replica opens");
        (
            AppState {
                db: Mutex::new(db),
                workspace: Mutex::new(crate::workspace::WorkspaceInfo {
                    root_path: path
                        .parent()
                        .expect("test path parent")
                        .display()
                        .to_string(),
                    database_path: path.display().to_string(),
                    default_root_path: path
                        .parent()
                        .expect("test path parent")
                        .display()
                        .to_string(),
                    config_path: path.with_extension("json").display().to_string(),
                    is_portable: false,
                    requires_restart: false,
                }),
                read_db: Mutex::new(read_db),
                workspace_open_error: Mutex::new(None),
                active_streams: Mutex::new(HashMap::new()),
                pending_streams: Mutex::new(HashMap::new()),
                active_imports: Mutex::new(HashMap::new()),
                storage_maintenance: AtomicBool::new(false),
                sidecar: Arc::new(Mutex::new(SidecarState::new())),
                observability_log: Arc::new(
                    Mutex::new(crate::observability::DiagnosticsLog::new()),
                ),
            },
            path,
        )
    }

    fn remove_test_database(path: &std::path::Path) {
        for candidate in [
            path.to_path_buf(),
            path.with_extension("sqlite3-wal"),
            path.with_extension("sqlite3-shm"),
        ] {
            let _ = fs::remove_file(candidate);
        }
    }

    fn basic_send_request(conversation_id: String, content: &str) -> SendChatRequest {
        SendChatRequest {
            conversation_id,
            content: content.to_string(),
            provider_id: DEFAULT_PROVIDER_ID.to_string(),
            model: "test-model".to_string(),
            temperature: None,
            max_tokens: None,
        }
    }

    #[test]
    fn send_returns_durable_ids_while_provider_work_is_still_only_queued() {
        let (state, path) = test_state();
        let conversation = state
            .db
            .lock()
            .expect("database lock")
            .create_conversation(Some("Ordering".to_string()))
            .expect("conversation created");

        let result = send_chat_message(
            &state,
            SendChatRequest {
                conversation_id: conversation.id,
                content: "immediate provider".to_string(),
                provider_id: DEFAULT_PROVIDER_ID.to_string(),
                model: "test-model".to_string(),
                temperature: None,
                max_tokens: None,
            },
        )
        .expect("durable generation is queued");

        let db = state.db.lock().expect("database lock");
        assert_eq!(
            db.get_message(&result.user_message_id)
                .expect("user message is durable")
                .status,
            "complete"
        );
        assert_eq!(
            db.get_message(&result.assistant_message_id)
                .expect("assistant placeholder is durable")
                .status,
            "streaming"
        );
        drop(db);
        assert!(state
            .pending_streams
            .lock()
            .expect("pending lock")
            .contains_key(&result.assistant_message_id));
        assert!(state.active_streams.lock().expect("active lock").is_empty());

        drop(state);
        remove_test_database(&path);
    }

    #[test]
    fn send_transaction_rolls_back_at_every_write_boundary() {
        let fault_triggers = [
            "CREATE TEMP TRIGGER inject_failure BEFORE INSERT ON messages
             WHEN NEW.role = 'user' BEGIN SELECT RAISE(ABORT, 'after user lookup'); END;",
            "CREATE TEMP TRIGGER inject_failure BEFORE UPDATE OF title ON conversations
             WHEN NEW.title <> OLD.title BEGIN SELECT RAISE(ABORT, 'after user insert'); END;",
            "CREATE TEMP TRIGGER inject_failure BEFORE INSERT ON messages
             WHEN NEW.role = 'assistant' BEGIN SELECT RAISE(ABORT, 'after title update'); END;",
            "CREATE TEMP TRIGGER inject_failure BEFORE UPDATE OF current_message_id ON conversations
             WHEN NEW.current_message_id IS NOT OLD.current_message_id
             BEGIN SELECT RAISE(ABORT, 'after assistant insert'); END;",
        ];

        for trigger in fault_triggers {
            let (state, path) = test_state();
            let conversation = state
                .db
                .lock()
                .expect("database lock")
                .create_conversation(None)
                .expect("conversation created");
            state
                .db
                .lock()
                .expect("database lock")
                .execute_batch_for_test(trigger)
                .expect("fault trigger installed");

            let error = send_chat_message(
                &state,
                basic_send_request(conversation.id.clone(), "transaction fault"),
            )
            .expect_err("fault injection must abort send");
            assert_eq!(error.code, "database_error");

            let db = state.db.lock().expect("database lock");
            let after = db
                .get_conversation(&conversation.id)
                .expect("conversation remains readable");
            assert_eq!(after.title, "New conversation");
            assert!(after.current_message_id.is_none());
            assert!(db
                .get_active_messages(&conversation.id)
                .expect("messages remain readable")
                .is_empty());
            drop(db);
            assert!(state
                .pending_streams
                .lock()
                .expect("pending lock")
                .is_empty());
            drop(state);
            remove_test_database(&path);
        }
    }

    #[test]
    fn edit_and_regenerate_transactions_preserve_ancestry_at_every_write_boundary() {
        let cases = [
            (
                "edit-user",
                "CREATE TEMP TRIGGER inject_failure BEFORE INSERT ON messages
                 WHEN NEW.role = 'user' AND NEW.revision_of_message_id IS NOT NULL
                 BEGIN SELECT RAISE(ABORT, 'edit user'); END;",
            ),
            (
                "edit-assistant",
                "CREATE TEMP TRIGGER inject_failure BEFORE INSERT ON messages
                 WHEN NEW.role = 'assistant' AND NEW.status = 'streaming'
                 BEGIN SELECT RAISE(ABORT, 'edit assistant'); END;",
            ),
            (
                "edit-pointer",
                "CREATE TEMP TRIGGER inject_failure BEFORE UPDATE OF current_message_id ON conversations
                 WHEN NEW.current_message_id IS NOT OLD.current_message_id
                 BEGIN SELECT RAISE(ABORT, 'edit pointer'); END;",
            ),
            (
                "regenerate-assistant",
                "CREATE TEMP TRIGGER inject_failure BEFORE INSERT ON messages
                 WHEN NEW.role = 'assistant' AND NEW.revision_of_message_id IS NOT NULL
                 BEGIN SELECT RAISE(ABORT, 'regenerate assistant'); END;",
            ),
            (
                "regenerate-pointer",
                "CREATE TEMP TRIGGER inject_failure BEFORE UPDATE OF current_message_id ON conversations
                 WHEN NEW.current_message_id IS NOT OLD.current_message_id
                 BEGIN SELECT RAISE(ABORT, 'regenerate pointer'); END;",
            ),
        ];

        for (operation, trigger) in cases {
            let (state, path) = test_state();
            let (conversation_id, user_id, assistant_id) = {
                let db = state.db.lock().expect("database lock");
                let conversation = db
                    .create_conversation(Some("Ancestry".to_string()))
                    .expect("conversation created");
                let user = db
                    .append_message(
                        &conversation.id,
                        None,
                        None,
                        "user",
                        "original",
                        "complete",
                        Some(DEFAULT_PROVIDER_ID),
                        Some("test-model"),
                    )
                    .expect("original user");
                let assistant = db
                    .append_message(
                        &conversation.id,
                        Some(&user.id),
                        None,
                        "assistant",
                        "original answer",
                        "complete",
                        Some(DEFAULT_PROVIDER_ID),
                        Some("test-model"),
                    )
                    .expect("original assistant");
                db.set_conversation_current_message(
                    &conversation.id,
                    &assistant.id,
                    DEFAULT_PROVIDER_ID,
                    "test-model",
                )
                .expect("initial branch selected");
                db.execute_batch_for_test(trigger)
                    .expect("fault trigger installed");
                (conversation.id, user.id, assistant.id)
            };

            let error = if operation.starts_with("edit") {
                edit_user_message(
                    &state,
                    EditUserMessageRequest {
                        conversation_id: conversation_id.clone(),
                        message_id: user_id.clone(),
                        content: "edited".to_string(),
                        provider_id: DEFAULT_PROVIDER_ID.to_string(),
                        model: "test-model".to_string(),
                        temperature: None,
                        max_tokens: None,
                    },
                )
                .expect_err("fault injection must abort edit")
            } else {
                regenerate_assistant_message(
                    &state,
                    RegenerateAssistantMessageRequest {
                        conversation_id: conversation_id.clone(),
                        message_id: assistant_id.clone(),
                        provider_id: DEFAULT_PROVIDER_ID.to_string(),
                        model: "test-model".to_string(),
                        temperature: None,
                        max_tokens: None,
                    },
                )
                .expect_err("fault injection must abort regeneration")
            };
            assert_eq!(error.code, "database_error", "case {operation}");

            let db = state.db.lock().expect("database lock");
            let after = db
                .get_conversation(&conversation_id)
                .expect("conversation remains");
            assert_eq!(
                after.current_message_id.as_deref(),
                Some(assistant_id.as_str())
            );
            let path_messages = db
                .get_active_messages(&conversation_id)
                .expect("active ancestry remains");
            assert_eq!(path_messages.len(), 2, "case {operation}");
            assert_eq!(path_messages[0].id, user_id);
            assert_eq!(path_messages[1].id, assistant_id);
            drop(db);
            assert!(state
                .pending_streams
                .lock()
                .expect("pending lock")
                .is_empty());
            drop(state);
            remove_test_database(&path);
        }
    }

    #[test]
    fn provider_launch_preflight_failure_compensates_the_committed_placeholder() {
        let (state, path) = test_state();
        let conversation = state
            .db
            .lock()
            .expect("database lock")
            .create_conversation(Some("Launch failure".to_string()))
            .expect("conversation created");
        let assistant = state
            .db
            .lock()
            .expect("database lock")
            .append_message(
                &conversation.id,
                None,
                None,
                "assistant",
                "",
                "streaming",
                Some(DEFAULT_PROVIDER_ID),
                Some("test-model"),
            )
            .expect("placeholder committed");
        let provider = ProviderConfig {
            id: "unsupported-provider".to_string(),
            name: "Unsupported".to_string(),
            provider_type: "unsupported".to_string(),
            base_url: Some("http://127.0.0.1".to_string()),
            api_key_ref: None,
            default_model_id: None,
            default_temperature: None,
            default_max_tokens: None,
            is_local: true,
            allow_insecure_remote: false,
            destination_class: "loopback".to_string(),
            capabilities: ProviderCapabilities::for_provider_type("unsupported"),
            is_enabled: true,
            created_at: "2026-08-14T00:00:00Z".to_string(),
            updated_at: "2026-08-14T00:00:00Z".to_string(),
        };
        let error = queue_provider_stream(
            &state,
            provider,
            ProviderChatRequest {
                model: "test-model".to_string(),
                messages: Vec::new(),
                temperature: None,
                max_tokens: None,
                user_deadline: None,
            },
            conversation.id,
            assistant.id.clone(),
        )
        .expect_err("unsupported provider cannot launch");
        assert_eq!(error.code, "invalid_input");
        let failed = state
            .db
            .lock()
            .expect("database lock")
            .get_message(&assistant.id)
            .expect("placeholder remains durable");
        assert_eq!(failed.status, "failed");
        assert!(failed.error_message.as_deref().is_some_and(
            |message| message.contains("Provider type 'unsupported' is not supported")
        ));
        drop(state);
        remove_test_database(&path);
    }

    #[test]
    fn concurrent_sends_on_one_conversation_are_serialized_into_one_coherent_branch() {
        const OPERATIONS: usize = 8;
        let (state, path) = test_state();
        let conversation_id = state
            .db
            .lock()
            .expect("database lock")
            .create_conversation(Some("Concurrent".to_string()))
            .expect("conversation created")
            .id;
        let state = Arc::new(state);
        let barrier = Arc::new(Barrier::new(OPERATIONS));
        let mut workers = Vec::new();
        for index in 0..OPERATIONS {
            let state = Arc::clone(&state);
            let barrier = Arc::clone(&barrier);
            let conversation_id = conversation_id.clone();
            workers.push(std::thread::spawn(move || {
                barrier.wait();
                send_chat_message(
                    &state,
                    basic_send_request(conversation_id, &format!("message {index}")),
                )
            }));
        }
        for worker in workers {
            worker
                .join()
                .expect("worker did not panic")
                .expect("serialized send succeeds");
        }

        let db = state.db.lock().expect("database lock");
        let branch = db
            .get_active_messages(&conversation_id)
            .expect("coherent active branch");
        assert_eq!(branch.len(), OPERATIONS * 2);
        for pair in branch.chunks_exact(2) {
            assert_eq!(pair[0].role, "user");
            assert_eq!(pair[1].role, "assistant");
            assert_eq!(
                pair[1].parent_message_id.as_deref(),
                Some(pair[0].id.as_str())
            );
        }
        drop(db);
        drop(state);
        remove_test_database(&path);
    }

    #[tokio::test]
    async fn cancellation_notification_is_retained_when_requested_before_the_task_waits() {
        let cancellation = StreamCancellation::new();
        cancellation.request();
        tokio::time::timeout(
            std::time::Duration::from_millis(20),
            cancellation.notified.notified(),
        )
        .await
        .expect("a pre-start cancellation retains its wakeup permit");
        assert!(cancellation.is_requested());
    }

    #[test]
    fn durable_cancellation_is_idempotent_preserves_partial_output_and_meets_ack_budget() {
        let (state, path) = test_state();
        let conversation = state
            .db
            .lock()
            .expect("database lock")
            .create_conversation(Some("Cancellation".to_string()))
            .expect("conversation created");
        let result = send_chat_message(
            &state,
            SendChatRequest {
                conversation_id: conversation.id,
                content: "cancel me".to_string(),
                provider_id: DEFAULT_PROVIDER_ID.to_string(),
                model: "test-model".to_string(),
                temperature: None,
                max_tokens: None,
            },
        )
        .expect("generation queued");
        state
            .db
            .lock()
            .expect("database lock")
            .append_to_message_content(&result.assistant_message_id, "partial")
            .expect("partial checkpoint");

        let started = std::time::Instant::now();
        let (_, first_changed) = request_cancellation(&state, &result.assistant_message_id)
            .expect("first cancellation succeeds");
        let elapsed = started.elapsed();
        let (_, second_changed) = request_cancellation(&state, &result.assistant_message_id)
            .expect("repeated cancellation succeeds");

        assert!(first_changed);
        assert!(!second_changed);
        assert!(
            elapsed < std::time::Duration::from_millis(100),
            "durable acknowledgement took {elapsed:?}"
        );
        let cancelled = state
            .db
            .lock()
            .expect("database lock")
            .get_message(&result.assistant_message_id)
            .expect("cancelled message");
        assert_eq!(cancelled.status, "cancelled");
        assert_eq!(cancelled.content, "partial");
        assert!(!state
            .pending_streams
            .lock()
            .expect("pending lock")
            .contains_key(&result.assistant_message_id));

        drop(state);
        for candidate in [
            path.clone(),
            path.with_extension("sqlite3-wal"),
            path.with_extension("sqlite3-shm"),
        ] {
            let _ = fs::remove_file(candidate);
        }
    }

    // ── FTR-004/FTR-003: four-tier settings precedence + provenance ────────────────────

    #[test]
    fn resolve_setting_prefers_request_then_conversation_then_project_then_provider_default() {
        assert_eq!(
            resolve_setting(Some(1), Some(2), Some(3), Some(4)),
            (Some(1), Some(SettingSource::Request))
        );
        assert_eq!(
            resolve_setting(None, Some(2), Some(3), Some(4)),
            (Some(2), Some(SettingSource::Conversation))
        );
        assert_eq!(
            resolve_setting(None, None, Some(3), Some(4)),
            (Some(3), Some(SettingSource::Project))
        );
        assert_eq!(
            resolve_setting(None, None, None, Some(4)),
            (Some(4), Some(SettingSource::ProviderDefault))
        );
        assert_eq!(resolve_setting::<i64>(None, None, None, None), (None, None));
    }

    #[test]
    fn resolve_system_prompt_prefers_conversation_then_project() {
        assert_eq!(
            resolve_system_prompt(Some("Be terse."), Some("Cite sources.")),
            (
                Some("Be terse.".to_string()),
                Some(SettingSource::Conversation)
            )
        );
        assert_eq!(
            resolve_system_prompt(None, Some("Cite sources.")),
            (
                Some("Cite sources.".to_string()),
                Some(SettingSource::Project)
            )
        );
        assert_eq!(resolve_system_prompt(None, None), (None, None));
    }

    #[test]
    fn send_chat_message_records_provider_default_provenance_when_no_override_is_set() {
        let (state, path) = test_state();
        let conversation = state
            .db
            .lock()
            .expect("database lock")
            .create_conversation(Some("Provenance default".to_string()))
            .expect("conversation created");

        let result = send_chat_message(&state, basic_send_request(conversation.id, "hello"))
            .expect("generation queued");

        let db = state.db.lock().expect("database lock");
        let provider = db
            .get_provider(DEFAULT_PROVIDER_ID)
            .expect("default provider readable");
        let assistant = db
            .get_message(&result.assistant_message_id)
            .expect("assistant placeholder readable");
        let metadata = assistant
            .metadata_json
            .expect("provenance metadata was recorded");
        let parsed: serde_json::Value =
            serde_json::from_str(&metadata).expect("provenance is valid JSON");
        assert_eq!(parsed["temperature"].as_f64(), provider.default_temperature);
        assert_eq!(parsed["temperatureSource"], "provider_default");
        assert_eq!(parsed["maxTokens"].as_i64(), provider.default_max_tokens);
        assert_eq!(parsed["maxTokensSource"], "provider_default");
        assert!(parsed["systemPromptSource"].is_null());

        drop(db);
        drop(state);
        remove_test_database(&path);
    }

    #[test]
    fn send_chat_message_records_conversation_override_provenance_and_applies_the_system_prompt() {
        let (state, path) = test_state();
        let conversation = {
            let db = state.db.lock().expect("database lock");
            let conversation = db
                .create_conversation(Some("Provenance override".to_string()))
                .expect("conversation created");
            db.update_conversation_settings(
                &conversation.id,
                Some("Be terse."),
                Some(0.1),
                Some(64),
            )
            .expect("conversation settings saved")
        };

        let result = send_chat_message(&state, basic_send_request(conversation.id, "hello"))
            .expect("generation queued");

        let db = state.db.lock().expect("database lock");
        let assistant = db
            .get_message(&result.assistant_message_id)
            .expect("assistant placeholder readable");
        let metadata = assistant
            .metadata_json
            .expect("provenance metadata was recorded");
        let parsed: serde_json::Value =
            serde_json::from_str(&metadata).expect("provenance is valid JSON");
        assert_eq!(parsed["temperature"].as_f64(), Some(0.1));
        assert_eq!(parsed["temperatureSource"], "conversation");
        assert_eq!(parsed["maxTokens"].as_i64(), Some(64));
        assert_eq!(parsed["maxTokensSource"], "conversation");
        assert_eq!(parsed["systemPromptSource"], "conversation");

        drop(db);
        drop(state);
        remove_test_database(&path);
    }

    #[test]
    fn send_chat_message_request_override_takes_precedence_over_the_conversation_setting() {
        let (state, path) = test_state();
        let conversation = {
            let db = state.db.lock().expect("database lock");
            let conversation = db
                .create_conversation(Some("Request wins".to_string()))
                .expect("conversation created");
            db.update_conversation_settings(&conversation.id, None, Some(0.1), Some(64))
                .expect("conversation settings saved")
        };

        let mut request = basic_send_request(conversation.id, "hello");
        request.temperature = Some(0.9);
        request.max_tokens = Some(128);
        let result = send_chat_message(&state, request).expect("generation queued");

        let db = state.db.lock().expect("database lock");
        let assistant = db
            .get_message(&result.assistant_message_id)
            .expect("assistant placeholder readable");
        let metadata = assistant
            .metadata_json
            .expect("provenance metadata was recorded");
        let parsed: serde_json::Value =
            serde_json::from_str(&metadata).expect("provenance is valid JSON");
        assert_eq!(parsed["temperature"].as_f64(), Some(0.9));
        assert_eq!(parsed["temperatureSource"], "request");
        assert_eq!(parsed["maxTokens"].as_i64(), Some(128));
        assert_eq!(parsed["maxTokensSource"], "request");

        drop(db);
        drop(state);
        remove_test_database(&path);
    }

    #[test]
    fn send_chat_message_falls_through_to_the_projects_defaults_when_the_conversation_has_none() {
        let (state, path) = test_state();
        let conversation = {
            let db = state.db.lock().expect("database lock");
            let project = db.create_project("Research").expect("project created");
            let project = db
                .update_project(
                    &project.id,
                    crate::projects::UpdateProjectChanges {
                        name: "Research",
                        instructions: Some("Cite sources."),
                        default_provider_id: None,
                        default_model_id: None,
                        default_temperature: Some(0.2),
                        default_max_tokens: Some(256),
                    },
                )
                .expect("project defaults saved");
            let conversation = db
                .create_conversation(Some("In a project".to_string()))
                .expect("conversation created");
            db.set_conversation_project(&conversation.id, Some(&project.id))
                .expect("conversation assigned to project")
        };

        let result = send_chat_message(&state, basic_send_request(conversation.id, "hello"))
            .expect("generation queued");

        let db = state.db.lock().expect("database lock");
        let assistant = db
            .get_message(&result.assistant_message_id)
            .expect("assistant placeholder readable");
        let metadata = assistant
            .metadata_json
            .expect("provenance metadata was recorded");
        let parsed: serde_json::Value =
            serde_json::from_str(&metadata).expect("provenance is valid JSON");
        assert_eq!(parsed["temperature"].as_f64(), Some(0.2));
        assert_eq!(parsed["temperatureSource"], "project");
        assert_eq!(parsed["maxTokens"].as_i64(), Some(256));
        assert_eq!(parsed["maxTokensSource"], "project");
        assert_eq!(parsed["systemPromptSource"], "project");
        assert!(parsed["projectId"].is_string());

        drop(db);
        drop(state);
        remove_test_database(&path);
    }
}
