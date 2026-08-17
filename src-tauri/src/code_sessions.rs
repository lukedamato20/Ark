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
    pub repository_path_snapshot: &'a str,
    pub repository_identity_hash: &'a str,
    pub max_steps: u32,
    pub max_active_ms: u64,
    pub max_tokens: u64,
    pub max_cost_microunits: Option<u64>,
    pub idempotency_key: &'a str,
    pub request_hash: &'a str,
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

pub fn request_hash<T: Serialize>(request: &T) -> Result<String, AppError> {
    let bytes = serde_json::to_vec(request).map_err(|_| {
        AppError::new(
            "code_request_serialization_failed",
            "Ark Code could not safely fingerprint this request.",
        )
    })?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

#[cfg(test)]
mod tests {
    use super::*;

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
