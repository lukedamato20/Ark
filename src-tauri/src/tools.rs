//! CMP-003: the first real tool-calling feature built on SEC-009's `tool_policy` contract. Scoped
//! deliberately to one built-in, user-triggered, chat-safe tool ("notes" — a per-conversation
//! scratch note, one of ADR 0002's own ChatSafe-tier examples) rather than a real MCP protocol
//! client, external tool discovery, or LLM-autonomous tool calling — see this feature's status
//! entry in `implementation-plan.md` for why those are a separate, larger lift. What this module
//! proves end to end is the safety mechanics every future tool must go through: a declared scope,
//! independently grantable/revocable capability, a human-readable preview before any side effect,
//! and a persisted tamper-evident audit trail.
//!
//! Read access (`list_conversation_notes`) is never gated — SEC-009's own model treats read-only
//! scopes as not side-effecting (`CapabilityScope::is_side_effecting`), so listing notes needs no
//! grant and produces no audit event. Every write (create/update/delete) requires a currently
//! valid grant; without one, the caller must request a preview and resubmit with `approve: true`,
//! mirroring the same "attempt, get a typed error, re-attempt with an explicit acknowledgement"
//! shape SEC-001's `acknowledge_remote_risk` already established for a different kind of risky
//! action.

use crate::db::{now, Database};
use crate::errors::AppError;
use crate::tool_policy::{CapabilityScope, CapabilityTier, IdempotencyPolicy, SideEffectPreview};
use serde::{Deserialize, Serialize};

pub const NOTES_TOOL_ID: &str = "notes";
/// CMP-004: the second built-in tool. Its scope's `data` field names exactly what leaves the
/// device (the query text) and what comes back (result titles/URLs/snippets) — `network: true`
/// and `secret: true` both independently make it side-effecting per `is_side_effecting()`, so it
/// goes through the same preview/grant gate as a write.
pub const WEB_SEARCH_TOOL_ID: &str = "web_search";

/// A short-lived grant automatically created the moment a user approves a previewed write — kept
/// deliberately narrow (ADR 0002 §3: "narrow, time-boxed grants only") rather than matching the
/// longer, explicit ceiling `validate_grant_ttl_minutes` allows for a proactive Settings-panel
/// grant.
pub const AUTO_APPROVAL_GRANT_TTL_MINUTES: i64 = 5;

/// A tool's declared identity and scope, shown before any grant exists — the "install/connect
/// shows publisher/source, scopes, data access, trust status" acceptance criterion, applied to a
/// built-in tool rather than an externally discovered one.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolDefinition {
    pub id: String,
    pub name: String,
    pub description: String,
    /// Trust/source disclosure. Always `"Ark (built-in)"` today — there is no external tool
    /// source yet, but the field exists now so a real MCP server's publisher has somewhere
    /// accurate to be shown later without a shape change.
    pub publisher: String,
    pub scope: CapabilityScope,
}

/// The built-in tools this build ships. A future MCP client would extend this list from
/// discovered servers rather than replace it.
pub fn built_in_tools() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            id: NOTES_TOOL_ID.to_string(),
            name: "Notes".to_string(),
            description: "Read and write a short scratch note attached to this conversation."
                .to_string(),
            publisher: "Ark (built-in)".to_string(),
            scope: CapabilityScope {
                tier: CapabilityTier::ChatSafe,
                read: true,
                write: true,
                network: false,
                secret: false,
                data: "This conversation's own notes".to_string(),
            },
        },
        ToolDefinition {
            id: WEB_SEARCH_TOOL_ID.to_string(),
            name: "Web Search".to_string(),
            description: "Search the web via Brave Search and bring back cited results."
                .to_string(),
            publisher: "Brave Search (via Ark)".to_string(),
            scope: CapabilityScope {
                tier: CapabilityTier::ChatSafe,
                read: true,
                write: false,
                network: true,
                secret: true,
                data: "Search query text sent to Brave Search API; result titles/URLs/snippets returned"
                    .to_string(),
            },
        },
    ]
}

/// A persisted `tool_policy::CapabilityGrant` — adds the row's own `id` (the abstract type has
/// none; a grant is identified by `tool_id` alone in-memory, but a persisted grant needs a
/// primary key distinct from `tool_id` since a tool can be granted, expire, and be re-granted
/// many times over its lifetime).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolCapabilityGrant {
    pub id: String,
    pub tool_id: String,
    pub tier: CapabilityTier,
    pub read: bool,
    pub write: bool,
    pub network: bool,
    pub secret: bool,
    pub data: String,
    pub granted_at: String,
    pub expires_at: String,
    pub revoked: bool,
}

impl ToolCapabilityGrant {
    pub fn scope(&self) -> CapabilityScope {
        CapabilityScope {
            tier: self.tier,
            read: self.read,
            write: self.write,
            network: self.network,
            secret: self.secret,
            data: self.data.clone(),
        }
    }

    /// Delegates to `tool_policy::CapabilityGrant::is_valid_at` rather than reimplementing the
    /// expiry/revocation check — this type is a persistence wrapper around that one, not a
    /// parallel definition of what "valid" means.
    pub fn is_valid_at(&self, now_rfc3339: &str) -> bool {
        crate::tool_policy::CapabilityGrant {
            tool_id: self.tool_id.clone(),
            scope: self.scope(),
            granted_at: self.granted_at.clone(),
            expires_at: self.expires_at.clone(),
            revoked: self.revoked,
        }
        .is_valid_at(now_rfc3339)
    }
}

/// A tool's current status for the frontend's Tools panel: its declared definition plus whichever
/// grant (if any) currently governs it, valid or not — the UI needs to show an expired/revoked
/// grant too, not just hide it, so a user can see *why* the next write will ask for approval
/// again.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolStatus {
    pub definition: ToolDefinition,
    pub active_grant: Option<ToolCapabilityGrant>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationNote {
    pub id: String,
    pub conversation_id: String,
    pub content: String,
    pub created_at: String,
    pub updated_at: String,
}

/// Which notes write is being previewed/attempted — kept as its own small enum rather than a free
/// string so an invalid action can never reach the database layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NoteWriteAction {
    Create,
    Update,
    Delete,
}

/// Builds the human-readable preview shown before a notes write runs. None of the three actions
/// can honestly claim to be safely re-runnable with the same effect (create makes a new note each
/// time; update overwrites; delete removes) — ADR 0002 §3's documented default,
/// `RequiresFreshApproval`, is the genuinely correct answer for all three, not a placeholder.
pub fn preview_note_write(action: NoteWriteAction, content: Option<&str>) -> SideEffectPreview {
    let summary = match action {
        NoteWriteAction::Create => format!(
            "Create a new note in this conversation: \"{}\"",
            truncate_for_preview(content.unwrap_or_default())
        ),
        NoteWriteAction::Update => format!(
            "Replace this note's content with: \"{}\"",
            truncate_for_preview(content.unwrap_or_default())
        ),
        NoteWriteAction::Delete => "Delete this note permanently".to_string(),
    };
    SideEffectPreview {
        tool_id: NOTES_TOOL_ID.to_string(),
        summary,
        idempotency: IdempotencyPolicy::RequiresFreshApproval,
    }
}

const PREVIEW_CHARS: usize = 80;

fn truncate_for_preview(content: &str) -> String {
    let trimmed = content.trim();
    if trimmed.chars().count() <= PREVIEW_CHARS {
        return trimmed.to_string();
    }
    let truncated: String = trimmed.chars().take(PREVIEW_CHARS).collect();
    format!("{truncated}…")
}

/// The result of attempting a tool invocation: either it ran (with the outcome), or it was
/// blocked pending approval. This is deliberately an in-process return type, not a wire type —
/// the Tauri command layer (`commands::mod`) turns the blocked case into a typed `AppError`
/// ("approval required") rather than sending a tagged union across IPC for what is, from the
/// frontend's perspective, the same "attempt, get an error, resubmit with acknowledgement" shape
/// SEC-001's `acknowledge_remote_risk` already uses.
#[derive(Debug)]
pub enum ToolInvocationAttempt {
    Applied,
    ApprovalRequired,
}

/// CMP-004: `NoteWriteAttempt` predates the second tool consumer; kept as an alias rather than
/// renamed at every existing Notes call site, minimizing blast radius on already-shipped code.
pub type NoteWriteAttempt = ToolInvocationAttempt;

/// Checks whether `tool_id` currently has a valid grant; if not and `approve` is `false`, tells
/// the caller approval is required without performing any side effect or creating any grant. If
/// `approve` is `true` and no valid grant exists, creates one (`AUTO_APPROVAL_GRANT_TTL_MINUTES`,
/// recording a `Granted` audit event) before proceeding. Returns `Applied` either way once the
/// call may proceed — the caller still must record its own `Invoked` audit event once it has
/// actually performed the action (this function only decides *whether* it may proceed, not what
/// it does). `tool_id` must be one `built_in_tools()` actually registers; an unknown id is a
/// genuinely reachable case now that it's a runtime parameter (a stale/forged id from IPC), so it
/// returns a typed "not found" error rather than panicking.
pub fn authorize_tool_invocation(
    db: &Database,
    tool_id: &str,
    approve: bool,
) -> Result<ToolInvocationAttempt, AppError> {
    let now_ts = now();
    if let Some(grant) = db.get_active_grant_for_tool(tool_id)? {
        if grant.is_valid_at(&now_ts) {
            return Ok(ToolInvocationAttempt::Applied);
        }
    }
    if !approve {
        return Ok(ToolInvocationAttempt::ApprovalRequired);
    }
    let scope = built_in_tools()
        .into_iter()
        .find(|tool| tool.id == tool_id)
        .ok_or_else(|| AppError::not_found("Tool"))?
        .scope;
    db.create_capability_grant(tool_id, &scope, AUTO_APPROVAL_GRANT_TTL_MINUTES)?;
    Ok(ToolInvocationAttempt::Applied)
}

/// Notes-specific convenience wrapper — kept so existing call sites read the same as before.
pub fn authorize_note_write(db: &Database, approve: bool) -> Result<NoteWriteAttempt, AppError> {
    authorize_tool_invocation(db, NOTES_TOOL_ID, approve)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn built_in_tools_includes_exactly_two_chat_safe_tools() {
        let tools = built_in_tools();
        assert_eq!(tools.len(), 2);
        assert!(tools
            .iter()
            .all(|tool| tool.scope.tier == CapabilityTier::ChatSafe));
    }

    #[test]
    fn notes_tool_is_read_write_with_no_network_or_secret_axis() {
        let tools = built_in_tools();
        let notes = tools.iter().find(|tool| tool.id == NOTES_TOOL_ID).unwrap();
        assert!(notes.scope.read && notes.scope.write);
        assert!(!notes.scope.network && !notes.scope.secret);
    }

    #[test]
    fn web_search_tool_is_read_only_with_network_and_secret_axes() {
        let tools = built_in_tools();
        let web_search = tools
            .iter()
            .find(|tool| tool.id == WEB_SEARCH_TOOL_ID)
            .unwrap();
        assert!(web_search.scope.read && !web_search.scope.write);
        assert!(web_search.scope.network && web_search.scope.secret);
    }

    #[test]
    fn preview_note_write_truncates_long_content_and_names_the_action() {
        let long = "a".repeat(200);
        let preview = preview_note_write(NoteWriteAction::Create, Some(&long));
        assert!(preview.summary.contains('…'));
        assert!(preview.summary.chars().count() < long.chars().count());
        assert_eq!(
            preview.idempotency,
            IdempotencyPolicy::RequiresFreshApproval
        );

        let delete_preview = preview_note_write(NoteWriteAction::Delete, None);
        assert!(delete_preview.summary.contains("Delete"));
    }

    #[test]
    fn preview_note_write_short_content_is_not_truncated() {
        let preview = preview_note_write(NoteWriteAction::Update, Some("short note"));
        assert!(preview.summary.contains("short note"));
        assert!(!preview.summary.contains('…'));
    }

    fn test_db() -> (Database, std::path::PathBuf) {
        let path =
            std::env::temp_dir().join(format!("ark-tools-test-{}.sqlite3", uuid::Uuid::new_v4()));
        (Database::open(&path).expect("database opens"), path)
    }

    /// CMP-004: `authorize_note_write`/`authorize_tool_invocation` previously had no direct test
    /// of any kind — this is the first, parametrized over both built-in tools so the
    /// generalization is proven correct for the tool it already shipped for, not just the new one.
    #[test]
    fn authorize_tool_invocation_grants_and_reuses_a_valid_grant() {
        for tool_id in [NOTES_TOOL_ID, WEB_SEARCH_TOOL_ID] {
            let (db, path) = test_db();

            assert!(matches!(
                authorize_tool_invocation(&db, tool_id, false).unwrap(),
                ToolInvocationAttempt::ApprovalRequired
            ));

            assert!(matches!(
                authorize_tool_invocation(&db, tool_id, true).unwrap(),
                ToolInvocationAttempt::Applied
            ));

            // A second attempt, still within the auto-approval TTL, reuses the existing grant
            // without requiring approval again.
            assert!(matches!(
                authorize_tool_invocation(&db, tool_id, false).unwrap(),
                ToolInvocationAttempt::Applied
            ));

            drop(db);
            let _ = std::fs::remove_file(path);
        }
    }

    #[test]
    fn authorize_tool_invocation_rejects_an_unknown_tool_id() {
        let (db, path) = test_db();
        let error = authorize_tool_invocation(&db, "not_a_real_tool", true).unwrap_err();
        assert_eq!(error.code, "not_found");
        drop(db);
        let _ = std::fs::remove_file(path);
    }
}
