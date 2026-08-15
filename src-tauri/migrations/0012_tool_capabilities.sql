-- CMP-003: the first real consumer of SEC-009's capability-scope/grant/audit type model
-- (`src-tauri/src/tool_policy.rs`, `docs/adr/0002-tool-capability-and-prompt-injection-policy.md`).
-- Scoped deliberately to one built-in, chat-safe, user-triggered tool ("notes" -- a per-conversation
-- scratch note, one of the ChatSafe-tier examples ADR 0002 itself names) rather than a real MCP
-- protocol client or LLM-autonomous tool calling -- see implementation-plan.md's CMP-003 entry for
-- why that is a separate, larger lift (no provider adapter today parses a tool-call response at
-- all; `ProviderCapabilities.tools` is `false` for every adapter per ARC-003).
--
-- `conversation_notes`: the one built-in tool's own data, scoped like `attachments` (hard
-- ownership, real cascading FK) rather than like `projects`/`personas` (soft/unconstrained).
CREATE TABLE IF NOT EXISTS conversation_notes (
    id TEXT PRIMARY KEY,
    conversation_id TEXT NOT NULL,
    content TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (conversation_id) REFERENCES conversations(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_conversation_notes_conversation ON conversation_notes(conversation_id, updated_at);

-- `capability_grants`: a persisted row per `tool_policy::CapabilityGrant`. There is deliberately no
-- "allow all tools" row shape -- every grant names exactly one tool, one scope, and one expiry.
-- `id` is the persistence key; `tool_id`/`tier`/`can_*`/`scope_data`/`granted_at`/`expires_at`/
-- `revoked` mirror `CapabilityScope`/`CapabilityGrant`'s fields directly so no lossy JSON blob is
-- needed to round-trip a grant.
CREATE TABLE IF NOT EXISTS capability_grants (
    id TEXT PRIMARY KEY,
    tool_id TEXT NOT NULL,
    tier TEXT NOT NULL,
    can_read INTEGER NOT NULL,
    can_write INTEGER NOT NULL,
    can_network INTEGER NOT NULL,
    can_secret INTEGER NOT NULL,
    scope_data TEXT NOT NULL,
    granted_at TEXT NOT NULL,
    expires_at TEXT NOT NULL,
    revoked INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_capability_grants_tool ON capability_grants(tool_id, revoked);

-- `tool_audit_events`: a single, global, append-only, hash-chained log -- persists exactly the
-- chain `tool_policy::next_audit_event`/`verify_audit_chain` already define and test. `sequence`
-- is the primary key (not a separate rowid) so the chain's own ordering invariant is the table's
-- own ordering invariant; there is no UPDATE or DELETE path anywhere in this codebase's Rust for
-- this table, matching the "an attacker cannot simply delete one inconvenient event" property
-- `verify_audit_chain`'s own tests already prove at the type level.
CREATE TABLE IF NOT EXISTS tool_audit_events (
    sequence INTEGER PRIMARY KEY,
    timestamp TEXT NOT NULL,
    kind TEXT NOT NULL,
    tool_id TEXT NOT NULL,
    redacted_detail TEXT NOT NULL,
    chain_hash TEXT NOT NULL
);
