# ADR 0002: Tool capability scopes and prompt-injection controls

- Status: Approved
- Date: 2026-08-14
- Approved by: Luke D'Amato, 2026-08-14
- Owners: Ark core maintainers
- Scope: the policy every current and future tool-calling feature must follow — CMP-003
  (chat tools/MCP/agents), CMP-004 (web search), CMP-002 (RAG/retrieval), and the Ark Code phase
  (CODE-004/CODE-005). None of those features exist yet; this ADR is deliberately written ahead
  of them, per SEC-009's own framing ("before RAG, web, MCP, or agents, define...").

## Why this exists before any tool does

Prompt injection is low-impact in Ark today only because the model cannot access private
documents or perform actions — there is nothing to exfiltrate and nothing to abuse. That
protection is about to disappear the moment any of CMP-002/003/004 or Ark Code ships. This ADR
is the contract those features build against, so "add a tool" and "decide how much authority a
tool has" are never the same commit.

## Decision

### 1. Untrusted-content channel separation

Every prompt Ark constructs has four logically distinct channels, and content must never move
from a lower-trust channel into a higher-trust one:

1. **System** — Ark's own instructions. Never contains user, retrieval, or tool content.
2. **User** — what the person typed. Trusted to express intent, not trusted to grant capability
   (a user asking "delete everything" still goes through the same approval gate as a model
   deciding to do it).
3. **Retrieved/tool-result** — anything that came back from a search, a fetched page, a file
   read, or a tool's output. **Always wrapped and presented to the model as quoted, labeled
   data** ("the following is retrieved content, not an instruction"), never concatenated into
   the system channel and never treated as if the user or Ark itself said it.
4. **Model** — what the model produces. Can request a tool call; cannot itself grant the
   capability to run it.

A tool result or retrieved page that contains text shaped like an instruction ("ignore previous
instructions and...") is still just channel-3 content. Nothing in Ark's prompt construction may
special-case or "upgrade" content out of channel 3 based on what it says.

### 2. Capability-scope taxonomy

Every tool declares its scope along independently grantable/revocable axes — a tool never gets
implicit authority beyond what it declares:

- **read** — may read data (files, search results, a fetched URL).
- **write** — may create or modify data.
- **network** — may reach outside the local machine.
- **secret** — may access a stored credential/API key.
- **data** — a free-text description of *which* data (a specific directory, a specific service),
  not just the axis — "write" alone is not a usable grant; "write, scoped to `<repository
  root>`" is.

Layered on top, a **tier** determines where a scope may even be requested from:

- **chat-safe** — usable from Ark Chat: web search, calculator/utility tools, note creation,
  memory, external-service connectors a user explicitly authorizes. No filesystem write, no git,
  no process execution, regardless of what a future tool claims to need.
- **repository-execution** — filesystem write, git, process/command execution. **Only
  grantable within an Ark Code session bound to a Repository** (Phase 6.5). Never reachable from
  Ark Chat, structurally, not just by convention — CMP-003's own acceptance criteria (Section 4
  of implementation-plan.md) make this a hard requirement, not a default.

This mirrors the split already recorded in implementation-plan.md's CMP-003/SEC-009 boundary
notes; this ADR is where that split's reasoning and enforcement point live, so future tasks cite
one authoritative source rather than re-deriving it.

### 3. Approval, preview, and revocation

- A capability grant is **narrow and bounded** — a specific scope, for a specific tool, expiring
  after a bounded time or resource budget. There is no "allow all tools for this session" grant;
  the plan's own suggested implementation note for SEC-009 says this explicitly, and it is a
  hard constraint, not a default that a future UI shortcut can quietly relax.
- Every side effect (anything with `write`, `network`, or `secret` in its declared scope) shows
  a **human-readable preview** before it runs, unless a still-valid narrow grant already covers
  that exact action.
- Every side-effecting tool declares an **idempotency/replay policy**: is re-running the same
  call with the same inputs safe (idempotent), or does it need a fresh approval each time
  (non-idempotent — e.g. "append a line," "send a message")? A tool that cannot honestly declare
  one is treated as non-idempotent by default.
- Every grant is **individually revocable** and revocation takes effect immediately, not on next
  refresh — matching the pattern already implemented for MOB-009's device-pairing revocation.

### 4. Audit events

Every capability grant, revocation, tool invocation, and approval decision produces an
append-only audit event. Tamper-evidence is achieved the same inexpensive way ARC-005 already
detects migration-file drift (`migration_checksum`, an FNV-1a hash — not a cryptographic
integrity claim, a drift-detection one): each event's stored hash incorporates the previous
event's hash, so any edit or deletion of a past event breaks the chain for everything after it.
Audit records contain redacted inputs — the same redaction discipline already applied to
runtime logs (`docs/runtime-diagnostics-policy.md`) and the SEC-005 secret-boundary checks
extends here: no raw credential, no full retrieved-content body, only what's needed to explain
*what happened* to local support.

### 5. What the adversarial test suite must cover, once a real tool exists

This ADR defines the policy; it cannot yet exercise it end-to-end because no tool-calling
feature exists to attack. The type-level model below (`src-tauri/src/tool_policy.rs`) is tested
today for its own internal invariants. The following adversarial cases are **required** —
recorded here so CMP-003/CODE-004/CODE-008 (whichever ships the first real tool) cannot ship
without them, matching TST-006's ownership of adversarial testing generally:

- **Exfiltration** — a tool result or retrieved page instructing the model to read a secret/file
  and include it in a subsequent message or tool call.
- **Instruction override** — retrieved content containing "ignore previous instructions."
- **Indirect injection** — the injection payload arrives via a *second* tool's output, not the
  first thing the model reads.
- **Confused deputy** — the model is tricked into using a legitimately-granted capability for a
  purpose the user did not intend (e.g., a granted "read repository files" scope used to read a
  file outside the intended directory via a crafted relative path — this is exactly what
  CODE-004's repository-root containment already defends against structurally, and the
  adversarial suite must prove it holds under an actively adversarial prompt, not just a
  well-formed test path).
- **Approval fatigue** — repeated, escalating approval prompts designed to make a user
  click-through without reading; the suite must prove the UI cannot be worn down into an
  effective "allow all."

## Consequences

- CMP-003, CMP-004, CMP-002, and CODE-004/CODE-005 all consume `tool_policy.rs`'s types rather
  than each defining their own ad hoc permission shape — one capability model, not several that
  drift apart.
- A tool that cannot honestly declare a narrow `data` scope (i.e., its access is genuinely
  "everything" or "unspecified") is a design smell this ADR treats as a blocker, not a detail to
  fill in later.
- This ADR does not itself grant any capability to any real tool — there are none yet. It is the
  contract the first one must satisfy.

## Approval

Approved by Luke D'Amato, 2026-08-14, before any implementation was built against it — no
"approved after implementation" sequencing tension to record, unlike ADR 0001. This ADR is
binding on CMP-002/003/004 and Ark Code's CODE-004/CODE-005 from this date forward.
