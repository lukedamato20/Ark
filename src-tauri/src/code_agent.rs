//! CODE-007 (partial): a synchronous, read-only Ark Code agent loop.
//!
//! `run_step` drives exactly one model turn of an existing `queued`/`observing` run: it checks
//! budgets and repository identity, claims `planning`, calls the selected provider through
//! `providers::stream_tools_for_model` with CODE-004's read-only tool schemas
//! (`code_tools::provider_tool_definitions()` — `edit_file` is never offered here; there is no
//! approval gate yet for a model-initiated write), executes at most one requested tool call, and
//! commits the step, moving the run to `observing` (a tool ran) or `completed` (the model gave a
//! final answer). The whole step is awaited by its Tauri command, not backgrounded: there is no
//! incremental streaming, cancellation, executor lease, or crash/startup recovery in this pass —
//! see `implementation-plan.md`'s CODE-007 entry for what remains.
//!
//! Every DB write happens through `Database::transition_code_agent_run`/`commit_code_agent_step`,
//! which the ADR requires. The DB mutex is never held across the provider call or tool execution
//! (both `.await` points): each phase locks, reads or writes, and unlocks before the next `.await`,
//! matching `generation.rs`'s existing discipline.

use crate::chat::ChatMessage;
use crate::code_sessions::{
    CodeObservationKind, CodeRunDetail, CodeRunState, NewCodeAgentStep, NewCodeToolCallOutcome,
};
use crate::code_tools::RepositoryContext;
use crate::errors::AppError;
use crate::providers::{
    stream_tools_for_model, ProviderChatRequest, ProviderContextBlock, ProviderContextKind,
    ProviderRegistry, ProviderToolCall, ProviderToolEvent, ProviderToolRequest,
};
use crate::AppState;
use std::collections::HashMap;
use std::time::Instant;

const MAX_OBSERVATION_CONTENT_CHARS: usize = 8_000;
const MIN_STEP_TOKEN_BUDGET: u64 = 256;

pub async fn run_step(
    state: &AppState,
    session_id: &str,
    run_id: &str,
) -> Result<CodeRunDetail, AppError> {
    let ready = prepare_step(state, session_id, run_id)?;
    let Some(ready) = ready else {
        // A budget/identity/provider-readiness check already transitioned the run to a terminal
        // or interrupted state; the caller re-fetches the resulting detail.
        return crate::commands::lock_read_db(state)?.get_code_run_detail(run_id);
    };

    crate::commands::lock_db(state)?.transition_code_agent_run(
        run_id,
        &[CodeRunState::Queued, CodeRunState::Observing],
        CodeRunState::Planning,
        None,
        "run_planning",
        "Step planning started",
    )?;

    let bearer_token = crate::secret_store::resolve_bearer_token(state, &ready.provider_config)?;
    let provider = match ProviderRegistry::create_with_bearer_token(
        ready.provider_config.clone(),
        bearer_token,
    ) {
        Ok(provider) => provider,
        Err(error) => return interrupt_for_provider_error(state, run_id, &error.message),
    };

    let system_instructions = format!(
        "You are Ark Code, a read-only repository investigation agent for the Repository at {}. \
         Use the provided tools to explore files, search text, and inspect Git status/diff so you \
         can answer the task below. When you have gathered enough information, respond in plain \
         text with your final answer and do not call another tool.",
        ready.run_repository_path
    );
    let chat = ProviderChatRequest {
        model: ready.model_name.clone(),
        system_instructions: Some(system_instructions),
        messages: vec![ChatMessage {
            role: "user".to_string(),
            content: ready.task.clone(),
        }],
        untrusted_context: ready.untrusted_context,
        temperature: None,
        max_tokens: Some(ready.max_tokens_for_request),
        user_deadline: None,
    };
    let tool_request = ProviderToolRequest {
        chat,
        tools: crate::code_tools::provider_tool_definitions(),
    };

    let mut text = String::new();
    let mut tool_calls: Vec<ProviderToolCall> = Vec::new();
    let mut on_event = |event: ProviderToolEvent| -> Result<(), AppError> {
        match event {
            ProviderToolEvent::TextDelta { delta } => {
                if text.chars().count() < MAX_OBSERVATION_CONTENT_CHARS {
                    text.push_str(&delta);
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

    let step_started = Instant::now();
    let usage =
        match stream_tools_for_model(provider.as_ref(), &ready.model, tool_request, &mut on_event)
            .await
        {
            Ok(usage) => usage,
            Err(error) => return interrupt_for_provider_error(state, run_id, &error.message),
        };
    let active_elapsed_ms_delta =
        u64::try_from(step_started.elapsed().as_millis()).unwrap_or(u64::MAX);
    let actual_tokens = Some(
        u64::try_from(
            usage
                .input_tokens
                .unwrap_or(0)
                .saturating_add(usage.output_tokens.unwrap_or(0)),
        )
        .unwrap_or(0),
    );

    // At most one tool call is executed per step (see this module's doc comment); any additional
    // calls in the same response are visible in the step's prompt manifest but not run.
    let tool_call_outcome = if let Some(call) = tool_calls.first() {
        let canonical_arguments_json = serde_json::to_string(&call.arguments).unwrap_or_default();
        let scope_json = tool_scope_json(&call.name);
        match crate::code_tools::execute_provider_call(&ready.context, call).await {
            Ok(result) => Some(NewCodeToolCallOutcome {
                tool_name: &call.name,
                canonical_arguments_json,
                scope_json,
                succeeded: true,
                observation_content: truncate_for_storage(&result.content),
            }),
            Err(error) => Some(NewCodeToolCallOutcome {
                tool_name: &call.name,
                canonical_arguments_json,
                scope_json,
                succeeded: false,
                observation_content: truncate_for_storage(&error.message),
            }),
        }
    } else {
        None
    };

    let new_run_state = if tool_call_outcome.is_some() {
        CodeRunState::Observing
    } else {
        CodeRunState::Completed
    };
    let prompt_manifest_json = serde_json::json!({
        "task": ready.task,
        "priorObservationCount": ready.prior_observation_count,
        "toolCallsReturned": tool_calls.len(),
    })
    .to_string();

    crate::commands::lock_db(state)?.commit_code_agent_step(&NewCodeAgentStep {
        run_id,
        step_index: ready.step_index,
        prompt_manifest_json,
        reserved_tokens: ready.max_tokens_for_request.max(0) as u64,
        actual_tokens,
        active_elapsed_ms_delta,
        model_text: (!text.is_empty()).then(|| truncate_for_storage(&text)),
        tool_call: tool_call_outcome,
        new_run_state,
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
    max_tokens_for_request: i64,
    untrusted_context: Vec<ProviderContextBlock>,
    prior_observation_count: usize,
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
    if run.actual_tokens >= run.max_tokens {
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

    let session = db.get_code_session(session_id)?;
    let project = db.get_project(&session.project_id)?;
    let context = match RepositoryContext::from_project(&project) {
        Ok(context) => context,
        Err(error) => {
            drop(db);
            fail_run(
                state,
                run_id,
                CodeRunState::Interrupted,
                "repository_unavailable",
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

    let prior = db.get_code_run_detail(run_id)?;
    let step_index_by_id: HashMap<&str, u32> = prior
        .steps
        .iter()
        .map(|step| (step.id.as_str(), step.step_index))
        .collect();
    let tool_name_by_step_id: HashMap<&str, &str> = prior
        .invocations
        .iter()
        .map(|invocation| (invocation.step_id.as_str(), invocation.tool_name.as_str()))
        .collect();
    let untrusted_context: Vec<ProviderContextBlock> = prior
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
                source: format!("tool:{tool_name}#{step_index}"),
                content: observation.content.clone(),
            }
        })
        .collect();
    let prior_observation_count = untrusted_context.len();

    let remaining_tokens = run
        .max_tokens
        .saturating_sub(run.actual_tokens)
        .max(MIN_STEP_TOKEN_BUDGET);
    let max_tokens_for_request = i64::try_from(remaining_tokens).unwrap_or(i64::MAX);
    let model_name = model.name.clone();
    let task = run.task.clone();
    let run_repository_path = run.repository_path_snapshot.clone();
    let step_index = run.steps_used;

    Ok(Some(ReadyStep {
        context,
        provider_config,
        model,
        model_name,
        task,
        run_repository_path,
        step_index,
        max_tokens_for_request,
        untrusted_context,
        prior_observation_count,
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

fn interrupt_for_provider_error(
    state: &AppState,
    run_id: &str,
    message: &str,
) -> Result<CodeRunDetail, AppError> {
    // ADR 0003: dispatched-but-unconfirmed provider work becomes `interrupted`, not `failed` —
    // Ark cannot know whether the provider processed or billed the abandoned request.
    fail_run(
        state,
        run_id,
        CodeRunState::Interrupted,
        "provider_error",
        message,
    )?;
    crate::commands::lock_read_db(state)?.get_code_run_detail(run_id)
}

fn tool_scope_json(tool_name: &str) -> String {
    crate::code_tools::ark_code_tools()
        .into_iter()
        .find(|tool| tool.id == tool_name)
        .and_then(|tool| serde_json::to_string(&tool.scope).ok())
        .unwrap_or_else(|| "{}".to_string())
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
    use uuid::Uuid;

    fn test_state() -> (AppState, std::path::PathBuf) {
        let path =
            std::env::temp_dir().join(format!("ark-code-agent-test-{}.sqlite3", Uuid::new_v4()));
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
                active_streams: Mutex::new(StdHashMap::new()),
                pending_streams: Mutex::new(StdHashMap::new()),
                active_imports: Mutex::new(StdHashMap::new()),
                active_ollama_pulls: Mutex::new(StdHashMap::new()),
                active_provider_refreshes: Mutex::new(StdHashMap::new()),
                active_managed_model_downloads: Mutex::new(StdHashMap::new()),
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
    ) -> (String, String) {
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
        let context = RepositoryContext::from_project(&project).expect("context created");
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
        (session.id, run.id)
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

    fn tool_call_response(path: &str) -> MockResponsePlan {
        ollama_response(&format!(
            "{{\"message\":{{\"content\":\"Checking the file.\",\"tool_calls\":[{{\"function\":{{\"name\":\"read_file\",\"arguments\":{{\"path\":\"{path}\"}}}}}}]}},\"done\":false}}\n\
             {{\"message\":{{\"content\":\"\"}},\"done\":true,\"prompt_eval_count\":8,\"eval_count\":4}}\n",
        ))
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
    async fn run_step_rejects_a_run_in_a_non_ready_state() {
        let (state, db_path) = test_state();
        let repository = fixture_repository();
        let (session_id, run_id) = fixture_run(&state, &repository, 3);
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
        let (session_id, run_id) = fixture_run(&state, &repository, 1);

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
        let (session_id, run_id) = fixture_run(&state, &repository, 3);

        // Recreating the directory changes its platform identity metadata (creation time on
        // Windows, device/inode on Unix) without changing its path or content.
        std::fs::remove_dir_all(&repository).expect("repository removed");
        std::fs::create_dir_all(&repository).expect("repository recreated");
        std::fs::write(repository.join("lib.rs"), "fn main() {}\n")
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
    async fn run_step_completes_when_the_model_returns_no_tool_call() {
        let (state, db_path) = test_state();
        let repository = fixture_repository();
        let (port, _requests) = start_scripted_stream_server(vec![final_answer_response()]).await;
        point_default_provider_at_mock(&state, port).await;
        let (session_id, run_id) = fixture_run(&state, &repository, 3);

        let detail = run_step(&state, &session_id, &run_id)
            .await
            .expect("step runs");
        assert_eq!(detail.run.state, CodeRunState::Completed);
        assert!(detail.run.completed_at.is_some());
        assert!(detail
            .observations
            .iter()
            .any(
                |observation| observation.kind == CodeObservationKind::ModelText
                    && observation.content.contains("looks fine")
            ));
        assert!(detail.invocations.is_empty());

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
        let (session_id, run_id) = fixture_run(&state, &repository, 3);

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
    async fn run_step_persists_a_tool_error_observation_without_failing_the_run() {
        let (state, db_path) = test_state();
        let repository = fixture_repository();
        let (port, _requests) =
            start_scripted_stream_server(vec![tool_call_response("does-not-exist.rs")]).await;
        point_default_provider_at_mock(&state, port).await;
        let (session_id, run_id) = fixture_run(&state, &repository, 3);

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
    async fn run_step_executes_only_the_first_of_multiple_tool_calls_in_one_response() {
        let (state, db_path) = test_state();
        let repository = fixture_repository();
        let (port, _requests) = start_scripted_stream_server(vec![two_tool_calls_response()]).await;
        point_default_provider_at_mock(&state, port).await;
        let (session_id, run_id) = fixture_run(&state, &repository, 3);

        let detail = run_step(&state, &session_id, &run_id)
            .await
            .expect("step runs");
        assert_eq!(detail.run.state, CodeRunState::Observing);
        assert_eq!(
            detail.invocations.len(),
            1,
            "only the first of the two returned tool calls is executed"
        );
        assert_eq!(detail.invocations[0].tool_name, "read_file");

        let _ = std::fs::remove_dir_all(repository);
        remove_test_database(&db_path);
    }
}
