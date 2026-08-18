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

/// Lightweight process event: notification only. Clients refetch `CodeRunDetail` and use the
/// durable event sequence to detect duplicates/gaps rather than rendering this payload as truth.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeRunUpdatedEvent {
    pub run_id: String,
    pub session_id: String,
    pub sequence: u64,
    pub schema_version: u32,
    pub state: CodeRunState,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeSessionDetail {
    pub session: CodeSession,
    pub runs: Vec<CodeAgentRun>,
    pub events: Vec<CodeRunEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeSessionRepository {
    pub session_id: String,
    pub root_path: String,
    pub repository_identity_hash: String,
    pub branch_name: String,
    pub base_commit_oid: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeGitCheckpoint {
    pub id: String,
    pub session_id: String,
    pub run_id: String,
    pub invocation_id: String,
    pub commit_oid: String,
    pub parent_commit_oid: String,
    pub tree_oid: String,
    pub message: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeCommandDefinition {
    pub id: String,
    pub label: String,
    pub program: String,
    pub arguments: Vec<String>,
    pub timeout_seconds: u32,
    pub enabled: bool,
    pub created_at: String,
    pub updated_at: String,
}

pub struct NewCodeSessionRepository<'a> {
    pub session_id: &'a str,
    pub root_path: &'a str,
    pub repository_identity_hash: &'a str,
    pub branch_name: &'a str,
    pub base_commit_oid: &'a str,
}

pub struct NewCodeGitCheckpoint<'a> {
    pub session_id: &'a str,
    pub run_id: &'a str,
    pub invocation_id: &'a str,
    pub commit_oid: &'a str,
    pub parent_commit_oid: &'a str,
    pub tree_oid: &'a str,
    pub message: &'a str,
}

pub struct SaveCodeCommandDefinition<'a> {
    pub id: Option<&'a str>,
    pub label: &'a str,
    pub program: &'a str,
    pub arguments: &'a [String],
    pub timeout_seconds: u32,
    pub enabled: bool,
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
    pub streaming_text: Option<String>,
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
    pub call_hash: String,
    pub state: CodeToolInvocationState,
    pub preview: Option<String>,
    pub preview_hash: Option<String>,
    pub precondition_hash: Option<String>,
    pub approved_at: Option<String>,
    pub verification_outcome: Option<CodeRecoveryOutcome>,
    pub created_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CodeObservationKind {
    ToolResult,
    ToolError,
    ModelText,
    System,
    /// G2/RC-03: persisted when a tool-free model response is rejected because it did not meet the
    /// completion contract (no current-run evidence, unresolved error, or empty text). The content
    /// explains the missing condition and is injected into the next provider turn so the model can
    /// correct its behaviour without consuming a silent context slot.
    CompletionRejected,
}

impl CodeObservationKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ToolResult => "tool_result",
            Self::ToolError => "tool_error",
            Self::ModelText => "model_text",
            Self::System => "system",
            Self::CompletionRejected => "completion_rejected",
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
            "completion_rejected" => Ok(Self::CompletionRejected),
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

/// Inputs for the ADR-0003 pre-dispatch claim. The step row, conservative token reservation, and
/// executor lease are committed together before any provider request can leave the process.
pub struct NewCodeAgentStepClaim<'a> {
    pub run_id: &'a str,
    pub step_index: u32,
    pub prompt_manifest_json: &'a str,
    /// Visible, durable notice when CODE-006 omitted older context to fit this model. The same
    /// summary is supplied to the provider as untrusted context; omitted content is never
    /// silently represented as complete.
    pub context_compaction_summary: Option<&'a str>,
    pub reserved_tokens: u64,
    pub reserved_cost_microunits: Option<u64>,
    pub executor_lease_id: &'a str,
    pub executor_lease_expires_at: &'a str,
}

/// Inputs for `Database::commit_code_agent_step` — bundles everything one already-reserved and
/// dispatched step produces after its provider call and all accepted tool executions complete,
/// so they can be persisted in a single ownership-checked transaction. `tool_calls` (G2/RC-08:
/// now a `Vec` to support multiple executed read-only calls per step), `model_text`, and
/// `completion_rejection` are each optional and independent.
pub struct NewCodeAgentStep<'a> {
    pub run_id: &'a str,
    pub step_id: &'a str,
    pub executor_lease_id: &'a str,
    pub executor_lease_expires_at: &'a str,
    pub step_index: u32,
    pub actual_tokens: Option<u64>,
    pub active_elapsed_ms_delta: u64,
    pub model_text: Option<String>,
    /// G2/RC-08: all tool calls executed (or proposed) in this step. Read-only calls all execute;
    /// approval-capable calls are represented as proposals; at most one approval per step.
    pub tool_calls: Vec<NewCodeToolCallOutcome<'a>>,
    /// G2/RC-03: typed reason when a tool-free response was rejected. Persisted as a
    /// `completion_rejected` observation that the next provider turn will see as context.
    pub completion_rejection: Option<String>,
    /// `Observing` when tool calls ran (ready for next step); `Completed` when the model's final
    /// answer was accepted; `AwaitingApproval` when a write proposal is pending.
    pub new_run_state: CodeRunState,
    pub terminal_reason: Option<&'a str>,
}

/// Ownership-checked terminalization of a claimed step. A dispatched provider request whose
/// usage is unknown retains its conservative reservation; known pre-dispatch or post-response
/// outcomes release it and record any authoritative usage that was returned.
pub struct FinishCodeAgentStep<'a> {
    pub run_id: &'a str,
    pub step_id: &'a str,
    pub executor_lease_id: &'a str,
    pub step_state: CodeAgentStepState,
    pub run_state: CodeRunState,
    pub terminal_reason: &'a str,
    pub event_kind: &'a str,
    pub event_summary: &'a str,
    pub actual_tokens: Option<u64>,
    pub active_elapsed_ms_delta: u64,
    pub retain_reservation: bool,
}

pub struct NewCodeToolCallOutcome<'a> {
    pub provider_call_id: Option<&'a str>,
    pub tool_name: &'a str,
    pub canonical_arguments_json: String,
    pub scope_json: String,
    pub succeeded: bool,
    /// The tool's result content on success, or a bounded error message on failure.
    pub observation_content: Option<String>,
    /// Present only for a write proposal. A proposal is persisted in `proposed` state and pauses
    /// the run; it never produces a tool observation until approved or denied.
    pub approval_preview: Option<CodeApprovalPreview>,
    pub loop_detected: bool,
}

#[derive(Debug, Clone)]
pub struct CodeApprovalPreview {
    pub content: String,
    pub call_hash: String,
    pub preview_hash: String,
    pub precondition_hash: String,
}

impl From<crate::code_write_tools::EditFilePreview> for CodeApprovalPreview {
    fn from(value: crate::code_write_tools::EditFilePreview) -> Self {
        Self {
            content: value.diff,
            call_hash: value.call_hash,
            preview_hash: value.preview_hash,
            precondition_hash: value.precondition_hash,
        }
    }
}

/// Hash-bound local-user authorization for one already persisted edit proposal.
pub struct ApproveCodeEdit<'a> {
    pub run_id: &'a str,
    pub invocation_id: &'a str,
    pub tool_name: &'a str,
    pub call_hash: &'a str,
    pub preview_hash: &'a str,
    pub precondition_hash: &'a str,
    pub execution_lease_id: &'a str,
    pub execution_lease_expires_at: &'a str,
    pub verification_plan_json: &'a str,
}

pub struct FinalizeCodeEdit<'a> {
    pub run_id: &'a str,
    pub invocation_id: &'a str,
    pub tool_name: &'a str,
    pub execution_lease_id: &'a str,
    pub outcome: CodeRecoveryOutcome,
    pub evidence_json: &'a str,
    pub observation_content: &'a str,
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

pub fn serialize_json<T: Serialize + ?Sized>(value: &T) -> Result<String, AppError> {
    serde_json::to_string(value).map_err(|_| {
        AppError::new(
            "code_serialization_failed",
            "Ark Code could not safely serialize durable execution data.",
        )
    })
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
