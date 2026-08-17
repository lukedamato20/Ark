# ADR 0003: Durable Ark Code agent-run lifecycle

- Status: Approved
- Date: 2026-08-17
- Approved by: Luke D'Amato (Ark product/engineering owner), 2026-08-17
- Owners: Ark core, security, and client maintainers
- Scope: Ark Code agent runs and their provider/tool activity; Ark Chat message generation remains
  governed by ADR 0001
- Builds on: ADR 0001 (durable state is authoritative), ADR 0002 (tool scopes and approvals), and
  CODE-001 (provider-independent structured tool calls)

## Context

A crashed text generation can safely become `interrupted`. A crashed tool invocation is different:
Ark may have written a file, moved a git ref, or started a process after its durable "executing"
record committed but before its result committed. Retrying that call blindly can duplicate or
compound a real-world side effect. SQLite also cannot make a filesystem, git, or process operation
atomic with a database transaction.

This ADR defines the state and recovery contract before Ark Code gains repository tools. It does
not grant a tool capability or implement a repository operation.

## Decision

SQLite is authoritative for every run, step, proposed invocation, approval, observation, budget,
and terminal outcome. In-memory tasks, cancellation tokens, provider streams, and frontend events
are process controls and overlays only. A client seeing an unknown event version or sequence gap
refetches durable run state.

External side effects are handled as a durable intent followed by execution and independent
verification. Ark guarantees **at-most-one automatic execution attempt**, not impossible
cross-system exactly-once semantics. After a crash, Ark verifies real state and records what it can
prove before offering resume or retry. It never treats an uncertain operation as completed and
never automatically repeats one.

## Durable entities

The migration implemented with CODE-007 must preserve these logical records even if physical table
names differ:

- **Code session** — belongs to one existing Project and its optional Repository binding.
- **Agent run** — one immutable attempt with `parent_run_id` for explicit retries; selected model,
  repository identity/root snapshot, budgets, state, terminal error, cancellation request time,
  and monotonic event sequence.
- **Agent step** — one planning/model turn with reserved and actual token/cost usage and a bounded
  prompt/observation manifest. Raw secrets are never stored.
- **Tool invocation** — model call, canonical arguments, capability scope, idempotency policy,
  approval/precondition/preview hashes, execution lease, and verification result.
- **Observation** — bounded/redacted result or error plus content hashes and provenance needed to
  reproduce the context decision.
- **Run event** — append-only, sequenced projection notification. Events do not replace the rows
  above and do not authorize work.

Every mutating command carries a client idempotency key. Uniqueness is scoped to the operation and
run; reusing a key with a different request hash is a conflict.

## Run states

| State | Durable meaning |
|---|---|
| `queued` | Run exists and owns snapshotted budgets/repository identity; no provider or tool work has started. |
| `planning` | A model turn has been durably reserved and may be in flight. No requested tool is authorized by this state. |
| `awaiting_approval` | One exact, persisted invocation and human-readable preview await a decision. Nothing is executing. |
| `executing_tool` | Intent, preconditions, and verification plan committed before the external operation began. On startup this state means recovery verification is required, not permission to rerun. |
| `observing` | A tool outcome is durably recorded and is ready to enter the next model turn. No external operation is active. |
| `completed` | Terminal success with a durable final response/summary. |
| `failed` | Terminal, known failure. No uncertain side effect remains. |
| `cancelled` | Terminal cancellation after Ark proved no invocation remains uncertain. |
| `interrupted` | Terminal recoverable stop caused by crash/lifecycle interruption or an outcome Ark could not prove. The recorded recovery reason distinguishes applied, not-applied, diverged, and unknown effects. |

Terminal states are never reopened. Resume/retry creates a new run with `parent_run_id`, while the
original attempt and its audit trail stay immutable. A session may display the attempts as one
thread, but storage and budgets never blur them together.

## State transitions and transaction boundaries

No database transaction is held across provider, filesystem, git, or process I/O.

| From → to | Transaction boundary | Idempotency, cancel, and retry behavior |
|---|---|---|
| absent → `queued` | Insert run, repository snapshot, hard budgets, first event, and request receipt in one transaction. | Same key/request returns the run; key reuse conflicts. Cancel conditionally commits `cancelled`. |
| `queued` → `planning` | Conditional claim records executor lease, step, prompt manifest, and worst-case token/cost reservation before provider dispatch. | One claimant wins. Cancel before dispatch commits `cancelled`; after dispatch it records a request and the provider future is dropped, then the run becomes `interrupted` unless no response work began can be proved. |
| `planning` → `awaiting_approval` | Persist model output, exact proposed invocation, canonical arguments, scope, preview hash, repository preconditions, and event together. | Duplicate provider/event delivery cannot create a second invocation. Cancel invalidates the proposal and commits `cancelled`. Retry is a new run. |
| `planning` → `executing_tool` | Allowed only for a non-side-effecting invocation already covered by policy. Persist the invocation first; a second transaction conditionally records its execution intent/lease. | Read-only does not mean replayed blindly after a crash; startup still creates a fresh invocation or records interruption. |
| `planning` → `completed` | Persist final response, reconcile budget reservation, set terminal state, and append terminal event atomically. | First terminal writer wins. Repeated completion is a no-op. |
| `planning` → `failed` / `interrupted` | Persist bounded error, reconcile/retain conservative budget reservation, set terminal, append event. | Known pre-dispatch failure is `failed`; dispatched-but-unconfirmed provider work is `interrupted`. |
| `awaiting_approval` → `executing_tool` | Approval transaction binds approver/time to invocation id, canonical call hash, preview hash, repository identity, precondition hash, and expiry. A separate conditional transaction records execution intent before I/O. | Approval is single-use and cannot be transferred to changed arguments/state. Preconditions are rechecked immediately before intent commit. Rejection becomes a durable denied observation or explicit cancellation; it never grants a broader scope. |
| `executing_tool` → `observing` | After external I/O, verify actual state; atomically store the verified outcome/observation, finalize invocation, reconcile budgets, and append event. | A duplicate completion sees a terminal invocation and performs no I/O. If cancellation was requested, observation is retained before transitioning to `cancelled`. |
| `executing_tool` → `failed` | Only when Ark proves the operation did not apply and the failure is known. Store proof/error and terminal event atomically. | A retry is a new run/invocation and needs whatever approval policy currently requires. |
| `executing_tool` → `cancelled` | Only after termination plus verification proves no side effect applied. | Repeated cancel is a no-op. A kill signal alone is not proof. |
| `executing_tool` → `interrupted` | Store verifier outcome `applied`, `not_applied`, `diverged`, or `unknown`, bounded evidence, and terminal event. | Never auto-runs the invocation again. Resume is offered only from the proven state and may require fresh approval. |
| `observing` → `planning` | Persist the selected observation manifest and next step/budget reservation before model dispatch. | Duplicate starts lose the conditional transition. Cancel commits `cancelled` because no side effect is active. |
| `observing` → `completed` / `failed` / `cancelled` / `interrupted` | Persist final response/reason and terminal event atomically. | First terminal writer wins; no terminal state reopens. |
| any nonterminal → terminal | Conditional update requires the expected current state and sequence. | Concurrent cancel/failure/completion has one durable winner. |
| any terminal → nonterminal | Forbidden. | Explicit retry creates a child run. |

An approval wait does not keep an execution lease. Approval expiry, repository binding changes, or
precondition drift invalidates the proposal before execution and requires a new preview.

## External-operation protocol

Every tool invocation follows these checkpoints:

1. Validate the tool, repository binding, capability grant, canonical arguments, bounds, and
   current preconditions without side effects.
2. Persist the proposed call and human preview.
3. Obtain an exact approval when policy requires it.
4. Revalidate repository identity and preconditions.
5. Commit `executing_tool` plus an operation-specific verification plan and execution lease.
6. Perform at most one automatic external execution attempt.
7. Independently inspect actual state.
8. Commit a verified observation or an explicit uncertain outcome.

The verification plan is mandatory before step 6. A tool that cannot say how its effect will be
verified is non-idempotent and cannot be automatically resumed.

### Verification requirements

| Tool class | Intent/precondition evidence | Startup/post-I/O verification |
|---|---|---|
| File read/list/search | Canonical repository identity/root, canonical relative path/query, bounds. | No side effect to prove; stale in-flight work becomes interrupted and a resume creates a fresh read. |
| Atomic file edit | File identity, before hash/metadata, expected after hash, staged-temp identity, exact diff hash. | Expected after hash = applied; before hash = not applied; any other content/identity = diverged. Never overwrite the diverged file. |
| Git checkpoint/ref update | Repository identity, starting HEAD/ref, index/worktree fingerprints, dedicated Ark branch, intended commit/tree/ref. | Inspect refs, commit/tree, index, and working tree. Applied/not-applied/diverged must be explicit; never reset or move the user's branch to force a match. |
| Allowlisted command | Executable/argv allowlist identity, stripped-environment hash, cwd/repository identity, declared timeout/output limits, process identity when available. | Killing/waiting for a PID is not proof of every side effect. Unless a tool-specific verifier proves its declared outputs, crash or lost process state is `unknown` and requires fresh user review/approval; it is never auto-rerun. |

The verifier itself is read-only, bounded, repository-contained, and safe to repeat. Its evidence is
stored as hashes/identifiers and a redacted summary, not full private file or command output.

## Startup recovery

Recovery acquires a per-run lease with compare-and-swap; only one process may recover a run.

- `queued`: remains/re-enters the queue; no work has started.
- `awaiting_approval`: remains pending only if the repository binding and proposal still validate.
  No approval is synthesized. Expired grants are not revived.
- `planning`: becomes `interrupted`; Ark cannot know whether a remote provider billed/completed the
  abandoned request and does not dispatch it again automatically.
- `observing`: the observation is already durable, so the run becomes `interrupted` and can offer a
  user-controlled child resume without repeating the tool.
- `executing_tool`: remains visibly in a recovery-required form of that state while the persisted
  operation-specific verifier inspects filesystem/git/process reality. Only after verification does
  one transaction record `applied`, `not_applied`, `diverged`, or `unknown` and move the run to
  `interrupted` (or `cancelled` when a pending cancellation plus proved no-effect permits it).
- terminal states: unchanged.

The UI must say what Ark proved: for example, "edit applied before interruption," "edit did not
apply," or "command outcome unknown." A generic "interrupted" label alone is insufficient for an
uncertain side effect. No startup path silently reports completion.

## Cancellation

Cancellation first commits `cancel_requested_at` and signals in-memory work. For queued,
approval-waiting, or observing runs it can synchronously reach `cancelled`. For planning, provider
work is dropped/bounded and the run is conservatively interrupted if dispatch may have occurred.
For `executing_tool`, Ark requests termination but does not acknowledge terminal cancellation until
verification establishes the external outcome. A timeout follows the same path as cancellation;
"kill requested" is not a terminal result.

Repeated cancellation returns the current durable state. Cancellation never deletes partial
observations, approvals, tool audit events, or repository evidence.

## Runaway and budget controls

Every run snapshots immutable hard limits at creation; approval and resume cannot reset them.

- **Step limit:** check and reserve the next step transactionally before planning. Exhaustion fails
  with `agent_step_budget_exhausted` before another provider/tool call.
- **Active wall-clock limit:** persisted active elapsed time plus a monotonic in-process clock;
  time waiting for a person in `awaiting_approval` is excluded. The earliest conservative deadline
  wins after clock anomalies or restart. Exhaustion enters the cancellation/verification path.
- **Token limit:** reserve the serialized input allocation plus maximum possible output before each
  provider dispatch. Reconcile only from trustworthy provider usage; if usage is absent, retain the
  conservative reservation. CODE-001 fallback repair attempts count as separate usage.
- **Cost limit:** when a selected paid provider has reviewed price metadata, reserve worst-case cost
  before dispatch and reconcile with actual usage. If monetary cost is unavailable, Ark must show
  that fact and rely on the mandatory token limit; it may not display a fictitious monetary total.
  A user policy requiring a hard monetary cap blocks such a run before dispatch.
- **Loop detection:** hash normalized tool name, canonical arguments, repository-state fingerprint,
  and resulting observation. Three identical call attempts with no new durable observation fail with
  `agent_loop_detected`; approval cannot bypass or reset the detector.

Limit values are visible/editable only within product-owned safe ranges defined with the CODE-007
implementation. Lower user limits are allowed; no UI or imported session may exceed hard ceilings.
Terminal reasons distinguish user cancellation, timeout, step/token/cost exhaustion, and loop
detection.

## Events and reconciliation

Each committed transition appends a sequenced, schema-versioned event in the same transaction.
Frontend events may notify that new durable events exist, but clients render authoritative rows and
refetch on a gap. Tool output/model output is untrusted content and never grants approval. Event and
diagnostic payloads are bounded/redacted and do not contain secrets or whole repository files.

## Security consequences

- All path resolution uses the run's snapshotted Repository binding and CODE-003 containment rules.
- Project repository changes interrupt active runs; they never retarget an existing run.
- Approval binds exact intent and preconditions, not a tool name in general.
- ADR 0002 capability, preview, grant, audit, and prompt-channel rules remain mandatory.
- Read-only recovery verification cannot call the model or execute a tool.
- No generic shell, filesystem escape, destructive git reset, or user-branch mutation is introduced
  by this lifecycle.

## Required conformance evidence

Before CODE-004/005 may be called complete, tests must cover every transition and conditional race;
duplicate create/approve/start/complete/cancel; crash injection before/after each transaction and
external operation; all file/git verifier outcomes; command unknown-outcome recovery; startup lease
races; repository/precondition drift; budget boundaries; loop detection; event gaps; and proof that
no recovery path performs a second external execution attempt.

Changes to states, recovery proof, idempotency, approval binding, or budget semantics require a new
ADR decision and matching migration/protocol/test review.

## Approval

Approved by Luke D'Amato, Ark product/engineering owner, on 2026-08-17. The reviewer and date are
recorded both here and in `implementation-plan.md`; CODE-004 and CODE-005 may consume this lifecycle
contract without a further approval gate.
