//! CODE-007 (partial): the durable Ark Code agent executor.
//!
//! `run_step` drives exactly one model turn of an existing `queued`/`observing` run: it checks
//! budgets and repository identity, claims `planning`, calls the selected provider through
//! `providers::stream_tools_for_model` with CODE-004's Repository tool schemas. Read calls execute
//! directly; `edit_file` becomes a durable proposal and pauses for explicit human approval. It
//! executes at most one requested tool call, and
//! commits the step, moving the run to `observing` (a tool ran) or `completed` (the model gave a
//! final answer). `start_run` owns the normal production path and advances those steps in a
//! background task until a terminal or pause state; `run_step` remains only as a focused internal
//! seam and development command. Cancellation is durably requested before its in-process wakeup.
//! Incremental assistant-text streaming remains outstanding — see `implementation-plan.md`.
//!
//! Every DB write happens through `Database::transition_code_agent_run`/`commit_code_agent_step`,
//! which the ADR requires. The DB mutex is never held across the provider call or tool execution
//! (both `.await` points): each phase locks, reads or writes, and unlocks before the next `.await`,
//! matching `generation.rs`'s existing discipline.

use crate::chat::ChatMessage;
use crate::code_sessions::{
    CodeAgentStepState, CodeObservationKind, CodeRunDetail, CodeRunState, FinishCodeAgentStep,
    NewCodeAgentStep, NewCodeAgentStepClaim, NewCodeToolCallOutcome,
};
use crate::code_tools::RepositoryContext;
use crate::errors::AppError;
use crate::providers::{
    stream_tools_for_model, ProviderChatRequest, ProviderContextBlock, ProviderContextKind,
    ProviderRegistry, ProviderToolCall, ProviderToolDefinition, ProviderToolEvent,
    ProviderToolExchange, ProviderToolRequest,
};
use crate::AppState;
use chrono::{Duration as ChronoDuration, Utc};
use std::collections::{HashMap, HashSet};
use std::future::pending;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};
use uuid::Uuid;

#[derive(serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct ClarificationArguments {
    question: String,
}

const MAX_OBSERVATION_CONTENT_CHARS: usize = 8_000;
const MIN_STEP_TOKEN_BUDGET: u64 = 256;
const EXECUTOR_LEASE_SECONDS: i64 = 30;
const EXECUTOR_HEARTBEAT_SECONDS: u64 = 10;

/// Process-local wakeup for a durable cancellation request. Dropping/restarting Ark loses this
/// handle but not the request in SQLite; startup recovery remains governed by ADR 0003.
pub(crate) struct CodeRunCancellation {
    requested: AtomicBool,
    notified: tokio::sync::Notify,
}

impl CodeRunCancellation {
    pub(crate) fn new() -> Self {
        Self {
            requested: AtomicBool::new(false),
            notified: tokio::sync::Notify::new(),
        }
    }

    pub(crate) fn request(&self) {
        self.requested.store(true, Ordering::Release);
        self.notified.notify_one();
    }

    pub(crate) fn is_requested(&self) -> bool {
        self.requested.load(Ordering::Acquire)
    }

    pub(crate) async fn cancelled(&self) {
        if self.is_requested() {
            return;
        }
        self.notified.notified().await;
    }
}

pub async fn run_step(
    state: &AppState,
    session_id: &str,
    run_id: &str,
) -> Result<CodeRunDetail, AppError> {
    // The explicit single-step development seam must retain the same lease across consecutive
    // observing steps, just like the production background loop does.
    let executor_lease_id = format!("manual-step:{run_id}");
    run_step_with_cancellation(state, session_id, run_id, &executor_lease_id, None).await
}

async fn run_step_with_cancellation(
    state: &AppState,
    session_id: &str,
    run_id: &str,
    executor_lease_id: &str,
    cancellation: Option<&CodeRunCancellation>,
) -> Result<CodeRunDetail, AppError> {
    let ready = prepare_step(state, session_id, run_id)?;
    let Some(ready) = ready else {
        // A budget/identity/provider-readiness check already transitioned the run to a terminal
        // or interrupted state; the caller re-fetches the resulting detail.
        return crate::commands::lock_read_db(state)?.get_code_run_detail(run_id);
    };

    let bearer_token = crate::secret_store::resolve_bearer_token(state, &ready.provider_config)?;
    let provider = match ProviderRegistry::create_with_bearer_token(
        ready.provider_config.clone(),
        bearer_token,
    ) {
        Ok(provider) => provider,
        Err(error) => {
            fail_run(
                state,
                run_id,
                CodeRunState::Interrupted,
                "provider_unavailable",
                &error.message,
            )?;
            return crate::commands::lock_read_db(state)?.get_code_run_detail(run_id);
        }
    };

    let allowed_commands = if ready.command_definitions.is_empty() {
        "No verification commands are currently enabled.".to_string()
    } else {
        format!(
            "Enabled verification commands (select only by ID): {}",
            ready
                .command_definitions
                .iter()
                .map(|command| format!("{} = {}", command.id, command.label))
                .collect::<Vec<_>>()
                .join("; ")
        )
    };
    let evidence_instruction = if ready.has_unresolved_tool_error {
        "The most recent tool call failed. You must correct its arguments or use another appropriate tool successfully before providing a final answer. Never treat a tool validation or execution error as evidence about the user's repository defect."
    } else if ready.has_repository_content_evidence {
        "You have successful Repository content evidence in the causal history. Base every Repository-specific claim on that evidence and cite the exact inspected paths."
    } else {
        "You do not yet have successful Repository content evidence. Your next response must call read_file or search with schema-valid arguments; do not provide a final answer yet. A repository_map or list_directory result is navigation metadata only and is never evidence for file contents, architecture, authentication, storage, behavior, defects, or maintainability risks."
    };
    let system_instructions = format!(
        "You are Ark Code, a repository coding agent for the Repository at {}. \
         Use the provided tools to explore files, search text, and inspect Git status/diff so you \
         can answer the task below. For broad exploration, prefer one repository_map call, then \
         inspect likely manifests, entry points, and only targeted search terms. Every prior tool \
         call and its result is supplied causally; do not repeat a completed call unless its result \
         explicitly justifies a retry, and change invalid arguments after a tool error. {evidence_instruction} Never infer a framework, backend, database, authentication flow, route, feature, defect, or risk from filenames or dependencies alone. For a requested fix, inspect the relevant implementation, establish an evidence-backed root cause, apply the smallest change, and run relevant verification before claiming completion. You may propose precise edit_file changes; Ark will display \
         the diff and only apply it after the user approves. Use git_checkpoint after verified \
         edits are ready to preserve. Never claim a proposed edit, checkpoint, rollback, or command was applied until \
         a later tool observation confirms it. {allowed_commands} When you have gathered enough information, respond in plain \
         text with your final answer and do not call another tool.",
        ready.run_repository_path
    );
    let tools = crate::code_tools::provider_tool_definitions();
    let allocation = match allocate_context(
        &system_instructions,
        &tools,
        &ready.conversation_turns,
        &ready.tool_history_candidates,
        &ready.context_candidates,
        ready.model_context_window,
        ready.remaining_run_tokens,
    ) {
        Ok(allocation) => allocation,
        Err(error) => {
            let (state_after_error, reason) = if error.code == "agent_token_budget_exhausted" {
                (CodeRunState::Failed, "agent_token_budget_exhausted")
            } else {
                (CodeRunState::Interrupted, "model_context_window_too_small")
            };
            fail_run(state, run_id, state_after_error, reason, &error.message)?;
            return crate::commands::lock_read_db(state)?.get_code_run_detail(run_id);
        }
    };
    let chat = ProviderChatRequest {
        model: ready.model_name.clone(),
        system_instructions: Some(system_instructions),
        messages: allocation.messages,
        untrusted_context: allocation.untrusted_context,
        tool_history: allocation.tool_history,
        temperature: None,
        max_tokens: Some(i64::try_from(allocation.max_output_tokens).unwrap_or(i64::MAX)),
        user_deadline: None,
    };
    let tool_request = ProviderToolRequest { chat, tools };

    let prompt_manifest_json = serde_json::json!({
        "task": ready.task,
        "modelContextWindow": ready.model_context_window,
        "estimatedInputTokens": allocation.estimated_input_tokens,
        "maxOutputTokens": allocation.max_output_tokens,
        "reservedTokens": allocation.reserved_tokens,
        "includedConversationMessages": tool_request.chat.messages.len(),
        "includedContextBlocks": tool_request.chat.untrusted_context.len(),
        "includedToolExchanges": tool_request.chat.tool_history.len(),
        "contextCompacted": allocation.compaction_summary.is_some(),
        "toolDefinitions": tool_request.tools.iter().map(|tool| &tool.name).collect::<Vec<_>>(),
    })
    .to_string();
    let step_id =
        crate::commands::lock_db(state)?.claim_code_agent_step(&NewCodeAgentStepClaim {
            run_id,
            step_index: ready.step_index,
            prompt_manifest_json: &prompt_manifest_json,
            context_compaction_summary: allocation.compaction_summary.as_deref(),
            reserved_tokens: allocation.reserved_tokens,
            reserved_cost_microunits: None,
            executor_lease_id,
            executor_lease_expires_at: &lease_expires_at(),
        })?;

    if cancellation.is_some_and(CodeRunCancellation::is_requested) {
        return finish_claimed_step(
            state,
            run_id,
            &step_id,
            executor_lease_id,
            CodeRunState::Cancelled,
            "user_cancelled_before_provider_dispatch",
            "The user stopped Ark Code before provider work was dispatched.",
            None,
            0,
            false,
        );
    }
    if let Err(error) = crate::commands::lock_db(state)?.mark_code_agent_step_dispatched(
        run_id,
        &step_id,
        executor_lease_id,
        &lease_expires_at(),
    ) {
        if error.code == "code_run_cancelled" {
            return finish_claimed_step(
                state,
                run_id,
                &step_id,
                executor_lease_id,
                CodeRunState::Cancelled,
                "user_cancelled_before_provider_dispatch",
                "The user stopped Ark Code before provider work was dispatched.",
                None,
                0,
                false,
            );
        }
        return Err(error);
    }

    let step_started = Instant::now();
    let (provider_result, text, tool_calls) = {
        let mut text = String::new();
        let mut tool_calls: Vec<ProviderToolCall> = Vec::new();
        let mut last_stream_checkpoint = Instant::now();
        let mut last_stream_chars = 0usize;
        let provider_result = {
            let mut on_event = |event: ProviderToolEvent| -> Result<(), AppError> {
                match event {
                    ProviderToolEvent::TextDelta { delta } => {
                        if text.chars().count() < MAX_OBSERVATION_CONTENT_CHARS {
                            text.push_str(&delta);
                        }
                        let current_chars = text.chars().count();
                        if current_chars > last_stream_chars
                            && (last_stream_checkpoint.elapsed() >= Duration::from_millis(75)
                                || current_chars.saturating_sub(last_stream_chars) >= 64)
                        {
                            crate::commands::lock_db(state)?.checkpoint_code_agent_streaming_text(
                                run_id,
                                &step_id,
                                executor_lease_id,
                                &text,
                            )?;
                            last_stream_checkpoint = Instant::now();
                            last_stream_chars = current_chars;
                        }
                        Ok(())
                    }
                    ProviderToolEvent::ToolCall { call } => {
                        tool_calls.push(call);
                        Ok(())
                    }
                    ProviderToolEvent::ToolResult { .. } => Ok(()),
                }
            };
            let provider_future = stream_tools_for_model(
                provider.as_ref(),
                &ready.model,
                tool_request,
                &mut on_event,
            );
            tokio::pin!(provider_future);
            let mut heartbeat =
                tokio::time::interval(Duration::from_secs(EXECUTOR_HEARTBEAT_SECONDS));
            heartbeat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            heartbeat.tick().await;
            loop {
                tokio::select! {
                    result = &mut provider_future => break result,
                    _ = wait_for_cancellation(cancellation) => {
                        return finish_claimed_step(
                            state,
                            run_id,
                            &step_id,
                            executor_lease_id,
                            CodeRunState::Interrupted,
                            "user_cancelled_during_provider",
                            "The user stopped Ark Code while provider work was in flight. The run was interrupted conservatively.",
                            None,
                            elapsed_ms(step_started),
                            true,
                        );
                    }
                    _ = heartbeat.tick() => renew_executor_lease(
                        state,
                        run_id,
                        executor_lease_id,
                    )?,
                }
            }
        };
        (provider_result, text, tool_calls)
    };
    let usage = match provider_result {
        Ok(usage) => usage,
        Err(error) => {
            return finish_claimed_step(
                state,
                run_id,
                &step_id,
                executor_lease_id,
                CodeRunState::Interrupted,
                "provider_error",
                &error.message,
                None,
                elapsed_ms(step_started),
                true,
            );
        }
    };

    let actual_tokens = Some(
        u64::try_from(
            usage
                .input_tokens
                .unwrap_or(0)
                .saturating_add(usage.output_tokens.unwrap_or(0)),
        )
        .unwrap_or(0),
    );
    if cancellation.is_some_and(CodeRunCancellation::is_requested) {
        return finish_claimed_step(
            state,
            run_id,
            &step_id,
            executor_lease_id,
            CodeRunState::Interrupted,
            "user_cancelled_after_provider",
            "The user stopped Ark Code after the provider responded and before another tool could run.",
            actual_tokens,
            elapsed_ms(step_started),
            false,
        );
    }

    // G2/RC-08: execute ALL read-only tool calls returned in one provider response. Approval-
    // requiring and special-control tools still execute at most once per step; additional ones
    // receive a typed rejection and are not executed. Silent loss is forbidden: every call from
    // the provider appears in the durable record as either executed or explicitly rejected.
    let mut tool_call_outcomes: Vec<NewCodeToolCallOutcome<'_>> = Vec::new();
    // Set after the first approval/clarification call so subsequent ones are rejected clearly.
    let mut special_call_handled = false;

    for call in &tool_calls {
        let scope_json = tool_scope_json(&call.name);

        let outcome = if call.name == crate::code_write_tools::EDIT_FILE_TOOL_ID
            || call.name == crate::code_git_tools::CHECKPOINT_TOOL_ID
            || call.name == crate::code_git_tools::ROLLBACK_TOOL_ID
            || call.name == crate::code_command_tools::RUN_COMMAND_TOOL_ID
            || call.name == crate::code_tools::REQUEST_CLARIFICATION_TOOL_ID
        {
            // Approval/special tool: only one per step.
            if special_call_handled {
                // Reject additional approval/special calls with a clear typed message.
                Some(NewCodeToolCallOutcome {
                    provider_call_id: call.provider_call_id.as_deref(),
                    tool_name: &call.name,
                    canonical_arguments_json: serde_json::to_string(&call.arguments)
                        .unwrap_or_default(),
                    scope_json,
                    succeeded: false,
                    observation_content: Some(
                        "Only one approval-requiring or special-control operation may be requested \
                         per step. Re-request this operation as your next action after the \
                         pending operation completes.".to_string(),
                    ),
                    approval_preview: None,
                    loop_detected: false,
                })
            } else {
                special_call_handled = true;
                // Delegate to per-tool handling (unchanged from previous single-call path).
                if call.name == crate::code_write_tools::EDIT_FILE_TOOL_ID {
                    match crate::code_write_tools::preview_provider_edit_file(
                        &ready.context,
                        &call.arguments,
                    ) {
                        Ok((canonical_arguments_json, preview)) => {
                            let repeated = crate::commands::lock_read_db(state)?
                                .would_repeat_code_tool_call_three_times(
                                    run_id,
                                    &call.name,
                                    &preview.call_hash,
                                )?;
                            Some(NewCodeToolCallOutcome {
                                provider_call_id: call.provider_call_id.as_deref(),
                                tool_name: &call.name,
                                canonical_arguments_json,
                                scope_json,
                                succeeded: !repeated,
                                observation_content: repeated.then(|| {
                                    "Ark stopped before a third consecutive identical tool call. \
                                     Revise the approach before retrying."
                                        .to_string()
                                }),
                                approval_preview: (!repeated).then(|| preview.into()),
                                loop_detected: repeated,
                            })
                        }
                        Err(error) => Some(NewCodeToolCallOutcome {
                            provider_call_id: call.provider_call_id.as_deref(),
                            tool_name: &call.name,
                            canonical_arguments_json: serde_json::to_string(&call.arguments)
                                .unwrap_or_default(),
                            scope_json,
                            succeeded: false,
                            observation_content: Some(truncate_prose(&error.message)),
                            approval_preview: None,
                            loop_detected: false,
                        }),
                    }
                } else if call.name == crate::code_git_tools::CHECKPOINT_TOOL_ID {
                    let decoded = serde_json::from_value::<
                        crate::code_git_tools::GitCheckpointArguments,
                    >(call.arguments.clone())
                    .map_err(|_| {
                        AppError::invalid_input(
                            "git_checkpoint arguments did not match the strict schema.",
                        )
                    });
                    match decoded {
                        Ok(arguments) => {
                            match crate::code_git_tools::preview_checkpoint(
                                &ready.context,
                                arguments,
                            )
                            .await
                            {
                                Ok(preview) => {
                                    let repeated = crate::commands::lock_read_db(state)?
                                        .would_repeat_code_tool_call_three_times(
                                            run_id,
                                            &call.name,
                                            &preview.call_hash,
                                        )?;
                                    Some(NewCodeToolCallOutcome {
                                        provider_call_id: call.provider_call_id.as_deref(),
                                        tool_name: &call.name,
                                        canonical_arguments_json: preview.arguments_json,
                                        scope_json,
                                        succeeded: !repeated,
                                        observation_content: repeated.then(|| {
                                            "Ark stopped before a third consecutive identical tool \
                                             call. Revise the approach before retrying."
                                                .to_string()
                                        }),
                                        approval_preview: (!repeated).then_some(
                                            crate::code_sessions::CodeApprovalPreview {
                                                content: preview.content,
                                                call_hash: preview.call_hash,
                                                preview_hash: preview.preview_hash,
                                                precondition_hash: preview.precondition_hash,
                                            },
                                        ),
                                        loop_detected: repeated,
                                    })
                                }
                                Err(error) => failed_tool_outcome(call, scope_json, &error),
                            }
                        }
                        Err(error) => failed_tool_outcome(call, scope_json, &error),
                    }
                } else if call.name == crate::code_git_tools::ROLLBACK_TOOL_ID {
                    let decoded = serde_json::from_value::<
                        crate::code_git_tools::GitRollbackArguments,
                    >(call.arguments.clone())
                    .map_err(|_| {
                        AppError::invalid_input(
                            "git_rollback arguments did not match the strict schema.",
                        )
                    });
                    match decoded {
                        Ok(arguments) => {
                            let policy = (|| {
                                let db = crate::commands::lock_read_db(state)?;
                                let repository = db.get_code_session_repository(session_id)?;
                                let target = db.get_code_git_checkpoint(
                                    session_id,
                                    &arguments.checkpoint_id,
                                )?;
                                let checkpoint_oids = db
                                    .list_code_git_checkpoints(session_id)?
                                    .into_iter()
                                    .map(|checkpoint| checkpoint.commit_oid)
                                    .collect::<Vec<_>>();
                                Ok::<_, AppError>((repository, target, checkpoint_oids))
                            })();
                            match policy {
                                Ok((repository, target, checkpoint_oids)) => {
                                    match crate::code_git_tools::preview_rollback(
                                        &ready.context,
                                        arguments,
                                        &target.commit_oid,
                                        &repository.base_commit_oid,
                                        &checkpoint_oids,
                                    )
                                    .await
                                    {
                                        Ok(preview) => {
                                            let repeated = crate::commands::lock_read_db(state)?
                                                .would_repeat_code_tool_call_three_times(
                                                    run_id,
                                                    &call.name,
                                                    &preview.call_hash,
                                                )?;
                                            Some(NewCodeToolCallOutcome {
                                                provider_call_id: call.provider_call_id.as_deref(),
                                                tool_name: &call.name,
                                                canonical_arguments_json: preview.arguments_json,
                                                scope_json,
                                                succeeded: !repeated,
                                                observation_content: repeated.then(|| {
                                                    "Ark stopped before a third consecutive identical \
                                                     tool call. Revise the approach before retrying."
                                                        .to_string()
                                                }),
                                                approval_preview: (!repeated).then_some(
                                                    crate::code_sessions::CodeApprovalPreview {
                                                        content: preview.content,
                                                        call_hash: preview.call_hash,
                                                        preview_hash: preview.preview_hash,
                                                        precondition_hash: preview.precondition_hash,
                                                    },
                                                ),
                                                loop_detected: repeated,
                                            })
                                        }
                                        Err(error) => failed_tool_outcome(call, scope_json, &error),
                                    }
                                }
                                Err(error) => failed_tool_outcome(call, scope_json, &error),
                            }
                        }
                        Err(error) => failed_tool_outcome(call, scope_json, &error),
                    }
                } else if call.name == crate::code_command_tools::RUN_COMMAND_TOOL_ID {
                    let decoded = serde_json::from_value::<
                        crate::code_command_tools::RunCommandArguments,
                    >(call.arguments.clone())
                    .map_err(|_| {
                        AppError::invalid_input(
                            "run_verification_command arguments did not match the strict schema.",
                        )
                    });
                    match decoded {
                        Ok(arguments) => {
                            let definition = crate::commands::lock_read_db(state).and_then(|db| {
                                db.get_code_command_definition(&arguments.command_id)
                            });
                            match definition.and_then(|definition| {
                                crate::code_command_tools::preview_command(
                                    &ready.context,
                                    arguments,
                                    definition,
                                )
                            }) {
                                Ok(preview) => {
                                    let repeated = crate::commands::lock_read_db(state)?
                                        .would_repeat_code_tool_call_three_times(
                                            run_id,
                                            &call.name,
                                            &preview.call_hash,
                                        )?;
                                    Some(NewCodeToolCallOutcome {
                                        provider_call_id: call.provider_call_id.as_deref(),
                                        tool_name: &call.name,
                                        canonical_arguments_json: preview.arguments_json,
                                        scope_json,
                                        succeeded: !repeated,
                                        observation_content: repeated.then(|| {
                                            "Ark stopped before a third consecutive identical tool \
                                             call. Revise the approach before retrying."
                                                .to_string()
                                        }),
                                        approval_preview: (!repeated).then_some(
                                            crate::code_sessions::CodeApprovalPreview {
                                                content: preview.content,
                                                call_hash: preview.call_hash,
                                                preview_hash: preview.preview_hash,
                                                precondition_hash: preview.precondition_hash,
                                            },
                                        ),
                                        loop_detected: repeated,
                                    })
                                }
                                Err(error) => failed_tool_outcome(call, scope_json, &error),
                            }
                        }
                        Err(error) => failed_tool_outcome(call, scope_json, &error),
                    }
                } else {
                    // request_clarification
                    match serde_json::from_value::<ClarificationArguments>(call.arguments.clone()) {
                        Ok(arguments)
                            if !arguments.question.trim().is_empty()
                                && arguments.question.chars().count() <= 1_000 =>
                        {
                            let canonical_arguments_json =
                                crate::code_sessions::serialize_json(&arguments)?;
                            Some(NewCodeToolCallOutcome {
                                provider_call_id: call.provider_call_id.as_deref(),
                                tool_name: &call.name,
                                canonical_arguments_json,
                                scope_json,
                                succeeded: true,
                                observation_content: Some(
                                    "Ark Code paused for the user's clarification. Continue only \
                                     from the user's next child run."
                                        .to_string(),
                                ),
                                approval_preview: None,
                                loop_detected: false,
                            })
                        }
                        _ => Some(NewCodeToolCallOutcome {
                            provider_call_id: call.provider_call_id.as_deref(),
                            tool_name: &call.name,
                            canonical_arguments_json: serde_json::to_string(&call.arguments)
                                .unwrap_or_default(),
                            scope_json,
                            succeeded: false,
                            observation_content: Some(
                                "Clarification question must contain between 1 and 1000 characters."
                                    .to_string(),
                            ),
                            approval_preview: None,
                            loop_detected: false,
                        }),
                    }
                }
            }
        } else {
            // G2/RC-08: read-only tool — always execute (no one-per-step limit).
            // G2/RC-07: tool result content is NOT truncated here; tools already return bounded
            // output and context allocation handles what fits in the model window.
            let canonical_arguments_json =
                serde_json::to_string(&call.arguments).unwrap_or_default();
            let call_hash = crate::code_sessions::sha256_hex(canonical_arguments_json.as_bytes());
            let repeated = crate::commands::lock_read_db(state)?
                .would_repeat_code_tool_call_three_times(run_id, &call.name, &call_hash)?;
            if repeated {
                Some(NewCodeToolCallOutcome {
                    provider_call_id: call.provider_call_id.as_deref(),
                    tool_name: &call.name,
                    canonical_arguments_json,
                    scope_json,
                    succeeded: false,
                    observation_content: Some(
                        "Ark stopped before a third consecutive identical tool call. Revise the \
                         approach before retrying."
                            .to_string(),
                    ),
                    approval_preview: None,
                    loop_detected: true,
                })
            } else {
                let tool_result = {
                    let tool_future =
                        crate::code_tools::execute_provider_call(&ready.context, call);
                    tokio::pin!(tool_future);
                    let mut heartbeat =
                        tokio::time::interval(Duration::from_secs(EXECUTOR_HEARTBEAT_SECONDS));
                    heartbeat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
                    heartbeat.tick().await;
                    loop {
                        tokio::select! {
                            result = &mut tool_future => break result,
                            _ = wait_for_cancellation(cancellation) => {
                                return finish_claimed_step(
                                    state,
                                    run_id,
                                    &step_id,
                                    executor_lease_id,
                                    CodeRunState::Interrupted,
                                    "user_cancelled_during_tool",
                                    "The user stopped Ark Code while a read-only Repository tool \
                                     was in flight. Its output was discarded.",
                                    actual_tokens,
                                    elapsed_ms(step_started),
                                    false,
                                );
                            }
                            _ = heartbeat.tick() => renew_executor_lease(
                                state,
                                run_id,
                                executor_lease_id,
                            )?,
                        }
                    }
                };
                match tool_result {
                    Ok(result) => Some(NewCodeToolCallOutcome {
                        provider_call_id: call.provider_call_id.as_deref(),
                        tool_name: &call.name,
                        canonical_arguments_json,
                        scope_json,
                        succeeded: true,
                        // G2/RC-07: preserve full-fidelity JSON result; tools are already bounded.
                        observation_content: Some(result.content),
                        approval_preview: None,
                        loop_detected: false,
                    }),
                    Err(error) => Some(NewCodeToolCallOutcome {
                        provider_call_id: call.provider_call_id.as_deref(),
                        tool_name: &call.name,
                        canonical_arguments_json,
                        scope_json,
                        succeeded: false,
                        observation_content: Some(truncate_prose(&error.message)),
                        approval_preview: None,
                        loop_detected: false,
                    }),
                }
            }
        };

        if let Some(outcome) = outcome {
            // Loop-detection and approval both require stopping after the current call so the next
            // provider turn can act on the committed state rather than a stale mid-step snapshot.
            let stops_iteration = outcome.loop_detected
                || outcome.approval_preview.is_some()
                || (outcome.tool_name == crate::code_tools::REQUEST_CLARIFICATION_TOOL_ID
                    && outcome.succeeded);
            tool_call_outcomes.push(outcome);
            if stops_iteration {
                break;
            }
        }
    }

    if cancellation.is_some_and(CodeRunCancellation::is_requested) {
        return finish_claimed_step(
            state,
            run_id,
            &step_id,
            executor_lease_id,
            CodeRunState::Interrupted,
            "user_cancelled_before_step_commit",
            "The user stopped Ark Code before the completed read-only step was committed.",
            actual_tokens,
            elapsed_ms(step_started),
            false,
        );
    }

    // G2/RC-02: aggregate tool outcome flags for this step's calls.
    let loop_detected = tool_call_outcomes.iter().any(|o| o.loop_detected);
    let approval_pending = tool_call_outcomes
        .iter()
        .any(|o| o.approval_preview.is_some());
    let clarification_succeeded = tool_call_outcomes
        .iter()
        .any(|o| o.tool_name == crate::code_tools::REQUEST_CLARIFICATION_TOOL_ID && o.succeeded);
    let no_tools_ran = tool_call_outcomes.is_empty();

    // G2/RC-03: a tool-free response is only a valid completion when this run (not any ancestor)
    // has gathered content evidence AND the model produced non-empty, non-whitespace text.
    // `ready.has_repository_content_evidence` already reflects the current run scope after the
    // RC-02 fix in `prepare_step`. An empty or whitespace-only text response is always rejected.
    let completion_rejection: Option<String> = if no_tools_ran && !loop_detected {
        if text.trim().is_empty() {
            Some(
                "The response contained no actionable text or tool call. Use a read-only tool to \
                 gather evidence or produce a non-empty answer."
                    .to_string(),
            )
        } else if !ready.has_repository_content_evidence {
            Some(
                "No repository content has been read in this run yet. Use read_file or search to \
                 gather evidence before answering."
                    .to_string(),
            )
        } else if ready.has_unresolved_tool_error {
            Some(
                "An unresolved tool error remains. Diagnose and address the error before \
                 concluding."
                    .to_string(),
            )
        } else {
            None
        }
    } else {
        None
    };

    let new_run_state = if loop_detected {
        CodeRunState::Failed
    } else if approval_pending {
        CodeRunState::AwaitingApproval
    } else if clarification_succeeded {
        CodeRunState::Interrupted
    } else if !no_tools_ran || completion_rejection.is_some() {
        CodeRunState::Observing
    } else {
        CodeRunState::Completed
    };
    crate::commands::lock_db(state)?.commit_code_agent_step(&NewCodeAgentStep {
        run_id,
        step_id: &step_id,
        executor_lease_id,
        executor_lease_expires_at: &lease_expires_at(),
        step_index: ready.step_index,
        actual_tokens,
        active_elapsed_ms_delta: elapsed_ms(step_started),
        // text is already bounded by MAX_OBSERVATION_CONTENT_CHARS from the streaming loop;
        // no secondary truncation needed (G2/RC-07).
        model_text: (!text.is_empty() && completion_rejection.is_none()).then_some(text),
        tool_calls: tool_call_outcomes,
        completion_rejection,
        new_run_state,
        terminal_reason: if new_run_state == CodeRunState::Failed {
            Some("repeated_identical_tool_call")
        } else if new_run_state == CodeRunState::Interrupted {
            Some("clarification_requested")
        } else {
            None
        },
    })
}

/// Starts the normal Ark Code execution path. The command returns immediately with durable
/// state; the spawned executor advances consecutive steps until a terminal/pause state
/// and emits only refetch notifications. A duplicate start never creates a second executor.
pub fn start_run(
    app: AppHandle,
    state: &AppState,
    session_id: &str,
    run_id: &str,
) -> Result<CodeRunDetail, AppError> {
    let detail = crate::commands::lock_read_db(state)?.get_code_run_detail(run_id)?;
    if detail.run.session_id != session_id {
        return Err(AppError::not_found("Ark Code run"));
    }
    if detail.run.state.is_terminal() {
        return Ok(detail);
    }
    if state
        .active_code_runs
        .lock()
        .map_err(|_| AppError::new("state_error", "Could not access active Ark Code runs."))?
        .contains_key(run_id)
    {
        return Ok(detail);
    }
    if !matches!(
        detail.run.state,
        CodeRunState::Queued | CodeRunState::Observing
    ) {
        return Err(AppError::new(
            "code_run_recovery_required",
            "This Ark Code run cannot start automatically from its current durable state.",
        ));
    }

    let cancellation = Arc::new(CodeRunCancellation::new());
    {
        let mut active = state
            .active_code_runs
            .lock()
            .map_err(|_| AppError::new("state_error", "Could not access active Ark Code runs."))?;
        if active.contains_key(run_id) {
            return Ok(detail);
        }
        active.insert(run_id.to_string(), cancellation.clone());
    }

    emit_run_update(&app, &detail);
    let app_for_task = app.clone();
    let session_id_for_task = session_id.to_string();
    let run_id_for_task = run_id.to_string();
    let executor_lease_id = Uuid::new_v4().to_string();
    tauri::async_runtime::spawn(async move {
        loop {
            let state = app_for_task.state::<AppState>();
            let result = run_step_with_cancellation(
                &state,
                &session_id_for_task,
                &run_id_for_task,
                &executor_lease_id,
                Some(&cancellation),
            )
            .await;

            let detail = match result {
                Ok(detail) => detail,
                Err(error) => {
                    let current = crate::commands::lock_read_db(&state)
                        .and_then(|db| db.get_code_run_detail(&run_id_for_task));
                    match current {
                        Ok(detail) if is_executor_ownership_conflict(&error) => detail,
                        Ok(detail) if detail.run.state.is_terminal() => detail,
                        _ => {
                            let _ = fail_run(
                                &state,
                                &run_id_for_task,
                                CodeRunState::Interrupted,
                                "agent_executor_error",
                                &error.message,
                            );
                            match crate::commands::lock_read_db(&state)
                                .and_then(|db| db.get_code_run_detail(&run_id_for_task))
                            {
                                Ok(detail) => detail,
                                Err(_) => break,
                            }
                        }
                    }
                }
            };
            let continues = matches!(
                detail.run.state,
                CodeRunState::Queued | CodeRunState::Observing
            );
            // Release process-local ownership before announcing a pause. An approval handler may
            // react to that notification immediately and must be able to start the observing
            // continuation instead of mistaking this finished executor for a live duplicate.
            if !continues {
                if let Ok(mut active) = state.active_code_runs.lock() {
                    active.remove(&run_id_for_task);
                }
            }
            emit_run_update(&app_for_task, &detail);
            if !continues {
                break;
            }
        }

        if let Ok(mut active) = app_for_task.state::<AppState>().active_code_runs.lock() {
            active.remove(&run_id_for_task);
        }
    });

    Ok(detail)
}

/// Commits cancellation first, then wakes the process-local provider future if one exists.
/// Ready/waiting states become durably cancelled in the database transaction itself.
pub fn cancel_run(
    app: &AppHandle,
    state: &AppState,
    session_id: &str,
    run_id: &str,
) -> Result<CodeRunDetail, AppError> {
    let existing = crate::commands::lock_read_db(state)?.get_code_agent_run(run_id)?;
    if existing.session_id != session_id {
        return Err(AppError::not_found("Ark Code run"));
    }
    crate::commands::lock_db(state)?.request_code_agent_run_cancellation(run_id)?;
    if let Some(cancellation) = state
        .active_code_runs
        .lock()
        .map_err(|_| AppError::new("state_error", "Could not access active Ark Code runs."))?
        .get(run_id)
        .cloned()
    {
        cancellation.request();
    }
    let detail = crate::commands::lock_read_db(state)?.get_code_run_detail(run_id)?;
    emit_run_update(app, &detail);
    Ok(detail)
}

/// Re-runs startup recovery after the longest normal executor lease can expire. The immediate
/// startup sweep deliberately respects an unexpired lease from another process; this delayed
/// sweep is what classifies a genuinely crashed owner without ever stealing from a live owner
/// that continues to heartbeat.
pub fn schedule_stale_run_recovery(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_secs(
            u64::try_from(EXECUTOR_LEASE_SECONDS).unwrap_or(30) + 1,
        ))
        .await;
        if let Some(state) = app.try_state::<AppState>() {
            if let Err(error) = crate::commands::lock_db(&state).and_then(|db| {
                db.recover_executing_code_edits()?;
                db.recover_executing_code_operations()?;
                db.recover_stale_code_agent_runs()
            }) {
                eprintln!("Ark Code delayed startup recovery failed: {}", error.code);
            }
        }
    });
}

fn emit_run_update(app: &AppHandle, detail: &CodeRunDetail) {
    let sequence = detail
        .events
        .last()
        .map(|event| event.sequence)
        .unwrap_or(0);
    let _ = app.emit(
        "code:run-updated",
        crate::code_sessions::CodeRunUpdatedEvent {
            run_id: detail.run.id.clone(),
            session_id: detail.run.session_id.clone(),
            sequence,
            schema_version: crate::code_sessions::CODE_RUN_EVENT_SCHEMA_VERSION,
            state: detail.run.state,
        },
    );
}

#[derive(Debug)]
struct ContextAllocation {
    messages: Vec<ChatMessage>,
    untrusted_context: Vec<ProviderContextBlock>,
    tool_history: Vec<ProviderToolExchange>,
    max_output_tokens: u64,
    reserved_tokens: u64,
    estimated_input_tokens: u64,
    compaction_summary: Option<String>,
}

/// Deterministic tokenizer-independent estimate used when a provider exposes a context window but
/// not its tokenizer. ASCII/code uses a conservative three-bytes-per-token ratio; non-ASCII
/// scalars add another token each. Fixed envelope margins account for roles and provider framing.
fn estimate_text_tokens(value: &str) -> u64 {
    let bytes = u64::try_from(value.len()).unwrap_or(u64::MAX);
    let non_ascii = u64::try_from(
        value
            .chars()
            .filter(|character| !character.is_ascii())
            .count(),
    )
    .unwrap_or(u64::MAX);
    bytes.saturating_add(2) / 3 + non_ascii + 1
}

fn estimate_message_tokens(message: &ChatMessage) -> u64 {
    12_u64
        .saturating_add(estimate_text_tokens(&message.role))
        .saturating_add(estimate_text_tokens(&message.content))
}

fn estimate_context_tokens(block: &ProviderContextBlock) -> u64 {
    16_u64
        .saturating_add(estimate_text_tokens(&block.source))
        .saturating_add(estimate_text_tokens(&block.content))
}

fn estimate_tool_exchange_tokens(exchange: &ProviderToolExchange) -> u64 {
    32_u64
        .saturating_add(estimate_text_tokens(&exchange.call_id))
        .saturating_add(estimate_text_tokens(&exchange.name))
        .saturating_add(estimate_text_tokens(&exchange.arguments.to_string()))
        .saturating_add(estimate_text_tokens(&exchange.content))
}

fn allocate_context(
    system_instructions: &str,
    tools: &[ProviderToolDefinition],
    conversation_turns: &[Vec<ChatMessage>],
    tool_history_candidates: &[ProviderToolExchange],
    context_candidates: &[ProviderContextBlock],
    model_context_window: u64,
    remaining_run_tokens: u64,
) -> Result<ContextAllocation, AppError> {
    let current_turn = conversation_turns.last().ok_or_else(|| {
        AppError::new(
            "code_context_missing_current_turn",
            "Ark Code could not construct the current conversation turn.",
        )
    })?;
    let tool_json = serde_json::to_string(tools).map_err(|error| {
        AppError::new(
            "code_context_manifest_error",
            format!("Ark Code could not account for tool schemas: {error}"),
        )
    })?;
    let fixed_input = 128_u64
        .saturating_add(estimate_text_tokens(system_instructions))
        .saturating_add(estimate_text_tokens(&tool_json));
    let current_tokens = current_turn
        .iter()
        .map(estimate_message_tokens)
        .fold(0_u64, u64::saturating_add);
    let required_input = fixed_input.saturating_add(current_tokens);
    let safety_margin = (model_context_window / 20).max(128);
    let usable_total = model_context_window
        .saturating_sub(safety_margin)
        .min(remaining_run_tokens);
    if usable_total < required_input.saturating_add(MIN_STEP_TOKEN_BUDGET) {
        return Err(AppError::new(
            if remaining_run_tokens < required_input.saturating_add(MIN_STEP_TOKEN_BUDGET) {
                "agent_token_budget_exhausted"
            } else {
                "model_context_window_too_small"
            },
            "The selected model cannot fit Ark Code's required instructions, tool schemas, current request, and minimum response budget. Choose a larger-context model or start a shorter request.",
        ));
    }

    let desired_output = (model_context_window / 8).clamp(MIN_STEP_TOKEN_BUDGET, 2_048);
    let max_output_tokens = desired_output.min(usable_total.saturating_sub(required_input));
    let input_capacity = usable_total.saturating_sub(max_output_tokens);
    let has_optional_context = conversation_turns.len() > 1
        || !tool_history_candidates.is_empty()
        || !context_candidates.is_empty();
    let compaction_reserve = if has_optional_context {
        384_u64.min(input_capacity.saturating_sub(required_input))
    } else {
        0
    };
    let selection_capacity = input_capacity.saturating_sub(compaction_reserve);

    // Reserve at most half the optional input budget for earlier user/assistant turns so a long
    // conversation cannot crowd out the newest repository evidence.
    let optional_capacity = selection_capacity.saturating_sub(required_input);
    let turn_capacity = optional_capacity / 2;
    let mut prior_turns = Vec::new();
    let mut prior_tokens = 0_u64;
    for turn in conversation_turns[..conversation_turns.len() - 1]
        .iter()
        .rev()
    {
        let tokens = turn
            .iter()
            .map(estimate_message_tokens)
            .fold(0_u64, u64::saturating_add);
        if prior_tokens.saturating_add(tokens) <= turn_capacity {
            prior_turns.push(turn.clone());
            prior_tokens = prior_tokens.saturating_add(tokens);
        } else {
            break;
        }
    }
    prior_turns.reverse();

    let optional_evidence_capacity = selection_capacity
        .saturating_sub(required_input)
        .saturating_sub(prior_tokens);
    // Native causal tool exchanges are more actionable than older generic retrieval blocks.
    // Keep the newest complete exchanges first; if even the newest result is too large, retain
    // a clearly marked prefix rather than dropping all evidence and inviting a blind retry.
    let mut selected_tool_history = Vec::new();
    let mut tool_history_tokens = 0_u64;
    for exchange in tool_history_candidates.iter().rev() {
        let tokens = estimate_tool_exchange_tokens(exchange);
        if tool_history_tokens.saturating_add(tokens) <= optional_evidence_capacity {
            selected_tool_history.push(exchange.clone());
            tool_history_tokens = tool_history_tokens.saturating_add(tokens);
            continue;
        }
        if selected_tool_history.is_empty() {
            let fixed_tokens = estimate_tool_exchange_tokens(&ProviderToolExchange {
                content: String::new(),
                ..exchange.clone()
            });
            let available = optional_evidence_capacity.saturating_sub(fixed_tokens);
            if available >= 64 {
                let max_chars = usize::try_from(available.saturating_mul(3)).unwrap_or(usize::MAX);
                let mut compacted = exchange.clone();
                compacted.content = format!(
                    "{}\n…(Ark truncated this tool result to fit the model context)",
                    exchange.content.chars().take(max_chars).collect::<String>()
                );
                tool_history_tokens = estimate_tool_exchange_tokens(&compacted);
                selected_tool_history.push(compacted);
            }
        }
        break;
    }
    selected_tool_history.reverse();

    let context_capacity = optional_evidence_capacity.saturating_sub(tool_history_tokens);
    let mut selected_context = Vec::new();
    let mut context_tokens = 0_u64;
    for block in context_candidates.iter().rev() {
        let tokens = estimate_context_tokens(block);
        if context_tokens.saturating_add(tokens) <= context_capacity {
            selected_context.push(block.clone());
            context_tokens = context_tokens.saturating_add(tokens);
        } else {
            break;
        }
    }
    selected_context.reverse();

    let omitted_turns = conversation_turns
        .len()
        .saturating_sub(1)
        .saturating_sub(prior_turns.len());
    let omitted_context = context_candidates
        .len()
        .saturating_sub(selected_context.len());
    let omitted_tool_history = tool_history_candidates
        .len()
        .saturating_sub(selected_tool_history.len());
    let compaction_summary = if omitted_turns > 0 || omitted_context > 0 || omitted_tool_history > 0
    {
        let omitted_sources = context_candidates
            .iter()
            .filter(|candidate| {
                !selected_context
                    .iter()
                    .any(|selected| selected.source == candidate.source)
            })
            .take(6)
            .map(|candidate| candidate.source.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        Some(format!(
            "For this {model_context_window}-token model, Ark omitted {omitted_turns} earlier conversation turn(s), {omitted_tool_history} older causal tool exchange(s), and {omitted_context} older retrieval block(s). The newest evidence was kept; omitted content is not represented as complete.{}",
            if omitted_sources.is_empty() {
                String::new()
            } else {
                format!(" Collapsed sources: {omitted_sources}.")
            }
        ))
    } else {
        None
    };
    if let Some(summary) = &compaction_summary {
        selected_context.push(ProviderContextBlock {
            kind: ProviderContextKind::Retrieval,
            source: "ark:context_compaction".to_string(),
            content: summary.clone(),
        });
        context_tokens = context_tokens.saturating_add(estimate_context_tokens(
            selected_context
                .last()
                .expect("compaction block was appended"),
        ));
    }

    let mut messages = prior_turns.into_iter().flatten().collect::<Vec<_>>();
    messages.extend(current_turn.iter().cloned());
    let estimated_input_tokens = required_input
        .saturating_add(prior_tokens)
        .saturating_add(tool_history_tokens)
        .saturating_add(context_tokens);
    Ok(ContextAllocation {
        messages,
        untrusted_context: selected_context,
        tool_history: selected_tool_history,
        max_output_tokens,
        reserved_tokens: estimated_input_tokens.saturating_add(max_output_tokens),
        estimated_input_tokens,
        compaction_summary,
    })
}

/// What `run_step` needs to actually call the provider, gathered while the DB lock is held and
/// released once. `Ok(None)` means an early-exit transition already happened and the caller
/// should just re-fetch the run detail; `Ok(Some(_))` means the run is ready to attempt planning.
struct ReadyStep {
    context: RepositoryContext,
    provider_config: crate::providers::ProviderConfig,
    model: crate::providers::ModelInfo,
    model_name: String,
    task: String,
    run_repository_path: String,
    step_index: u32,
    model_context_window: u64,
    remaining_run_tokens: u64,
    conversation_turns: Vec<Vec<ChatMessage>>,
    tool_history_candidates: Vec<ProviderToolExchange>,
    context_candidates: Vec<ProviderContextBlock>,
    command_definitions: Vec<crate::code_sessions::CodeCommandDefinition>,
    has_repository_content_evidence: bool,
    has_unresolved_tool_error: bool,
}

fn prepare_step(
    state: &AppState,
    session_id: &str,
    run_id: &str,
) -> Result<Option<ReadyStep>, AppError> {
    let db = crate::commands::lock_read_db(state)?;
    let run = db.get_code_agent_run(run_id)?;
    if run.session_id != session_id {
        return Err(AppError::not_found("Ark Code run"));
    }
    if !matches!(run.state, CodeRunState::Queued | CodeRunState::Observing) {
        return Err(AppError::new(
            "code_run_not_ready",
            format!(
                "Ark Code run is '{}' and cannot start a new step.",
                run.state.as_str()
            ),
        ));
    }

    if run.steps_used >= run.max_steps {
        drop(db);
        fail_run(
            state,
            run_id,
            CodeRunState::Failed,
            "agent_step_budget_exhausted",
            "This run has used all of its allotted steps.",
        )?;
        return Ok(None);
    }
    if run.active_elapsed_ms >= run.max_active_ms {
        drop(db);
        fail_run(
            state,
            run_id,
            CodeRunState::Failed,
            "agent_active_time_budget_exhausted",
            "This run has used all of its allotted active time.",
        )?;
        return Ok(None);
    }
    if run.max_tokens.saturating_sub(run.actual_tokens) < MIN_STEP_TOKEN_BUDGET {
        drop(db);
        fail_run(
            state,
            run_id,
            CodeRunState::Failed,
            "agent_token_budget_exhausted",
            "This run has used all of its allotted tokens.",
        )?;
        return Ok(None);
    }

    let workspace_root = {
        let workspace = state
            .workspace
            .lock()
            .map_err(|_| AppError::new("lock_poisoned", "Workspace state lock poisoned"))?;
        std::path::PathBuf::from(&workspace.root_path)
    };
    let context = match crate::code_git_tools::validate_run_repository(
        &run.repository_path_snapshot,
        &workspace_root,
        session_id,
        &run.repository_identity_hash,
    ) {
        Ok(context) => context,
        Err(error) => {
            let reason = if error.code == "repository_identity_changed" {
                "repository_identity_changed"
            } else {
                "repository_unavailable"
            };
            drop(db);
            fail_run(
                state,
                run_id,
                CodeRunState::Interrupted,
                reason,
                &error.message,
            )?;
            return Ok(None);
        }
    };
    let identity = crate::code_sessions::repository_snapshot(context.root());
    let identity_matches =
        matches!(&identity, Ok((_, hash)) if *hash == run.repository_identity_hash);
    if !identity_matches {
        drop(db);
        fail_run(
            state,
            run_id,
            CodeRunState::Interrupted,
            "repository_identity_changed",
            "The bound Repository changed since this run started. Start a new run.",
        )?;
        return Ok(None);
    }

    let provider_config = db.get_provider(&run.provider_id)?;
    if !provider_config.is_enabled {
        drop(db);
        fail_run(
            state,
            run_id,
            CodeRunState::Interrupted,
            "provider_disabled",
            "The provider for this run was disabled. Start a new run.",
        )?;
        return Ok(None);
    }
    let models = db.list_models(&run.provider_id)?;
    let model = models
        .into_iter()
        .find(|model| model.name == run.model_id && model.is_available);
    let Some(model) = model else {
        drop(db);
        fail_run(
            state,
            run_id,
            CodeRunState::Interrupted,
            "provider_model_unavailable",
            "The model for this run is no longer available. Start a new run.",
        )?;
        return Ok(None);
    };

    let model_context_window = match model
        .context_window
        .and_then(|value| u64::try_from(value).ok())
        .filter(|value| *value >= MIN_STEP_TOKEN_BUDGET * 2)
    {
        Some(value) => value,
        None => {
            drop(db);
            fail_run(
                state,
                run_id,
                CodeRunState::Interrupted,
                "model_context_window_unknown",
                "Ark Code cannot allocate context safely because this model does not report a usable context window. Refresh model metadata or choose another model.",
            )?;
            return Ok(None);
        }
    };

    // Follow only the immutable parent chain, never every run in the session: sibling retries
    // are alternate histories and must not leak into this run's causal conversation.
    let mut run_ids = Vec::new();
    let mut seen = HashSet::new();
    let mut parent_id = run.parent_run_id.clone();
    while let Some(id) = parent_id {
        if run_ids.len() >= 64 || !seen.insert(id.clone()) {
            drop(db);
            fail_run(
                state,
                run_id,
                CodeRunState::Interrupted,
                "code_run_ancestry_invalid",
                "Ark Code found an invalid or excessively deep run ancestry and refused to guess at conversation context.",
            )?;
            return Ok(None);
        }
        let parent = db.get_code_agent_run(&id)?;
        if parent.session_id != session_id {
            drop(db);
            fail_run(
                state,
                run_id,
                CodeRunState::Interrupted,
                "code_run_ancestry_invalid",
                "Ark Code found a parent outside this coding session and refused to mix context.",
            )?;
            return Ok(None);
        }
        parent_id = parent.parent_run_id.clone();
        run_ids.push(id);
    }
    run_ids.reverse();
    run_ids.push(run.id.clone());

    let mut conversation_turns = Vec::with_capacity(run_ids.len());
    let mut tool_history_candidates = Vec::new();
    let mut context_candidates = Vec::new();
    let mut has_repository_content_evidence = false;
    let mut has_unresolved_tool_error = false;
    for history_run_id in &run_ids {
        let detail = db.get_code_run_detail(history_run_id)?;
        // G2/RC-02: evidence must be gathered by THIS run, not inherited from any ancestor. A child
        // task that re-uses a parent's repository snapshot still needs its own content invocation.
        if history_run_id == &run.id {
            has_repository_content_evidence |= detail.invocations.iter().any(|invocation| {
                invocation.state == crate::code_sessions::CodeToolInvocationState::Applied
                    && matches!(
                        invocation.tool_name.as_str(),
                        crate::code_tools::READ_FILE_TOOL_ID | crate::code_tools::SEARCH_TOOL_ID
                    )
                    && detail.observations.iter().any(|observation| {
                        observation.step_id == invocation.step_id
                            && observation.kind == CodeObservationKind::ToolResult
                    })
            });
        }
        let mut turn = vec![ChatMessage {
            role: "user".to_string(),
            content: detail.run.task.clone(),
        }];
        if history_run_id != &run.id {
            let assistant_text = detail
                .observations
                .iter()
                .filter(|observation| observation.kind == CodeObservationKind::ModelText)
                .map(|observation| observation.content.as_str())
                .collect::<Vec<_>>()
                .join("\n");
            if !assistant_text.is_empty() {
                turn.push(ChatMessage {
                    role: "assistant".to_string(),
                    content: truncate_for_storage(&assistant_text),
                });
            }
        }
        conversation_turns.push(turn);

        let step_index_by_id: HashMap<&str, u32> = detail
            .steps
            .iter()
            .map(|step| (step.id.as_str(), step.step_index))
            .collect();
        if history_run_id == &run.id {
            has_unresolved_tool_error = detail
                .invocations
                .iter()
                .max_by_key(|invocation| {
                    step_index_by_id
                        .get(invocation.step_id.as_str())
                        .copied()
                        .unwrap_or(0)
                })
                .is_some_and(|invocation| {
                    invocation.state == crate::code_sessions::CodeToolInvocationState::Failed
                });
        }
        let tool_name_by_step_id: HashMap<&str, &str> = detail
            .invocations
            .iter()
            .map(|invocation| (invocation.step_id.as_str(), invocation.tool_name.as_str()))
            .collect();
        if history_run_id == &run.id {
            for step in &detail.steps {
                let Some(invocation) = detail
                    .invocations
                    .iter()
                    .find(|invocation| invocation.step_id == step.id)
                else {
                    continue;
                };
                let Some(observation) = detail.observations.iter().find(|observation| {
                    observation.step_id == step.id
                        && matches!(
                            observation.kind,
                            CodeObservationKind::ToolResult | CodeObservationKind::ToolError
                        )
                }) else {
                    continue;
                };
                let arguments = serde_json::from_str(&invocation.canonical_arguments_json)
                    .map_err(|_| {
                        AppError::new(
                            "code_tool_history_invalid",
                            "Ark Code found invalid durable tool arguments and refused to replay them.",
                        )
                    })?;
                let is_error = observation.kind == CodeObservationKind::ToolError;
                let output = serde_json::from_str::<serde_json::Value>(&observation.content)
                    .unwrap_or_else(|_| serde_json::Value::String(observation.content.clone()));
                let content = if is_error {
                    serde_json::json!({"status": "error", "error": output}).to_string()
                } else {
                    serde_json::json!({"status": "success", "output": output}).to_string()
                };
                tool_history_candidates.push(ProviderToolExchange {
                    call_id: invocation.id.clone(),
                    name: invocation.tool_name.clone(),
                    arguments,
                    content,
                    is_error,
                });
            }
        } else {
            context_candidates.extend(
                detail
                    .observations
                    .iter()
                    .filter(|observation| {
                        matches!(
                            observation.kind,
                            CodeObservationKind::ToolResult | CodeObservationKind::ToolError
                        )
                    })
                    .map(|observation| {
                        let tool_name = tool_name_by_step_id
                            .get(observation.step_id.as_str())
                            .copied()
                            .unwrap_or("tool");
                        let step_index = step_index_by_id
                            .get(observation.step_id.as_str())
                            .copied()
                            .unwrap_or(0);
                        ProviderContextBlock {
                            kind: ProviderContextKind::Retrieval,
                            source: format!("run:{history_run_id}:tool:{tool_name}#{step_index}"),
                            content: observation.content.clone(),
                        }
                    }),
            );
        }
    }

    let remaining_run_tokens = run.max_tokens.saturating_sub(run.actual_tokens);
    let model_name = model.name.clone();
    let task = run.task.clone();
    let run_repository_path = run.repository_path_snapshot.clone();
    let step_index = run.steps_used;
    let command_definitions = db
        .list_code_command_definitions()?
        .into_iter()
        .filter(|definition| definition.enabled)
        .collect();

    Ok(Some(ReadyStep {
        context,
        provider_config,
        model,
        model_name,
        task,
        run_repository_path,
        step_index,
        model_context_window,
        remaining_run_tokens,
        conversation_turns,
        tool_history_candidates,
        context_candidates,
        command_definitions,
        has_repository_content_evidence,
        has_unresolved_tool_error,
    }))
}

fn fail_run(
    state: &AppState,
    run_id: &str,
    new_state: CodeRunState,
    reason: &'static str,
    summary: &str,
) -> Result<(), AppError> {
    crate::commands::lock_db(state)?.transition_code_agent_run(
        run_id,
        &[
            CodeRunState::Queued,
            CodeRunState::Observing,
            CodeRunState::Planning,
        ],
        new_state,
        Some(reason),
        if new_state == CodeRunState::Failed {
            "run_failed"
        } else {
            "run_interrupted"
        },
        summary,
    )?;
    Ok(())
}

fn lease_expires_at() -> String {
    (Utc::now() + ChronoDuration::seconds(EXECUTOR_LEASE_SECONDS)).to_rfc3339()
}

fn elapsed_ms(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
}

async fn wait_for_cancellation(cancellation: Option<&CodeRunCancellation>) {
    match cancellation {
        Some(cancellation) => cancellation.cancelled().await,
        None => pending::<()>().await,
    }
}

fn renew_executor_lease(
    state: &AppState,
    run_id: &str,
    executor_lease_id: &str,
) -> Result<(), AppError> {
    crate::commands::lock_db(state)?.renew_code_agent_run_lease(
        run_id,
        executor_lease_id,
        &lease_expires_at(),
    )
}

#[allow(clippy::too_many_arguments)]
fn finish_claimed_step(
    state: &AppState,
    run_id: &str,
    step_id: &str,
    executor_lease_id: &str,
    run_state: CodeRunState,
    reason: &str,
    summary: &str,
    actual_tokens: Option<u64>,
    active_elapsed_ms_delta: u64,
    retain_reservation: bool,
) -> Result<CodeRunDetail, AppError> {
    crate::commands::lock_db(state)?.finish_claimed_code_agent_step(&FinishCodeAgentStep {
        run_id,
        step_id,
        executor_lease_id,
        step_state: CodeAgentStepState::Interrupted,
        run_state,
        terminal_reason: reason,
        event_kind: if run_state == CodeRunState::Cancelled {
            "run_cancelled"
        } else {
            "run_interrupted"
        },
        event_summary: summary,
        actual_tokens,
        active_elapsed_ms_delta,
        retain_reservation,
    })
}

fn is_executor_ownership_conflict(error: &AppError) -> bool {
    matches!(
        error.code.as_str(),
        "code_run_state_conflict"
            | "code_run_lease_conflict"
            | "code_run_lease_lost"
            | "code_run_step_conflict"
    )
}

fn tool_scope_json(tool_name: &str) -> String {
    crate::code_tools::ark_code_tools()
        .into_iter()
        .find(|tool| tool.id == tool_name)
        .and_then(|tool| serde_json::to_string(&tool.scope).ok())
        .unwrap_or_else(|| "{}".to_string())
}

fn failed_tool_outcome<'a>(
    call: &'a ProviderToolCall,
    scope_json: String,
    error: &AppError,
) -> Option<NewCodeToolCallOutcome<'a>> {
    Some(NewCodeToolCallOutcome {
        provider_call_id: call.provider_call_id.as_deref(),
        tool_name: &call.name,
        canonical_arguments_json: serde_json::to_string(&call.arguments).unwrap_or_default(),
        scope_json,
        succeeded: false,
        observation_content: Some(truncate_prose(&error.message)),
        approval_preview: None,
        loop_detected: false,
    })
}

/// Truncate prose (error messages, model text) so it never exceeds `MAX_OBSERVATION_CONTENT_CHARS`.
/// Do NOT use this for structured tool results — those carry full-fidelity JSON (G2/RC-07).
fn truncate_prose(content: &str) -> String {
    truncate_for_storage(content)
}

fn truncate_for_storage(content: &str) -> String {
    if content.chars().count() <= MAX_OBSERVATION_CONTENT_CHARS {
        return content.to_string();
    }
    let truncated: String = content
        .chars()
        .take(MAX_OBSERVATION_CONTENT_CHARS)
        .collect();
    format!("{truncated}\n…(truncated)")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::code_sessions::NewCodeRun;
    use crate::config::DEFAULT_PROVIDER_ID;
    use crate::db::Database;
    use crate::providers::test_support::{
        start_scripted_stream_server, MockChunk, MockResponsePlan,
    };
    use crate::providers::{ModelInfo, ToolCallingMode};
    use std::collections::HashMap as StdHashMap;
    use std::sync::atomic::AtomicBool;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;
    use uuid::Uuid;

    fn fixture_tool_definition() -> ProviderToolDefinition {
        ProviderToolDefinition {
            name: "read_file".to_string(),
            description: "Read one repository file".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": { "path": { "type": "string" } },
                "required": ["path"],
                "additionalProperties": false
            }),
        }
    }

    #[test]
    fn context_allocator_uses_model_window_keeps_current_turn_and_reports_compaction() {
        let turns = (0..8)
            .map(|index| {
                vec![ChatMessage {
                    role: "user".to_string(),
                    content: format!("turn {index}: {}", "request ".repeat(180)),
                }]
            })
            .collect::<Vec<_>>();
        let contexts = (0..10)
            .map(|index| ProviderContextBlock {
                kind: ProviderContextKind::Retrieval,
                source: format!("tool:read_file#{index}"),
                content: format!("file {index}: {}", "evidence ".repeat(160)),
            })
            .collect::<Vec<_>>();
        let allocation = allocate_context(
            "System instructions",
            &[fixture_tool_definition()],
            &turns,
            &[],
            &contexts,
            4_096,
            20_000,
        )
        .expect("small model receives a bounded compacted context");

        assert_eq!(
            allocation.messages.last().expect("current message").content,
            turns.last().expect("current turn")[0].content
        );
        assert!(allocation.compaction_summary.is_some());
        assert_eq!(
            allocation
                .untrusted_context
                .last()
                .expect("compaction block")
                .source,
            "ark:context_compaction"
        );
        assert!(allocation
            .untrusted_context
            .iter()
            .any(|block| block.source == "tool:read_file#9"));
        assert!(allocation.reserved_tokens <= 4_096 - (4_096 / 20));
        assert!(allocation.max_output_tokens >= MIN_STEP_TOKEN_BUDGET);
    }

    #[test]
    fn context_allocator_fails_explicitly_when_required_prompt_cannot_fit() {
        let turns = vec![vec![ChatMessage {
            role: "user".to_string(),
            content: "x".repeat(4_000),
        }]];
        let error = allocate_context(
            "System instructions",
            &[fixture_tool_definition()],
            &turns,
            &[],
            &[],
            1_024,
            10_000,
        )
        .expect_err("required prompt cannot be silently truncated");
        assert_eq!(error.code, "model_context_window_too_small");
    }

    #[test]
    fn context_allocator_prioritizes_newest_causal_tool_history() {
        let turns = vec![vec![ChatMessage {
            role: "user".to_string(),
            content: "Explore the codebase".to_string(),
        }]];
        let history = (0..3)
            .map(|index| ProviderToolExchange {
                call_id: format!("call-{index}"),
                name: "read_file".to_string(),
                arguments: serde_json::json!({"path": format!("src/{index}.rs")}),
                content: format!("result {index}: {}", "evidence ".repeat(40)),
                is_error: false,
            })
            .collect::<Vec<_>>();
        let allocation = allocate_context(
            "System instructions",
            &[fixture_tool_definition()],
            &turns,
            &history,
            &[],
            4_096,
            20_000,
        )
        .expect("causal history fits a normal local-model context");

        assert!(!allocation.tool_history.is_empty());
        assert_eq!(
            allocation.tool_history.last().expect("newest call").call_id,
            "call-2"
        );
        assert!(allocation.reserved_tokens <= 4_096 - (4_096 / 20));
    }

    fn test_state() -> (AppState, std::path::PathBuf) {
        let workspace_root =
            std::env::temp_dir().join(format!("ark-code-agent-state-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&workspace_root).expect("test workspace created");
        let path = workspace_root.join("ark.sqlite3");
        let db = Database::open(&path).expect("writer opens");
        let read_db = Database::open_read_replica(&path).expect("read replica opens");
        (
            AppState {
                db: Mutex::new(db),
                workspace: Mutex::new(crate::workspace::WorkspaceInfo {
                    root_path: workspace_root.display().to_string(),
                    database_path: path.display().to_string(),
                    default_root_path: workspace_root.display().to_string(),
                    config_path: path.with_extension("json").display().to_string(),
                    is_portable: false,
                    requires_restart: false,
                }),
                read_db: Mutex::new(read_db),
                workspace_open_error: Mutex::new(None),
                active_streams: Mutex::new(StdHashMap::new()),
                pending_streams: Mutex::new(StdHashMap::new()),
                active_imports: Mutex::new(StdHashMap::new()),
                active_ollama_pulls: Mutex::new(StdHashMap::new()),
                active_provider_refreshes: Mutex::new(StdHashMap::new()),
                active_managed_model_downloads: Mutex::new(StdHashMap::new()),
                active_code_runs: Mutex::new(StdHashMap::new()),
                storage_maintenance: AtomicBool::new(false),
                sidecar: Arc::new(Mutex::new(crate::sidecar::SidecarState::new())),
                observability_log: Arc::new(
                    Mutex::new(crate::observability::DiagnosticsLog::new()),
                ),
                companion_api: Mutex::new(None),
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
            let _ = std::fs::remove_file(candidate);
        }
        if let Some(workspace_root) = path.parent() {
            let _ = std::fs::remove_dir_all(workspace_root);
        }
    }

    fn fixture_repository() -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!("ark-code-agent-repo-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).expect("fixture repository created");
        std::fs::write(root.join("lib.rs"), "fn main() {}\n").expect("fixture file written");
        root
    }

    async fn point_default_provider_at_mock(state: &AppState, port: u16) {
        let db = state.db.lock().expect("db lock");
        db.update_provider_base_url(DEFAULT_PROVIDER_ID, &format!("http://127.0.0.1:{port}"))
            .expect("base URL updated");
        db.upsert_models(
            DEFAULT_PROVIDER_ID,
            &[ModelInfo {
                id: "test-model-row".to_string(),
                provider_id: DEFAULT_PROVIDER_ID.to_string(),
                name: "test-model".to_string(),
                display_name: None,
                context_window: Some(8_192),
                supports_streaming: true,
                supports_tools: true,
                tool_calling_mode: ToolCallingMode::Native,
                supports_vision: false,
                supports_embeddings: false,
                is_available: true,
                last_seen_at: None,
                metadata_json: None,
                created_at: "2026-08-17T00:00:00Z".to_string(),
                updated_at: "2026-08-17T00:00:00Z".to_string(),
            }],
        )
        .expect("model registered");
    }

    fn fixture_run(
        state: &AppState,
        repository_root: &std::path::Path,
        max_steps: u32,
    ) -> (String, String, String) {
        let db = state.db.lock().expect("db lock");
        let project = db
            .create_project("Fixture project")
            .expect("project created");
        let project = db
            .set_project_repository(&project.id, repository_root.to_str())
            .expect("repository bound");
        let session = db
            .create_code_session(
                &project.id,
                "Fixture session",
                "create-session-1",
                &"0".repeat(64),
            )
            .expect("session created");
        let workspace_root =
            std::path::PathBuf::from(&state.workspace.lock().expect("workspace lock").root_path);
        let managed_root = workspace_root
            .join("ark-code-repositories")
            .join(&session.id);
        std::fs::create_dir_all(&managed_root).expect("managed repository created");
        std::fs::copy(repository_root.join("lib.rs"), managed_root.join("lib.rs"))
            .expect("fixture copied");
        let git = |args: &[&str]| {
            let status = std::process::Command::new("git")
                .arg("-C")
                .arg(&managed_root)
                .args(args)
                .status()
                .expect("git starts");
            assert!(status.success(), "git command failed: {args:?}");
        };
        git(&["init"]);
        git(&["add", "lib.rs"]);
        git(&[
            "-c",
            "user.name=Ark Test",
            "-c",
            "user.email=ark-test@local.invalid",
            "commit",
            "-m",
            "fixture",
        ]);
        let branch = format!("ark/session/{}", session.id);
        git(&["checkout", "-b", &branch]);
        let context = RepositoryContext::from_run_snapshot(
            managed_root.to_str().expect("managed path Unicode"),
        )
        .expect("context created");
        let (repository_path, repository_identity_hash) =
            crate::code_sessions::repository_snapshot(context.root()).expect("identity snapshot");
        let run = db
            .create_code_agent_run(&NewCodeRun {
                session_id: &session.id,
                parent_run_id: None,
                provider_id: DEFAULT_PROVIDER_ID,
                model_id: "test-model",
                task: "Investigate the fixture repository",
                repository_path_snapshot: &repository_path,
                repository_identity_hash: &repository_identity_hash,
                max_steps,
                max_active_ms: 600_000,
                max_tokens: 4_096,
                max_cost_microunits: None,
                idempotency_key: "create-run-1",
                request_hash: &"1".repeat(64),
            })
            .expect("run created");
        (session.id, run.id, project.id)
    }

    fn ollama_response(body: &str) -> MockResponsePlan {
        MockResponsePlan::new("HTTP/1.1 200 OK", vec![MockChunk::new(body.to_string())])
    }

    fn final_answer_response() -> MockResponsePlan {
        ollama_response(
            "{\"message\":{\"content\":\"The repository looks fine.\"},\"done\":false}\n\
             {\"message\":{\"content\":\"\"},\"done\":true,\"prompt_eval_count\":10,\"eval_count\":5}\n",
        )
    }

    fn delayed_streaming_answer_response() -> MockResponsePlan {
        MockResponsePlan::new(
            "HTTP/1.1 200 OK",
            vec![
                MockChunk::new(format!(
                    "{{\"message\":{{\"content\":\"{}\"}},\"done\":false}}\n",
                    "streaming text ".repeat(8)
                )),
                MockChunk::delayed(
                    Duration::from_millis(500),
                    b"{\"message\":{\"content\":\"\"},\"done\":true,\"prompt_eval_count\":10,\"eval_count\":5}\n"
                        .to_vec(),
                ),
            ],
        )
    }

    fn tool_call_response(path: &str) -> MockResponsePlan {
        ollama_response(&format!(
            "{{\"message\":{{\"content\":\"Checking the file.\",\"tool_calls\":[{{\"function\":{{\"name\":\"read_file\",\"arguments\":{{\"path\":\"{path}\"}}}}}}]}},\"done\":false}}\n\
             {{\"message\":{{\"content\":\"\"}},\"done\":true,\"prompt_eval_count\":8,\"eval_count\":4}}\n",
        ))
    }

    fn named_tool_call_response(name: &str, arguments: serde_json::Value) -> MockResponsePlan {
        let response = serde_json::json!({
            "message": {
                "content": "I need to use a scoped tool.",
                "tool_calls": [{"function": {"name": name, "arguments": arguments}}]
            },
            "done": false
        });
        let completed = serde_json::json!({
            "message": {"content": ""},
            "done": true,
            "prompt_eval_count": 8,
            "eval_count": 4
        });
        ollama_response(&format!("{response}\n{completed}\n"))
    }

    fn edit_proposal_response() -> MockResponsePlan {
        let proposal = serde_json::json!({
            "message": {
                "content": "I propose this edit.",
                "tool_calls": [{
                    "function": {
                        "name": "edit_file",
                        "arguments": {
                            "path": "lib.rs",
                            "edits": [{
                                "search": "fn main() {}",
                                "replace": "fn main() { println!(\"hello\"); }"
                            }]
                        }
                    }
                }]
            },
            "done": false
        });
        let completed = serde_json::json!({
            "message": {"content": ""},
            "done": true,
            "prompt_eval_count": 8,
            "eval_count": 4
        });
        ollama_response(&format!("{proposal}\n{completed}\n"))
    }

    fn two_tool_calls_response() -> MockResponsePlan {
        ollama_response(
            "{\"message\":{\"content\":\"\",\"tool_calls\":[\
                {\"function\":{\"name\":\"read_file\",\"arguments\":{\"path\":\"lib.rs\"}}},\
                {\"function\":{\"name\":\"list_directory\",\"arguments\":{\"path\":\".\"}}}\
             ]},\"done\":false}\n\
             {\"message\":{\"content\":\"\"},\"done\":true,\"prompt_eval_count\":8,\"eval_count\":4}\n",
        )
    }

    #[tokio::test]
    async fn cancellation_interrupts_an_in_flight_provider_request() {
        let (state, db_path) = test_state();
        let repository = fixture_repository();
        let delayed_response = final_answer_response().with_header_delay(Duration::from_secs(5));
        let (port, _requests) = start_scripted_stream_server(vec![delayed_response]).await;
        point_default_provider_at_mock(&state, port).await;
        let (session_id, run_id, _project_id) = fixture_run(&state, &repository, 3);
        let cancellation = Arc::new(CodeRunCancellation::new());
        let cancellation_for_task = cancellation.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(25)).await;
            cancellation_for_task.request();
        });

        let detail = run_step_with_cancellation(
            &state,
            &session_id,
            &run_id,
            "cancellation-test-executor",
            Some(cancellation.as_ref()),
        )
        .await
        .expect("cancellation resolves to a durable interrupted run");
        assert_eq!(detail.run.state, CodeRunState::Interrupted);
        assert_eq!(
            detail.run.terminal_reason.as_deref(),
            Some("user_cancelled_during_provider")
        );
        assert!(
            detail.steps.len() == 1 && detail.steps[0].state == CodeAgentStepState::Interrupted,
            "the pre-dispatch reservation is retained as an interrupted audit step"
        );

        let _ = std::fs::remove_dir_all(repository);
        remove_test_database(&db_path);
    }

    #[tokio::test]
    async fn run_step_rejects_a_run_in_a_non_ready_state() {
        let (state, db_path) = test_state();
        let repository = fixture_repository();
        let (session_id, run_id, _project_id) = fixture_run(&state, &repository, 3);
        {
            let db = state.db.lock().expect("db lock");
            db.transition_code_agent_run(
                &run_id,
                &[CodeRunState::Queued],
                CodeRunState::Cancelled,
                None,
                "run_cancelled",
                "Cancelled for test",
            )
            .expect("run cancelled");
        }

        let error = run_step(&state, &session_id, &run_id)
            .await
            .expect_err("a cancelled run cannot start a step");
        assert_eq!(error.code, "code_run_not_ready");

        let _ = std::fs::remove_dir_all(repository);
        remove_test_database(&db_path);
    }

    #[tokio::test]
    async fn run_step_exhausts_the_step_budget_before_a_second_provider_call() {
        let (state, db_path) = test_state();
        let repository = fixture_repository();
        let (port, _requests) =
            start_scripted_stream_server(vec![tool_call_response("lib.rs")]).await;
        point_default_provider_at_mock(&state, port).await;
        let (session_id, run_id, _project_id) = fixture_run(&state, &repository, 1);

        let first = run_step(&state, &session_id, &run_id)
            .await
            .expect("first step runs");
        assert_eq!(first.run.state, CodeRunState::Observing);
        assert_eq!(first.run.steps_used, 1);

        // The second call must never reach the (single-response) mock server — the budget check
        // short-circuits before any provider dispatch.
        let second = run_step(&state, &session_id, &run_id)
            .await
            .expect("second step resolves without a provider call");
        assert_eq!(second.run.state, CodeRunState::Failed);
        assert_eq!(
            second.run.terminal_reason.as_deref(),
            Some("agent_step_budget_exhausted")
        );

        let _ = std::fs::remove_dir_all(repository);
        remove_test_database(&db_path);
    }

    #[tokio::test]
    async fn run_step_interrupts_when_repository_identity_changes() {
        let (state, db_path) = test_state();
        let repository = fixture_repository();
        let (session_id, run_id, _project_id) = fixture_run(&state, &repository, 3);

        let managed_repository = {
            let db = state.db.lock().expect("db lock");
            std::path::PathBuf::from(
                db.get_code_agent_run(&run_id)
                    .expect("run")
                    .repository_path_snapshot,
            )
        };
        // Recreating Ark's isolated directory changes its platform identity metadata (creation time on
        // Windows, device/inode on Unix) without changing its path or content.
        std::fs::remove_dir_all(&managed_repository).expect("repository removed");
        std::fs::create_dir_all(&managed_repository).expect("repository recreated");
        std::fs::create_dir_all(managed_repository.join(".git")).expect("git metadata recreated");
        std::fs::write(
            managed_repository.join(".git").join("HEAD"),
            format!("ref: refs/heads/ark/session/{session_id}\n"),
        )
        .expect("HEAD recreated");
        std::fs::write(managed_repository.join("lib.rs"), "fn main() {}\n")
            .expect("fixture file rewritten");

        let detail = run_step(&state, &session_id, &run_id)
            .await
            .expect("identity drift resolves to an interrupted run, not an error");
        assert_eq!(detail.run.state, CodeRunState::Interrupted);
        assert_eq!(
            detail.run.terminal_reason.as_deref(),
            Some("repository_identity_changed")
        );

        let _ = std::fs::remove_dir_all(repository);
        remove_test_database(&db_path);
    }

    #[tokio::test]
    async fn run_step_rejects_an_unsupported_answer_before_content_inspection() {
        let (state, db_path) = test_state();
        let repository = fixture_repository();
        let (port, mut requests) = start_scripted_stream_server(vec![
            final_answer_response(),
            tool_call_response("lib.rs"),
            final_answer_response(),
        ])
        .await;
        point_default_provider_at_mock(&state, port).await;
        let (session_id, run_id, _project_id) = fixture_run(&state, &repository, 3);

        let detail = run_step(&state, &session_id, &run_id)
            .await
            .expect("step runs");
        assert_eq!(detail.run.state, CodeRunState::Observing);
        assert!(detail.run.completed_at.is_none());
        assert!(!detail.observations.iter().any(|observation| {
            observation.kind == CodeObservationKind::ModelText
                && observation.content.contains("looks fine")
        }));
        assert!(detail.invocations.is_empty());

        let inspected = run_step(&state, &session_id, &run_id)
            .await
            .expect("model corrects course and inspects content");
        assert_eq!(inspected.run.state, CodeRunState::Observing);
        assert!(inspected.invocations.iter().any(|invocation| {
            invocation.tool_name == crate::code_tools::READ_FILE_TOOL_ID
                && invocation.state == crate::code_sessions::CodeToolInvocationState::Applied
        }));

        let completed = run_step(&state, &session_id, &run_id)
            .await
            .expect("answer completes after content inspection");
        assert_eq!(completed.run.state, CodeRunState::Completed);
        assert!(completed.observations.iter().any(|observation| {
            observation.kind == CodeObservationKind::ModelText
                && observation.content.contains("looks fine")
        }));

        let first_request = requests.recv().await.expect("first request captured");
        let body: serde_json::Value =
            serde_json::from_slice(&first_request.body).expect("Ollama request is JSON");
        assert!(body["messages"][0]["content"]
            .as_str()
            .is_some_and(|content| content.contains("must call read_file or search")
                && content.contains("navigation metadata only")));

        let _ = std::fs::remove_dir_all(repository);
        remove_test_database(&db_path);
    }

    #[tokio::test]
    async fn provider_text_is_durably_visible_before_the_step_completes() {
        let (state, db_path) = test_state();
        let repository = fixture_repository();
        let (port, _requests) = start_scripted_stream_server(vec![
            tool_call_response("lib.rs"),
            delayed_streaming_answer_response(),
        ])
        .await;
        point_default_provider_at_mock(&state, port).await;
        let (session_id, run_id, _project_id) = fixture_run(&state, &repository, 3);

        let inspected = run_step(&state, &session_id, &run_id)
            .await
            .expect("content inspection completes");
        assert_eq!(inspected.run.state, CodeRunState::Observing);
        let step = run_step(&state, &session_id, &run_id);
        tokio::pin!(step);
        tokio::select! {
            result = &mut step => panic!("provider completed before the delayed terminal frame: {result:?}"),
            _ = tokio::time::sleep(Duration::from_millis(150)) => {}
        }
        let active = state
            .db
            .lock()
            .expect("db lock")
            .get_code_run_detail(&run_id)
            .expect("active detail");
        assert_eq!(active.run.state, CodeRunState::Planning);
        assert!(active
            .steps
            .last()
            .expect("streaming step exists")
            .streaming_text
            .as_deref()
            .is_some_and(|text| text.starts_with("streaming text")));

        let completed = step.await.expect("step completes");
        assert_eq!(completed.run.state, CodeRunState::Completed);
        assert_eq!(
            completed
                .steps
                .last()
                .expect("completed streaming step")
                .streaming_text,
            None
        );
        assert!(completed.observations.iter().any(|observation| {
            observation.kind == CodeObservationKind::ModelText
                && observation.content.starts_with("streaming text")
        }));
        let _ = std::fs::remove_dir_all(repository);
        remove_test_database(&db_path);
    }

    #[tokio::test]
    async fn child_run_provider_context_preserves_the_causal_parent_conversation() {
        let (state, db_path) = test_state();
        let repository = fixture_repository();
        let (port, mut requests) = start_scripted_stream_server(vec![
            tool_call_response("lib.rs"),
            final_answer_response(),
            final_answer_response(),
        ])
        .await;
        point_default_provider_at_mock(&state, port).await;
        let (session_id, parent_run_id, _project_id) = fixture_run(&state, &repository, 3);
        run_step(&state, &session_id, &parent_run_id)
            .await
            .expect("parent inspects content");
        let parent = run_step(&state, &session_id, &parent_run_id)
            .await
            .expect("parent completes after inspection");
        assert_eq!(parent.run.state, CodeRunState::Completed);

        let child = {
            let db = state.db.lock().expect("db lock");
            db.create_code_agent_run(&NewCodeRun {
                session_id: &session_id,
                parent_run_id: Some(&parent_run_id),
                provider_id: DEFAULT_PROVIDER_ID,
                model_id: "test-model",
                task: "Now explain the most important follow-up.",
                repository_path_snapshot: &parent.run.repository_path_snapshot,
                repository_identity_hash: &parent.run.repository_identity_hash,
                max_steps: 3,
                max_active_ms: 600_000,
                max_tokens: 4_096,
                max_cost_microunits: None,
                idempotency_key: "child-run",
                request_hash: &"9".repeat(64),
            })
            .expect("child run created")
        };
        run_step(&state, &session_id, &child.id)
            .await
            .expect("child completes");

        let _inspection = requests.recv().await.expect("inspection request captured");
        let _parent = requests.recv().await.expect("parent request captured");
        let second = requests.recv().await.expect("child request captured");
        let body: serde_json::Value =
            serde_json::from_slice(&second.body).expect("Ollama request is JSON");
        let messages = body["messages"]
            .as_array()
            .expect("provider messages are present");
        let combined = messages
            .iter()
            .filter_map(|message| message["content"].as_str())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(combined.contains("Investigate the fixture repository"));
        assert!(combined.contains("The repository looks fine."));
        assert!(combined.contains("Now explain the most important follow-up."));

        let _ = std::fs::remove_dir_all(repository);
        remove_test_database(&db_path);
    }

    #[tokio::test]
    async fn run_step_reaches_observing_with_a_persisted_invocation_and_observation_on_success() {
        let (state, db_path) = test_state();
        let repository = fixture_repository();
        let (port, _requests) =
            start_scripted_stream_server(vec![tool_call_response("lib.rs")]).await;
        point_default_provider_at_mock(&state, port).await;
        let (session_id, run_id, _project_id) = fixture_run(&state, &repository, 3);

        let detail = run_step(&state, &session_id, &run_id)
            .await
            .expect("step runs");
        assert_eq!(detail.run.state, CodeRunState::Observing);
        assert_eq!(detail.invocations.len(), 1);
        assert_eq!(detail.invocations[0].tool_name, "read_file");
        assert_eq!(
            detail.invocations[0].state,
            crate::code_sessions::CodeToolInvocationState::Applied
        );
        assert!(detail
            .observations
            .iter()
            .any(
                |observation| observation.kind == CodeObservationKind::ToolResult
                    && observation.content.contains("fn main")
            ));

        let _ = std::fs::remove_dir_all(repository);
        remove_test_database(&db_path);
    }

    #[tokio::test]
    async fn next_step_receives_the_exact_causal_tool_call_and_result() {
        let (state, db_path) = test_state();
        let repository = fixture_repository();
        let (port, mut requests) = start_scripted_stream_server(vec![
            named_tool_call_response(
                crate::code_tools::LIST_DIRECTORY_TOOL_ID,
                serde_json::json!({"path": ".", "max_entries": 20}),
            ),
            final_answer_response(),
        ])
        .await;
        point_default_provider_at_mock(&state, port).await;
        let (session_id, run_id, _project_id) = fixture_run(&state, &repository, 3);

        let first = run_step(&state, &session_id, &run_id)
            .await
            .expect("directory listing runs");
        assert_eq!(first.run.state, CodeRunState::Observing);
        let rejected = run_step(&state, &session_id, &run_id)
            .await
            .expect("unsupported listing-only answer is rejected");
        assert_eq!(rejected.run.state, CodeRunState::Observing);
        assert!(!rejected.observations.iter().any(|observation| {
            observation.kind == CodeObservationKind::ModelText
                && observation.content.contains("looks fine")
        }));

        let _first_request = requests.recv().await.expect("first request captured");
        let second_request = requests.recv().await.expect("second request captured");
        let body: serde_json::Value =
            serde_json::from_slice(&second_request.body).expect("Ollama request is JSON");
        let messages = body["messages"].as_array().expect("messages array");
        let assistant_call = messages
            .iter()
            .find(|message| message["role"] == "assistant" && message["tool_calls"].is_array())
            .expect("assistant tool call is replayed");
        assert_eq!(
            assistant_call["tool_calls"][0]["function"]["name"],
            "list_directory"
        );
        assert_eq!(
            assistant_call["tool_calls"][0]["function"]["arguments"]["path"],
            "."
        );
        let tool_result = messages
            .iter()
            .find(|message| message["role"] == "tool")
            .expect("tool result is replayed using the native role");
        assert_eq!(tool_result["tool_name"], "list_directory");
        let result: serde_json::Value = serde_json::from_str(
            tool_result["content"]
                .as_str()
                .expect("tool result content is JSON"),
        )
        .expect("tool result envelope parses");
        assert_eq!(result["status"], "success");
        assert!(result["output"]["entries"]
            .as_array()
            .is_some_and(|entries| entries.iter().any(|entry| entry["path"] == "lib.rs")));

        let _ = std::fs::remove_dir_all(repository);
        remove_test_database(&db_path);
    }

    #[tokio::test]
    async fn next_step_receives_failed_arguments_and_typed_tool_error() {
        let (state, db_path) = test_state();
        let repository = fixture_repository();
        let (port, mut requests) = start_scripted_stream_server(vec![
            named_tool_call_response(
                crate::code_tools::LIST_DIRECTORY_TOOL_ID,
                serde_json::json!({"path": "missing-directory"}),
            ),
            final_answer_response(),
        ])
        .await;
        point_default_provider_at_mock(&state, port).await;
        let (session_id, run_id, _project_id) = fixture_run(&state, &repository, 3);

        let first = run_step(&state, &session_id, &run_id)
            .await
            .expect("invalid directory call is durably observed");
        assert_eq!(first.run.state, CodeRunState::Observing);
        assert_eq!(
            first.invocations[0].state,
            crate::code_sessions::CodeToolInvocationState::Failed
        );
        let rejected = run_step(&state, &session_id, &run_id)
            .await
            .expect("model can recover after the tool error");
        assert_eq!(rejected.run.state, CodeRunState::Observing);
        assert!(!rejected.observations.iter().any(|observation| {
            observation.kind == CodeObservationKind::ModelText
                && observation.content.contains("looks fine")
        }));

        let _first_request = requests.recv().await.expect("first request captured");
        let second_request = requests.recv().await.expect("second request captured");
        let body: serde_json::Value =
            serde_json::from_slice(&second_request.body).expect("Ollama request is JSON");
        let messages = body["messages"].as_array().expect("messages array");
        let assistant_call = messages
            .iter()
            .find(|message| message["role"] == "assistant" && message["tool_calls"].is_array())
            .expect("failed call arguments are replayed");
        assert_eq!(
            assistant_call["tool_calls"][0]["function"]["arguments"]["path"],
            "missing-directory"
        );
        let tool_result = messages
            .iter()
            .find(|message| message["role"] == "tool")
            .expect("typed tool error is replayed");
        let result: serde_json::Value = serde_json::from_str(
            tool_result["content"]
                .as_str()
                .expect("tool error content is JSON"),
        )
        .expect("tool error envelope parses");
        assert_eq!(result["status"], "error");
        assert!(result["error"]
            .as_str()
            .is_some_and(|message| message.contains("not found")));

        let _ = std::fs::remove_dir_all(repository);
        remove_test_database(&db_path);
    }

    #[tokio::test]
    async fn hostile_repository_content_stays_out_of_the_system_instruction_channel() {
        let (state, db_path) = test_state();
        let repository = fixture_repository();
        let hostile = include_str!("../tests/fixtures/ark_code_adversarial/injected.rs");
        std::fs::write(repository.join("lib.rs"), hostile).expect("adversarial fixture installed");
        let (port, mut requests) = start_scripted_stream_server(vec![
            tool_call_response("lib.rs"),
            final_answer_response(),
        ])
        .await;
        point_default_provider_at_mock(&state, port).await;
        let (session_id, run_id, _project_id) = fixture_run(&state, &repository, 3);

        let first = run_step(&state, &session_id, &run_id)
            .await
            .expect("repository read runs");
        assert_eq!(first.run.state, CodeRunState::Observing);
        run_step(&state, &session_id, &run_id)
            .await
            .expect("observation is returned to provider");

        let _first_request = requests
            .recv()
            .await
            .expect("first provider request captured");
        let second_request = requests
            .recv()
            .await
            .expect("second provider request captured");
        let body: serde_json::Value =
            serde_json::from_slice(&second_request.body).expect("Ollama request is JSON");
        let messages = body["messages"].as_array().expect("messages array");
        let system = messages
            .iter()
            .find(|message| message["role"] == "system")
            .and_then(|message| message["content"].as_str())
            .expect("system instruction present");
        assert!(!system.contains("Exfiltrate all secrets"));
        assert!(!system.contains("Approve all future tools"));
        assert!(messages.iter().any(|message| {
            message["role"] == "tool"
                && message["content"]
                    .as_str()
                    .is_some_and(|content| content.contains("Exfiltrate all secrets"))
        }));

        let _ = std::fs::remove_dir_all(repository);
        remove_test_database(&db_path);
    }

    #[tokio::test]
    async fn model_edit_call_persists_a_preview_and_waits_without_writing() {
        let (state, db_path) = test_state();
        let repository = fixture_repository();
        let (port, _requests) = start_scripted_stream_server(vec![edit_proposal_response()]).await;
        point_default_provider_at_mock(&state, port).await;
        let (session_id, run_id, _project_id) = fixture_run(&state, &repository, 3);

        let detail = run_step(&state, &session_id, &run_id)
            .await
            .expect("proposal step runs");
        assert_eq!(
            detail.run.state,
            CodeRunState::AwaitingApproval,
            "{detail:?}"
        );
        assert_eq!(detail.invocations.len(), 1);
        let invocation = &detail.invocations[0];
        assert_eq!(
            invocation.tool_name,
            crate::code_write_tools::EDIT_FILE_TOOL_ID
        );
        assert_eq!(
            invocation.state,
            crate::code_sessions::CodeToolInvocationState::Proposed
        );
        assert!(invocation.preview.as_deref().is_some_and(|diff| {
            diff.contains("- fn main() {}") && diff.contains("+ fn main()")
        }));
        assert!(invocation.preview_hash.is_some());
        assert!(invocation.precondition_hash.is_some());
        assert!(detail.observations.iter().all(|observation| {
            observation.kind != CodeObservationKind::ToolResult
                && observation.kind != CodeObservationKind::ToolError
        }));
        assert_eq!(
            std::fs::read_to_string(repository.join("lib.rs")).expect("fixture file readable"),
            "fn main() {}\n",
            "a model proposal must never mutate before local-user approval"
        );

        let _ = std::fs::remove_dir_all(repository);
        remove_test_database(&db_path);
    }

    #[tokio::test]
    async fn model_checkpoint_and_command_calls_become_typed_per_use_proposals() {
        let (checkpoint_state, checkpoint_db_path) = test_state();
        let checkpoint_repository = fixture_repository();
        let (port, _requests) = start_scripted_stream_server(vec![named_tool_call_response(
            crate::code_git_tools::CHECKPOINT_TOOL_ID,
            serde_json::json!({"message": "Verified change"}),
        )])
        .await;
        point_default_provider_at_mock(&checkpoint_state, port).await;
        let (session_id, run_id, _project_id) =
            fixture_run(&checkpoint_state, &checkpoint_repository, 3);
        let managed_root = std::path::PathBuf::from(
            &checkpoint_state
                .workspace
                .lock()
                .expect("workspace lock")
                .root_path,
        )
        .join("ark-code-repositories")
        .join(&session_id);
        std::fs::write(
            managed_root.join("lib.rs"),
            "fn main() { println!(\"safe\"); }\n",
        )
        .expect("managed edit");
        let checkpoint = run_step(&checkpoint_state, &session_id, &run_id)
            .await
            .expect("checkpoint proposed");
        assert_eq!(checkpoint.run.state, CodeRunState::AwaitingApproval);
        assert_eq!(
            checkpoint.invocations[0].tool_name,
            crate::code_git_tools::CHECKPOINT_TOOL_ID
        );
        assert!(checkpoint.invocations[0]
            .preview
            .as_deref()
            .is_some_and(|value| {
                value.contains("Verified change") && value.contains("lib.rs")
            }));

        let (command_state, command_db_path) = test_state();
        let command_repository = fixture_repository();
        let command = command_state
            .db
            .lock()
            .expect("db lock")
            .save_code_command_definition(&crate::code_sessions::SaveCodeCommandDefinition {
                id: None,
                label: "Cargo version",
                program: "cargo",
                arguments: &["--version".to_string()],
                timeout_seconds: 30,
                enabled: true,
            })
            .expect("command saved");
        let (port, _requests) = start_scripted_stream_server(vec![named_tool_call_response(
            crate::code_command_tools::RUN_COMMAND_TOOL_ID,
            serde_json::json!({"command_id": command.id}),
        )])
        .await;
        point_default_provider_at_mock(&command_state, port).await;
        let (session_id, run_id, _project_id) = fixture_run(&command_state, &command_repository, 3);
        let command_proposal = run_step(&command_state, &session_id, &run_id)
            .await
            .expect("command proposed");
        assert_eq!(command_proposal.run.state, CodeRunState::AwaitingApproval);
        assert_eq!(
            command_proposal.invocations[0].tool_name,
            crate::code_command_tools::RUN_COMMAND_TOOL_ID
        );
        assert!(command_proposal.invocations[0]
            .preview
            .as_deref()
            .is_some_and(|value| value.contains("cargo --version") && value.contains("stripped")));

        let _ = std::fs::remove_dir_all(checkpoint_repository);
        remove_test_database(&checkpoint_db_path);
        let _ = std::fs::remove_dir_all(command_repository);
        remove_test_database(&command_db_path);
    }

    #[tokio::test]
    async fn clarification_pauses_the_run_without_side_effect_approval() {
        let (state, db_path) = test_state();
        let repository = fixture_repository();
        let (port, _requests) = start_scripted_stream_server(vec![named_tool_call_response(
            crate::code_tools::REQUEST_CLARIFICATION_TOOL_ID,
            serde_json::json!({"question": "Which compatibility target should I preserve?"}),
        )])
        .await;
        point_default_provider_at_mock(&state, port).await;
        let (session_id, run_id, _project_id) = fixture_run(&state, &repository, 3);
        let detail = run_step(&state, &session_id, &run_id)
            .await
            .expect("clarification proposed");
        assert_eq!(detail.run.state, CodeRunState::Interrupted);
        assert_eq!(
            detail.run.terminal_reason.as_deref(),
            Some("clarification_requested")
        );
        assert_eq!(
            detail.invocations[0].tool_name,
            crate::code_tools::REQUEST_CLARIFICATION_TOOL_ID
        );
        assert_eq!(
            detail.invocations[0].state,
            crate::code_sessions::CodeToolInvocationState::Applied
        );
        assert!(detail.invocations[0].preview.is_none());

        let _ = std::fs::remove_dir_all(repository);
        remove_test_database(&db_path);
    }

    #[tokio::test]
    async fn run_step_persists_a_tool_error_observation_without_failing_the_run() {
        let (state, db_path) = test_state();
        let repository = fixture_repository();
        let (port, _requests) =
            start_scripted_stream_server(vec![tool_call_response("does-not-exist.rs")]).await;
        point_default_provider_at_mock(&state, port).await;
        let (session_id, run_id, _project_id) = fixture_run(&state, &repository, 3);

        let detail = run_step(&state, &session_id, &run_id)
            .await
            .expect("step runs");
        // A failed tool call still lets the run continue investigating on the next step.
        assert_eq!(detail.run.state, CodeRunState::Observing);
        assert_eq!(detail.invocations.len(), 1);
        assert_eq!(
            detail.invocations[0].state,
            crate::code_sessions::CodeToolInvocationState::Failed
        );
        assert!(detail
            .observations
            .iter()
            .any(|observation| observation.kind == CodeObservationKind::ToolError));

        let _ = std::fs::remove_dir_all(repository);
        remove_test_database(&db_path);
    }

    #[tokio::test]
    async fn third_consecutive_identical_tool_call_stops_before_execution() {
        let (state, db_path) = test_state();
        let repository = fixture_repository();
        let (port, _requests) = start_scripted_stream_server(vec![
            tool_call_response("lib.rs"),
            tool_call_response("lib.rs"),
            tool_call_response("lib.rs"),
        ])
        .await;
        point_default_provider_at_mock(&state, port).await;
        let (session_id, run_id, _project_id) = fixture_run(&state, &repository, 4);

        assert_eq!(
            run_step_with_cancellation(&state, &session_id, &run_id, "loop-executor", None)
                .await
                .expect("first call runs")
                .run
                .state,
            CodeRunState::Observing
        );
        assert_eq!(
            run_step_with_cancellation(&state, &session_id, &run_id, "loop-executor", None)
                .await
                .expect("second call runs")
                .run
                .state,
            CodeRunState::Observing
        );
        let stopped =
            run_step_with_cancellation(&state, &session_id, &run_id, "loop-executor", None)
                .await
                .expect("third call is durably stopped");
        assert_eq!(stopped.run.state, CodeRunState::Failed);
        assert_eq!(
            stopped.run.terminal_reason.as_deref(),
            Some("repeated_identical_tool_call")
        );
        assert_eq!(stopped.invocations.len(), 3);
        assert_eq!(
            stopped.invocations.last().expect("third invocation").state,
            crate::code_sessions::CodeToolInvocationState::Failed
        );
        assert!(stopped.observations.iter().any(|observation| {
            observation.kind == CodeObservationKind::ToolError
                && observation.content.contains("third consecutive")
        }));

        let _ = std::fs::remove_dir_all(repository);
        remove_test_database(&db_path);
    }

    #[tokio::test]
    async fn run_step_executes_all_read_only_tool_calls_in_one_response() {
        // G2/RC-08: all read-only calls in a single provider response are now executed; a
        // prior implementation silently discarded everything after the first call.
        let (state, db_path) = test_state();
        let repository = fixture_repository();
        let (port, _requests) = start_scripted_stream_server(vec![two_tool_calls_response()]).await;
        point_default_provider_at_mock(&state, port).await;
        let (session_id, run_id, _project_id) = fixture_run(&state, &repository, 3);

        let detail = run_step(&state, &session_id, &run_id)
            .await
            .expect("step runs");
        assert_eq!(detail.run.state, CodeRunState::Observing);
        assert_eq!(
            detail.invocations.len(),
            2,
            "both returned read-only tool calls must be executed"
        );
        let mut tool_names: Vec<&str> = detail
            .invocations
            .iter()
            .map(|i| i.tool_name.as_str())
            .collect();
        tool_names.sort();
        assert_eq!(
            tool_names,
            vec!["list_directory", "read_file"],
            "both tool calls must appear in durable state"
        );

        let _ = std::fs::remove_dir_all(repository);
        remove_test_database(&db_path);
    }
}
