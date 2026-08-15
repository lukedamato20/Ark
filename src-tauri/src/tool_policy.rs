//! SEC-009: the capability-scope, approval, and audit-event type model every current and future
//! tool-calling feature (CMP-002/003/004, Ark Code's CODE-004/CODE-005) must consume rather than
//! each defining its own permission shape. See
//! `docs/adr/0002-tool-capability-and-prompt-injection-policy.md` for the policy this
//! implements. CMP-003's `tools.rs` is the first real consumer (a single built-in, chat-safe
//! "notes" tool) — the `#![allow(dead_code)]` this module carried while it had none has been
//! removed accordingly.

use crate::db::now;
use serde::{Deserialize, Serialize};

/// Where a capability scope may be requested from. See ADR 0002 section 2 — this is a hard
/// structural split, not a default a future UI shortcut may relax.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityTier {
    /// Usable from Ark Chat: web search, utilities, notes, memory, external-service connectors.
    ChatSafe,
    /// Filesystem write, git, process/command execution. Only grantable within an Ark Code
    /// session bound to a Repository — never reachable from Ark Chat.
    RepositoryExecution,
}

/// A tool's declared scope. `data` is a free-text description of *which* data, not just the
/// axis — "write" alone is not a usable grant.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CapabilityScope {
    pub tier: CapabilityTier,
    pub read: bool,
    pub write: bool,
    pub network: bool,
    pub secret: bool,
    /// Which data this scope actually covers (e.g. "the bound repository root", "the selected
    /// provider's API key") — required, not optional, because an axis flag alone is not a
    /// meaningful grant.
    pub data: String,
}

impl CapabilityScope {
    /// ADR 0002 section 3: write/network/secret access makes this a side effect requiring
    /// preview unless a still-valid narrow grant already covers it. `read`-only scopes are not
    /// side-effecting.
    pub fn is_side_effecting(&self) -> bool {
        self.write || self.network || self.secret
    }
}

/// A bounded, narrow, time-boxed authorization to use one tool's declared scope. There is
/// deliberately no "allow all tools" variant — every grant names exactly one tool and expires.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CapabilityGrant {
    pub tool_id: String,
    pub scope: CapabilityScope,
    pub granted_at: String,
    /// RFC3339 timestamp; a grant with no expiry is not representable by this type.
    pub expires_at: String,
    pub revoked: bool,
}

impl CapabilityGrant {
    /// `now_rfc3339` is a parameter, not read internally, so this stays testable without
    /// depending on wall-clock time.
    pub fn is_valid_at(&self, now_rfc3339: &str) -> bool {
        !self.revoked && now_rfc3339 < self.expires_at.as_str()
    }
}

/// Whether re-running a side-effecting tool call with the same inputs is safe. A tool that
/// cannot honestly declare a real policy defaults to `RequiresFreshApproval` (ADR 0002 §3) —
/// there is no `Default` impl that silently picks the more permissive variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IdempotencyPolicy {
    Idempotent,
    RequiresFreshApproval,
}

/// The human-readable preview shown before a side-effecting call runs, unless a still-valid
/// narrow grant already covers this exact action.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SideEffectPreview {
    pub tool_id: String,
    pub summary: String,
    pub idempotency: IdempotencyPolicy,
}

/// SEC-009 §4: an append-only, hash-chained audit record. Tamper-evidence, not cryptographic
/// integrity — the same "detect drift, not prove a signature" property `db::migration_checksum`
/// already relies on for the same reason (no new dependency for a local, single-user threat
/// model). `redacted_detail` must never contain a raw secret or a retrieved page's full body —
/// the same discipline already enforced for runtime logs
/// (`docs/runtime-diagnostics-policy.md`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditEvent {
    pub sequence: u64,
    pub timestamp: String,
    pub kind: AuditEventKind,
    pub tool_id: String,
    pub redacted_detail: String,
    /// FNV-1a hash of this event's own fields *and* the previous event's `chain_hash` (empty
    /// string for the first event).
    pub chain_hash: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditEventKind {
    Granted,
    Revoked,
    Invoked,
    ApprovalRequested,
    ApprovalDenied,
}

/// Same non-cryptographic FNV-1a drift-detection hash as `db::migration_checksum`, duplicated
/// rather than shared across a module boundary that otherwise has no reason to depend on `db`.
fn fnv1a(input: &str) -> String {
    const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
    const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut hash = FNV_OFFSET_BASIS;
    for byte in input.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    format!("{hash:016x}")
}

fn event_hash_input(
    previous_hash: &str,
    sequence: u64,
    timestamp: &str,
    kind: AuditEventKind,
    tool_id: &str,
    redacted_detail: &str,
) -> String {
    format!("{previous_hash}|{sequence}|{timestamp}|{kind:?}|{tool_id}|{redacted_detail}")
}

/// Builds the next event in an audit chain. `previous` is `None` only for the very first event.
pub fn next_audit_event(
    previous: Option<&AuditEvent>,
    kind: AuditEventKind,
    tool_id: &str,
    redacted_detail: &str,
) -> AuditEvent {
    let sequence = previous.map(|event| event.sequence + 1).unwrap_or(0);
    let previous_hash = previous
        .map(|event| event.chain_hash.as_str())
        .unwrap_or("");
    let timestamp = now();
    let chain_hash = fnv1a(&event_hash_input(
        previous_hash,
        sequence,
        &timestamp,
        kind,
        tool_id,
        redacted_detail,
    ));
    AuditEvent {
        sequence,
        timestamp,
        kind,
        tool_id: tool_id.to_string(),
        redacted_detail: redacted_detail.to_string(),
        chain_hash,
    }
}

/// Verifies an entire audit chain is unmodified — recomputes each event's hash from its own
/// fields plus the previous event's stored hash and confirms it matches what's stored. Detects
/// any edit, deletion, or reordering, not just a corrupted final event.
pub fn verify_audit_chain(events: &[AuditEvent]) -> bool {
    let mut previous_hash = String::new();
    for (index, event) in events.iter().enumerate() {
        if event.sequence != index as u64 {
            return false;
        }
        let expected = fnv1a(&event_hash_input(
            &previous_hash,
            event.sequence,
            &event.timestamp,
            event.kind,
            &event.tool_id,
            &event.redacted_detail,
        ));
        if expected != event.chain_hash {
            return false;
        }
        previous_hash = event.chain_hash.clone();
    }
    true
}

/// SEC-009 §2: the structural enforcement point. A repository-execution-tier scope must never
/// be usable without an active Repository binding — this is the "never reachable from Ark Chat,
/// structurally" requirement made real as a callable check, not just a comment.
/// `repository_bound` stands in for a real Ark Code Repository context (Phase 6.5, not yet
/// implemented); this function is what CODE-004/CODE-005 must call before honoring any
/// repository-execution grant.
/// Not yet called by production code: CMP-003 (`tools.rs`) only registers `ChatSafe`-tier tools,
/// so nothing in this build ever constructs a `RepositoryExecution` scope to check. This becomes
/// a real, called function the moment Phase 6.5's CODE-004/CODE-005 exist — kept here now rather
/// than deleted so that work has a tested enforcement point to call on day one, matching the
/// "define the extensible structure before its first consumer" pattern this module already used
/// for its own types before CMP-003 existed.
#[allow(dead_code)]
pub fn enforce_tier_boundary(
    scope: &CapabilityScope,
    repository_bound: bool,
) -> Result<(), String> {
    if scope.tier == CapabilityTier::RepositoryExecution && !repository_bound {
        return Err(format!(
            "capability scope \"{}\" is repository-execution tier and requires an active Repository binding",
            scope.data
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chat_safe_scope() -> CapabilityScope {
        CapabilityScope {
            tier: CapabilityTier::ChatSafe,
            read: true,
            write: false,
            network: true,
            secret: false,
            data: "web search results".to_string(),
        }
    }

    fn repository_execution_scope() -> CapabilityScope {
        CapabilityScope {
            tier: CapabilityTier::RepositoryExecution,
            read: true,
            write: true,
            network: false,
            secret: false,
            data: "the bound repository root".to_string(),
        }
    }

    #[test]
    fn chat_safe_scope_does_not_require_a_repository_binding() {
        assert!(enforce_tier_boundary(&chat_safe_scope(), false).is_ok());
        assert!(enforce_tier_boundary(&chat_safe_scope(), true).is_ok());
    }

    #[test]
    fn repository_execution_scope_is_rejected_without_a_repository_binding() {
        let error = enforce_tier_boundary(&repository_execution_scope(), false).unwrap_err();
        assert!(error.contains("repository-execution"));
        assert!(error.contains("Repository binding"));
    }

    #[test]
    fn repository_execution_scope_is_accepted_with_a_repository_binding() {
        assert!(enforce_tier_boundary(&repository_execution_scope(), true).is_ok());
    }

    #[test]
    fn read_only_scope_is_not_side_effecting() {
        let scope = CapabilityScope {
            tier: CapabilityTier::ChatSafe,
            read: true,
            write: false,
            network: false,
            secret: false,
            data: "notes".to_string(),
        };
        assert!(!scope.is_side_effecting());
    }

    #[test]
    fn write_network_or_secret_scope_is_side_effecting() {
        let base = chat_safe_scope();
        assert!(CapabilityScope {
            write: true,
            ..base.clone()
        }
        .is_side_effecting());
        assert!(CapabilityScope {
            network: true,
            write: false,
            ..base.clone()
        }
        .is_side_effecting());
        assert!(CapabilityScope {
            secret: true,
            write: false,
            network: false,
            ..base
        }
        .is_side_effecting());
    }

    #[test]
    fn capability_grant_expires() {
        let grant = CapabilityGrant {
            tool_id: "web_search".to_string(),
            scope: chat_safe_scope(),
            granted_at: "2026-08-14T00:00:00Z".to_string(),
            expires_at: "2026-08-14T00:05:00Z".to_string(),
            revoked: false,
        };
        assert!(grant.is_valid_at("2026-08-14T00:01:00Z"));
        assert!(
            !grant.is_valid_at("2026-08-14T00:10:00Z"),
            "an expired grant must not be valid"
        );
    }

    #[test]
    fn capability_grant_revocation_is_immediate_regardless_of_expiry() {
        let grant = CapabilityGrant {
            tool_id: "web_search".to_string(),
            scope: chat_safe_scope(),
            granted_at: "2026-08-14T00:00:00Z".to_string(),
            expires_at: "2026-08-14T01:00:00Z".to_string(),
            revoked: true,
        };
        assert!(
            !grant.is_valid_at("2026-08-14T00:01:00Z"),
            "a revoked grant must be invalid even while still within its expiry window"
        );
    }

    #[test]
    fn audit_chain_verifies_a_genuine_untampered_chain() {
        let first = next_audit_event(
            None,
            AuditEventKind::Granted,
            "web_search",
            "granted: web search, 5 min",
        );
        let second = next_audit_event(
            Some(&first),
            AuditEventKind::Invoked,
            "web_search",
            "query: redacted (12 chars)",
        );
        let third = next_audit_event(
            Some(&second),
            AuditEventKind::Revoked,
            "web_search",
            "revoked by user",
        );
        assert!(verify_audit_chain(&[first, second, third]));
    }

    #[test]
    fn audit_chain_detects_a_tampered_event_body() {
        let first = next_audit_event(
            None,
            AuditEventKind::Granted,
            "web_search",
            "granted: web search, 5 min",
        );
        let mut second = next_audit_event(
            Some(&first),
            AuditEventKind::Invoked,
            "web_search",
            "query: redacted",
        );
        // Tamper with the stored detail after the hash was computed — simulates an attempt to
        // rewrite what an audit event says happened without also being able to forge the chain.
        second.redacted_detail = "query: something else entirely".to_string();
        assert!(!verify_audit_chain(&[first, second]));
    }

    #[test]
    fn audit_chain_detects_a_deleted_event() {
        let first = next_audit_event(None, AuditEventKind::Granted, "web_search", "granted");
        let second = next_audit_event(
            Some(&first),
            AuditEventKind::Invoked,
            "web_search",
            "invoked",
        );
        let third = next_audit_event(
            Some(&second),
            AuditEventKind::Revoked,
            "web_search",
            "revoked",
        );
        // Removing the middle event breaks both the sequence numbering and the hash chain for
        // everything after it — an attacker cannot simply delete one inconvenient event.
        assert!(!verify_audit_chain(&[first, third]));
    }

    #[test]
    fn audit_chain_detects_reordered_events() {
        let first = next_audit_event(None, AuditEventKind::Granted, "web_search", "granted");
        let second = next_audit_event(
            Some(&first),
            AuditEventKind::Invoked,
            "web_search",
            "invoked",
        );
        assert!(!verify_audit_chain(&[second, first]));
    }
}
