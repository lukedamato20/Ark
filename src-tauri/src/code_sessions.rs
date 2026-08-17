//! CODE-007 / ADR 0003 durable Ark Code session and run protocol.
//!
//! SQLite rows represented here are authoritative. Frontend events are only notifications and
//! provider/tool futures are only process controls; neither may manufacture state transitions.

use crate::errors::AppError;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::Path;

pub const CODE_RUN_EVENT_SCHEMA_VERSION: u32 = 1;
pub const DEFAULT_MAX_STEPS: u32 = 12;
pub const DEFAULT_MAX_ACTIVE_MS: u64 = 10 * 60 * 1_000;
pub const DEFAULT_MAX_TOKENS: u64 = 32_768;
pub const MAX_SESSION_TITLE_CHARS: usize = 120;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CodeRunState {
    Queued,
    Planning,
    AwaitingApproval,
    ExecutingTool,
    Observing,
    Completed,
    Failed,
    Cancelled,
    Interrupted,
}

impl CodeRunState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Planning => "planning",
            Self::AwaitingApproval => "awaiting_approval",
            Self::ExecutingTool => "executing_tool",
            Self::Observing => "observing",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
            Self::Interrupted => "interrupted",
        }
    }

    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Completed | Self::Failed | Self::Cancelled | Self::Interrupted
        )
    }
}

impl TryFrom<&str> for CodeRunState {
    type Error = AppError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "queued" => Ok(Self::Queued),
            "planning" => Ok(Self::Planning),
            "awaiting_approval" => Ok(Self::AwaitingApproval),
            "executing_tool" => Ok(Self::ExecutingTool),
            "observing" => Ok(Self::Observing),
            "completed" => Ok(Self::Completed),
            "failed" => Ok(Self::Failed),
            "cancelled" => Ok(Self::Cancelled),
            "interrupted" => Ok(Self::Interrupted),
            _ => Err(AppError::new(
                "code_run_state_invalid",
                "Ark Code found an unknown durable run state and refused to guess.",
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CodeRecoveryOutcome {
    Applied,
    NotApplied,
    Diverged,
    Unknown,
}

impl TryFrom<&str> for CodeRecoveryOutcome {
    type Error = AppError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "applied" => Ok(Self::Applied),
            "not_applied" => Ok(Self::NotApplied),
            "diverged" => Ok(Self::Diverged),
            "unknown" => Ok(Self::Unknown),
            _ => Err(AppError::new(
                "code_recovery_outcome_invalid",
                "Ark Code found an unknown recovery outcome and refused to guess.",
            )),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeSession {
    pub id: String,
    pub project_id: String,
    pub title: String,
    pub archived: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeAgentRun {
    pub id: String,
    pub session_id: String,
    pub parent_run_id: Option<String>,
    pub provider_id: String,
    pub model_id: String,
    /// CODE-007: what the user asked Ark Code to investigate. Immutable once the run is created —
    /// a different task is a different run.
    pub task: String,
    pub repository_path_snapshot: String,
    pub repository_identity_hash: String,
    pub state: CodeRunState,
    pub max_steps: u32,
    pub max_active_ms: u64,
    pub max_tokens: u64,
    pub max_cost_microunits: Option<u64>,
    pub steps_used: u32,
    pub active_elapsed_ms: u64,
    pub reserved_tokens: u64,
    pub actual_tokens: u64,
    pub actual_cost_microunits: Option<u64>,
    pub cancel_requested_at: Option<String>,
    pub terminal_reason: Option<String>,
    pub recovery_outcome: Option<CodeRecoveryOutcome>,
    pub created_at: String,
    pub updated_at: String,
    pub completed_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeRunEvent {
    pub run_id: String,
    pub sequence: u64,
    pub schema_version: u32,
    pub kind: String,
    pub state: CodeRunState,
    pub summary: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeSessionDetail {
    pub session: CodeSession,
    pub runs: Vec<CodeAgentRun>,
    pub events: Vec<CodeRunEvent>,
}

pub struct NewCodeRun<'a> {
    pub session_id: &'a str,
    pub parent_run_id: Option<&'a str>,
    pub provider_id: &'a str,
    pub model_id: &'a str,
    pub task: &'a str,
    pub repository_path_snapshot: &'a str,
    pub repository_identity_hash: &'a str,
    pub max_steps: u32,
    pub max_active_ms: u64,
    pub max_tokens: u64,
    pub max_cost_microunits: Option<u64>,
    pub idempotency_key: &'a str,
    pub request_hash: &'a str,
}

/// CODE-007's read-only agent loop's own DTOs. One step, at most one executed tool call, and its
/// resulting observation — see `code_agent::run_step`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CodeAgentStepState {
    Reserved,
    Dispatched,
    Completed,
    Failed,
    Interrupted,
}

impl CodeAgentStepState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Reserved => "reserved",
            Self::Dispatched => "dispatched",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Interrupted => "interrupted",
        }
    }
}

impl TryFrom<&str> for CodeAgentStepState {
    type Error = AppError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "reserved" => Ok(Self::Reserved),
            "dispatched" => Ok(Self::Dispatched),
            "completed" => Ok(Self::Completed),
            "failed" => Ok(Self::Failed),
            "interrupted" => Ok(Self::Interrupted),
            _ => Err(AppError::new(
                "code_agent_step_state_invalid",
                "Ark Code found an unknown durable step state and refused to guess.",
            )),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeAgentStep {
    pub id: String,
    pub run_id: String,
    pub step_index: u32,
    pub state: CodeAgentStepState,
    pub reserved_tokens: u64,
    pub actual_tokens: Option<u64>,
    pub created_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CodeToolInvocationState {
    Proposed,
    Approved,
    Executing,
    Applied,
    Failed,
    Denied,
    Interrupted,
}

impl CodeToolInvocationState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Proposed => "proposed",
            Self::Approved => "approved",
            Self::Executing => "executing",
            Self::Applied => "applied",
            Self::Failed => "failed",
            Self::Denied => "denied",
            Self::Interrupted => "interrupted",
        }
    }
}

impl TryFrom<&str> for CodeToolInvocationState {
    type Error = AppError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "proposed" => Ok(Self::Proposed),
            "approved" => Ok(Self::Approved),
            "executing" => Ok(Self::Executing),
            "applied" => Ok(Self::Applied),
            "failed" => Ok(Self::Failed),
            "denied" => Ok(Self::Denied),
            "interrupted" => Ok(Self::Interrupted),
            _ => Err(AppError::new(
                "code_tool_invocation_state_invalid",
                "Ark Code found an unknown durable tool invocation state and refused to guess.",
            )),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeToolInvocation {
    pub id: String,
    pub run_id: String,
    pub step_id: String,
    pub tool_name: String,
    pub canonical_arguments_json: String,
    pub state: CodeToolInvocationState,
    pub created_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CodeObservationKind {
    ToolResult,
    ToolError,
    ModelText,
    System,
}

impl CodeObservationKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ToolResult => "tool_result",
            Self::ToolError => "tool_error",
            Self::ModelText => "model_text",
            Self::System => "system",
        }
    }
}

impl TryFrom<&str> for CodeObservationKind {
    type Error = AppError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "tool_result" => Ok(Self::ToolResult),
            "tool_error" => Ok(Self::ToolError),
            "model_text" => Ok(Self::ModelText),
            "system" => Ok(Self::System),
            _ => Err(AppError::new(
                "code_observation_kind_invalid",
                "Ark Code found an unknown durable observation kind and refused to guess.",
            )),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeObservation {
    pub id: String,
    pub run_id: String,
    pub step_id: String,
    pub kind: CodeObservationKind,
    pub content: String,
    pub created_at: String,
}

/// Everything CODE-007's `CodeView` needs to render one run's autonomous progress: the run itself
/// plus every step/invocation/observation/event it has produced so far, run-scoped (steps are not
/// session-scoped, unlike `CodeSessionDetail`'s runs/events).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeRunDetail {
    pub run: CodeAgentRun,
    pub steps: Vec<CodeAgentStep>,
    pub invocations: Vec<CodeToolInvocation>,
    pub observations: Vec<CodeObservation>,
    pub events: Vec<CodeRunEvent>,
}

/// Inputs for `Database::commit_code_agent_step` — bundles everything one `code_agent::run_step`
/// call produces after its provider call and (at most one) tool execution complete, so it can be
/// persisted in a single transaction. `tool_call` and `model_text` are each optional and
/// independent: a step may produce either, both, or (defensively) neither.
pub struct NewCodeAgentStep<'a> {
    pub run_id: &'a str,
    pub step_index: u32,
    pub prompt_manifest_json: String,
    pub reserved_tokens: u64,
    pub actual_tokens: Option<u64>,
    pub active_elapsed_ms_delta: u64,
    pub model_text: Option<String>,
    pub tool_call: Option<NewCodeToolCallOutcome<'a>>,
    /// `Observing` when a tool call was executed (ready for the next step); `Completed` when the
    /// model returned a final answer with no tool call.
    pub new_run_state: CodeRunState,
}

pub struct NewCodeToolCallOutcome<'a> {
    pub tool_name: &'a str,
    pub canonical_arguments_json: String,
    pub scope_json: String,
    pub succeeded: bool,
    /// The tool's result content on success, or a bounded error message on failure.
    pub observation_content: String,
}

pub const MAX_TASK_CHARS: usize = 4_000;

pub fn validate_task(value: &str) -> Result<String, AppError> {
    let task = value.trim();
    if task.is_empty() || task.chars().count() > MAX_TASK_CHARS {
        return Err(AppError::invalid_input(format!(
            "Ark Code run task must be between 1 and {MAX_TASK_CHARS} characters."
        )));
    }
    Ok(task.to_string())
}

pub fn validate_session_title(value: &str) -> Result<String, AppError> {
    let title = value.trim();
    if title.is_empty() || title.chars().count() > MAX_SESSION_TITLE_CHARS {
        return Err(AppError::invalid_input(format!(
            "Ark Code session title must be between 1 and {MAX_SESSION_TITLE_CHARS} characters."
        )));
    }
    Ok(title.to_string())
}

pub fn validate_run_budgets(
    max_steps: u32,
    max_active_ms: u64,
    max_tokens: u64,
) -> Result<(), AppError> {
    if !(1..=64).contains(&max_steps) {
        return Err(AppError::invalid_input(
            "Ark Code max steps must be between 1 and 64.",
        ));
    }
    if !(1_000..=3_600_000).contains(&max_active_ms) {
        return Err(AppError::invalid_input(
            "Ark Code active time limit must be between 1 second and 1 hour.",
        ));
    }
    if !(256..=1_000_000).contains(&max_tokens) {
        return Err(AppError::invalid_input(
            "Ark Code token budget must be between 256 and 1,000,000 tokens.",
        ));
    }
    Ok(())
}

/// Snapshots the canonical Repository root and a platform metadata identity fingerprint. A
/// delete/recreate at the same path must not silently retarget an existing run.
pub fn repository_snapshot(root: &Path) -> Result<(String, String), AppError> {
    let canonical = crate::validation::canonicalize_for_use(root, "Repository path")?;
    let metadata = std::fs::symlink_metadata(&canonical).map_err(|_| {
        AppError::new(
            "repository_unavailable",
            "The bound Repository is no longer available.",
        )
    })?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(AppError::new(
            "repository_identity_changed",
            "The bound Repository identity changed. Rebind it before starting Ark Code.",
        ));
    }
    let path = canonical
        .to_str()
        .ok_or_else(|| AppError::invalid_input("Repository path must contain valid Unicode."))?;

    let mut identity = format!("path:{path}");
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        identity.push_str(&format!(
            "|created:{}|attributes:{}",
            metadata.creation_time(),
            metadata.file_attributes()
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        identity.push_str(&format!("|dev:{}|ino:{}", metadata.dev(), metadata.ino()));
    }
    #[cfg(not(any(unix, windows)))]
    {
        identity.push_str(&format!("|modified:{:?}", metadata.modified().ok()));
    }

    let hash = format!("{:x}", Sha256::digest(identity.as_bytes()));
    Ok((path.to_string(), hash))
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn hash_json<T: Serialize>(value: &T) -> Result<String, AppError> {
    let bytes = serde_json::to_vec(value).map_err(|_| {
        AppError::new(
            "code_request_serialization_failed",
            "Ark Code could not safely fingerprint this request.",
        )
    })?;
    Ok(sha256_hex(&bytes))
}

pub fn request_hash<T: Serialize>(request: &T) -> Result<String, AppError> {
    hash_json(request)
}

/// ADR 0003's approval binding: the fingerprint of a proposed tool call's own typed arguments.
/// Struct field order is fixed by the Rust source, so `serde_json::to_vec` on a strongly-typed
/// argument struct (never a loosely-typed `Value`) serializes deterministically without needing
/// canonical-JSON key sorting.
pub fn compute_call_hash<T: Serialize>(arguments: &T) -> Result<String, AppError> {
    hash_json(arguments)
}

/// ADR 0003's approval binding: the fingerprint of the exact human-readable preview text shown to
/// the user before approval. An approval echoing back a different hash than what is currently
/// proposed did not approve the change actually about to run.
pub fn compute_preview_hash(preview_text: &str) -> String {
    sha256_hex(preview_text.as_bytes())
}

/// ADR 0003's approval binding: the fingerprint of the typed preconditions a proposal was
/// evaluated against (e.g. a file's current content hash). Execution re-derives this hash from
/// live state and refuses if it no longer matches what was approved, rather than trusting a
/// stale approval against state that has since changed.
pub fn compute_precondition_hash<T: Serialize>(preconditions: &T) -> Result<String, AppError> {
    hash_json(preconditions)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_agent_step_and_tool_invocation_and_observation_state_round_trips() {
        for state in [
            CodeAgentStepState::Reserved,
            CodeAgentStepState::Dispatched,
            CodeAgentStepState::Completed,
            CodeAgentStepState::Failed,
            CodeAgentStepState::Interrupted,
        ] {
            assert_eq!(CodeAgentStepState::try_from(state.as_str()).unwrap(), state);
        }
        for state in [
            CodeToolInvocationState::Proposed,
            CodeToolInvocationState::Approved,
            CodeToolInvocationState::Executing,
            CodeToolInvocationState::Applied,
            CodeToolInvocationState::Failed,
            CodeToolInvocationState::Denied,
            CodeToolInvocationState::Interrupted,
        ] {
            assert_eq!(
                CodeToolInvocationState::try_from(state.as_str()).unwrap(),
                state
            );
        }
        for kind in [
            CodeObservationKind::ToolResult,
            CodeObservationKind::ToolError,
            CodeObservationKind::ModelText,
            CodeObservationKind::System,
        ] {
            assert_eq!(CodeObservationKind::try_from(kind.as_str()).unwrap(), kind);
        }
        assert!(CodeAgentStepState::try_from("bogus").is_err());
        assert!(CodeToolInvocationState::try_from("bogus").is_err());
        assert!(CodeObservationKind::try_from("bogus").is_err());
    }

    #[test]
    fn task_validation_trims_and_bounds_length() {
        assert_eq!(
            validate_task("  investigate the parser  ").unwrap(),
            "investigate the parser"
        );
        assert!(validate_task(" ").is_err());
        assert!(validate_task(&"x".repeat(MAX_TASK_CHARS + 1)).is_err());
        assert!(validate_task(&"x".repeat(MAX_TASK_CHARS)).is_ok());
    }

    #[test]
    fn every_durable_state_round_trips_and_terminal_states_stay_closed() {
        let states = [
            CodeRunState::Queued,
            CodeRunState::Planning,
            CodeRunState::AwaitingApproval,
            CodeRunState::ExecutingTool,
            CodeRunState::Observing,
            CodeRunState::Completed,
            CodeRunState::Failed,
            CodeRunState::Cancelled,
            CodeRunState::Interrupted,
        ];
        for state in states {
            assert_eq!(CodeRunState::try_from(state.as_str()).unwrap(), state);
        }
        assert!(states[5..].iter().all(|state| state.is_terminal()));
        assert!(states[..5].iter().all(|state| !state.is_terminal()));
    }

    #[test]
    fn titles_and_budget_hard_limits_fail_closed() {
        assert!(validate_session_title("  investigate parser  ").is_ok());
        assert!(validate_session_title(" ").is_err());
        assert!(validate_session_title(&"x".repeat(121)).is_err());
        assert!(validate_run_budgets(1, 1_000, 256).is_ok());
        assert!(validate_run_budgets(65, 1_000, 256).is_err());
        assert!(validate_run_budgets(1, 999, 256).is_err());
        assert!(validate_run_budgets(1, 1_000, 255).is_err());
    }

    #[derive(Serialize)]
    struct Fixture<'a> {
        path: &'a str,
        edits: Vec<&'a str>,
    }

    #[test]
    fn approval_hashes_are_deterministic_and_sensitive_to_any_change() {
        let a = Fixture {
            path: "src/lib.rs",
            edits: vec!["one"],
        };
        let a_again = Fixture {
            path: "src/lib.rs",
            edits: vec!["one"],
        };
        let b = Fixture {
            path: "src/lib.rs",
            edits: vec!["two"],
        };
        assert_eq!(
            compute_call_hash(&a).unwrap(),
            compute_call_hash(&a_again).unwrap()
        );
        assert_ne!(
            compute_call_hash(&a).unwrap(),
            compute_call_hash(&b).unwrap()
        );
        assert_eq!(
            compute_precondition_hash(&a).unwrap(),
            compute_precondition_hash(&a_again).unwrap()
        );
        assert_ne!(
            compute_precondition_hash(&a).unwrap(),
            compute_precondition_hash(&b).unwrap()
        );

        assert_eq!(
            compute_preview_hash("Replace X with Y"),
            compute_preview_hash("Replace X with Y")
        );
        assert_ne!(
            compute_preview_hash("Replace X with Y"),
            compute_preview_hash("Replace X with Z")
        );
        assert_eq!(compute_call_hash(&a).unwrap().len(), 64);
    }

    #[test]
    fn repository_identity_is_stable_across_normal_content_changes() {
        let root = std::env::temp_dir().join(format!("ark-code-identity-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).expect("repository created");
        let before = repository_snapshot(&root).expect("initial identity");
        std::fs::write(root.join("new-file.txt"), "content").expect("repository content changed");
        let after = repository_snapshot(&root).expect("identity after content change");
        assert_eq!(before, after);
        std::fs::remove_dir_all(root).expect("fixture removed");
    }
}
