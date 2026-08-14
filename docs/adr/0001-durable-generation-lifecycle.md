# ADR 0001: Durable generation lifecycle

- Status: Approved
- Date: 2026-08-14
- Approved by: Luke D'Amato, 2026-08-14
- Owners: Ark core and client maintainers
- Scope: desktop now; shared protocol/mobile clients must preserve these semantics
- Process note: COR-001–005 were implemented against this ADR's content before formal approval
  was recorded, deviating from the plan's original "approved before implementation" sequencing.
  The approver has explicitly reviewed and accepted this as a one-time historical exception rather
  than requiring COR-001–005 to be redone; this ADR's content is the binding contract going
  forward regardless of the order in which it was formally signed off.

## Decision

SQLite is authoritative for message existence, branch position, accumulated checkpoint content,
and terminal state. Tauri events are versioned notifications/overlays; a client that sees an
unknown schema or revision gap refetches durable state. In-memory task/cancellation records are
best-effort process controls and never the only source of a user-visible terminal result.

Ark keeps the persisted message-state set deliberately smaller than the logical execution phases:

| Logical phase | Durable message status | Why |
|---|---|---|
| queued / starting | `pending` | Provider work begins immediately after the request transaction; separate durable rows would add transitions without a current queue consumer. |
| streaming | `pending` plus checkpointed content | `chat:stream-start` and revisioned deltas provide the live overlay. A crash recovers either active form identically. |
| cancelling | no separate status | Cancellation commits `cancelled` synchronously before returning; provider/process termination remains best effort. |
| complete | `complete` | Terminal, immutable except explicit branch operations creating new messages. |
| protocol/crash interruption | `interrupted` | Terminal but recoverable through Retry, Keep partial, or Discard. |
| unrecoverable failure | `failed` | Terminal; retry creates a new append-only branch message. |
| user cancellation | `cancelled` | Terminal; retry/regenerate creates a new message. |

The smaller durable set is intentional while Ark has no queued scheduler. PERF-004 may introduce a
first-class generation entity/queue; it must migrate this contract rather than overloading message
states silently.

## Transitions, transactions, events, and retry

| From → to | Durable boundary | Event | Retry/recovery |
|---|---|---|---|
| absent → pending | Send/edit/regenerate transaction inserts every related row and moves the active branch, then commits | `chat:stream-start` only after commit | If the transaction fails, no row/event exists and the command returns a typed error. |
| pending + content | Checkpoint append transaction at ≤4 writes/sec or 8 KiB; final tail flush precedes terminal write | revisioned `chat:stream-delta` | Duplicate revisions are ignored; gaps refetch the active path. |
| pending → complete | Conditional `finish_message_if_active` update; first terminal writer wins | `chat:stream-complete` with authoritative full content | Regenerate creates a sibling assistant message; the completed row is not reopened. |
| pending → cancelled | Synchronous conditional terminal update in `cancel_stream` | one `chat:stream-cancelled` if this caller won | Repeated cancellation is a no-op; retry creates a new message. |
| pending → interrupted | Conditional terminal update after invalid/truncated/idle stream, or startup recovery | `chat:stream-interrupted`; startup recovery is returned by bootstrap/path refetch | Retry creates a sibling; Keep partial changes interrupted → complete; Discard moves the active branch without deleting provenance. |
| pending → failed | Conditional terminal update after unrecoverable provider/application error | `chat:stream-error` if this writer won | Retry/regenerate creates a sibling. |
| any terminal → active | Forbidden | none | No update statement matches a terminal row. A new immutable message ID is required. |
| imported transient → interrupted | Import validation/normalization and all inserts occur in one transaction | no stream event; import completion summary reports normalization count | User chooses the same interrupted recovery actions. |

Keep partial (`interrupted → complete`) is the sole terminal-to-terminal recovery mutation and is
explicitly not a return to an active state. Discard changes the conversation's active branch; it
does not rewrite the message's provenance.

## Event ordering and reconciliation

1. The full send/edit/regenerate transaction commits.
2. The command establishes the in-memory cancellation record before provider work can emit data.
3. `chat:stream-start` announces the durable assistant ID.
4. Delta revisions start at 1 and increase exactly once per emitted delta.
5. One conditional terminal transition wins and emits at most one matching terminal event.
6. Terminal events carry authoritative full content; clients clear their transient overlay.

Events can be delayed, duplicated, or dropped. They never authorize a transition that SQLite did
not commit. Unknown schema versions are discarded and refetched; duplicate revisions are ignored;
revision gaps or events for an unknown message invalidate/refetch the active transcript.

## Crash-point outcomes

| Crash point | Required outcome on restart |
|---|---|
| Before row transaction | No new message exists and no stream event was emitted. |
| During row transaction | SQLite rolls back every related insert/branch update. |
| After commit, before task registration/start | Assistant remains `pending`; startup recovery changes it to `interrupted`. |
| During streaming before checkpoint | At most the in-memory, not-yet-checkpointed tail is lost; durable partial content becomes `interrupted`. |
| After checkpoint / before terminal write | Checkpointed content survives and becomes `interrupted`. |
| During terminal race | Conditional update makes the first terminal state authoritative; later writers emit nothing. |
| After terminal commit / before event delivery | Terminal state/content are recovered by authoritative refetch. |

## Cancellation and provider termination

The cancel command first sets the live task's atomic flag when present, then synchronously commits
`cancelled`. The provider read loop checks the flag and stops accepting deltas. HTTP/native
termination is best effort and must be bounded; it cannot change the already-durable result.
Cancelling a missing task or a terminal message succeeds as an idempotent no-op.

## Provenance

Edit/regenerate/retry are append-only. `parent_message_id`, `revision_of_message_id`, path index,
provider/model, timestamps, content, status, and errors remain attached to the immutable attempt.
Switching/discarding changes the active path pointer rather than deleting alternate research.

## Conformance evidence

- DB tests cover transactional send/edit/regenerate/import, terminal first-writer-wins,
  cancellation idempotency/races, startup recovery, Keep partial, Discard, and branch switching.
- Provider fixtures cover terminal markers, malformed/truncated/idle outcomes and partial content.
- Frontend reconciliation tests cover duplicate/gap revisions, terminal overlay reconciliation,
  stale query responses, and unknown-message refetch.
- Rust/TypeScript contract checks version the event/status DTOs.

Changes to states, ordering, retry semantics, or event meanings require a new ADR decision, schema
migration where applicable, a stream schema-version review, and updated conformance tests.
