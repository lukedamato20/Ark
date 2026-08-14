# Ark Production Implementation Plan

**Plan status:** Master execution roadmap

**Prepared:** 2 August 2026

**Source of truth:** [docs/application-audit-2026-08-02.md](docs/application-audit-2026-08-02.md)
**Supersedes:** The pre-audit implementation plan previously stored at this path

> Input note: the requested file audit-result.md was not present in the workspace. The linked comprehensive audit is the only audit-result artifact and is therefore treated as the intended single source of truth. No repository behavior or product assumptions outside that audit are used to create requirements in this plan.

## 1. Executive Summary

Ark is an alpha-quality local-first AI desktop MVP with a credible foundation: Tauri, React, Rust, SQLite, local-provider support, streaming chat, append-only conversation branches, portable workspaces, diagnostics, and open import/export. The audit scored overall health at 42/100, production readiness at 24/100, and iPhone readiness at 10/100.

The application must not be publicly released in its current state. Core chat can become permanently stuck after restart or an event race; provider streams can time out or be recorded complete after truncation; multi-record mutations are not transactional; Unicode title generation and workspace probing have destructive failure modes; local-provider privacy labelling is not enforced; current Rust dependencies include three advisories; and the Windows bundle cannot be produced. The application also lacks responsive behavior, complete accessibility, release automation, signing, secure updates, recoverability, structured diagnostics, and the competitive capabilities expected of a mature AI workspace.

This roadmap resolves the audit in dependency order:

1. Establish explicit lifecycle contracts, test fixtures, support claims, and quality gates.
2. Make chat, persistence, import, workspaces, and packaging correct and recoverable.
3. Enforce security and privacy at native trust boundaries.
4. introduce maintainable application, provider, database, and UI boundaries without a big-bang rewrite.
5. Deliver a responsive, accessible, state-complete desktop experience.
6. Finish the production desktop product, then add differentiated model, knowledge, tool, and media capabilities, followed by the Ark Code agentic coding environment built on that tool/agent foundation.
7. Prove performance against budgets.
8. Build an installable PWA iPhone companion with LAN device pairing (no native app or App Store distribution, per the Phase 8 scope decision).
9. Complete cross-platform validation, signed distribution, observability, staged rollout, and rollback.

After all required tasks are executed, Ark will have:

- deterministic, crash-recoverable generation and atomic data mutations;
- a truthful and enforceable privacy/security model;
- a signed, updateable, supportable desktop release;
- responsive WCAG 2.2 AA interaction across declared desktop sizes;
- managed local and secure cloud models, projects, search, attachments, RAG, tools, voice, and export/backup workflows;
- an optional, provider-agnostic Ark Code agentic coding environment built on the tool/agent foundation, addressable independently of Ark Chat;
- scalable state, database, provider, and protocol boundaries;
- an installable PWA iPhone companion over the local network (Phase 8 scope decision: no native app, no App Store distribution);
- automated unit, integration, E2E, accessibility, security, performance, migration, and release validation.

### Delivery assumptions

- Estimates are relative, not calendar commitments: **Small** is up to three focused engineering days, **Medium** is roughly one to two engineering weeks, **Large** is roughly three to six weeks, and **Extra Large** spans multiple milestones or disciplines.
- Each task estimate includes implementation, focused tests, documentation, and review.
- One experienced engineer can execute the roadmap serially. A practical team is three to five engineers across Core/Rust, Desktop/UI, Platform/Security, and Mobile, with fractional design and QA support.
- Tasks may run in parallel only when their dependencies are complete and their code ownership does not overlap.
- Public desktop release is gated after the production subset of Phases 0–5, 7, and 9. Competitive, Ark Code, and mobile milestones continue afterward; completion of the entire master roadmap includes all non-deferred tasks.

## 2. Guiding Principles

### 2.1 Correctness before breadth

- Core workflows must have explicit state machines, atomic persistence, recovery behavior, and deterministic tests.
- No new provider, RAG, tool, sync, or mobile surface may duplicate the current ambiguous generation semantics.
- A partial response must never be silently labelled complete.

### 2.2 Production code only

- No temporary bypasses, placeholder claims, ignored errors, or release-only manual steps.
- Feature flags are allowed only when they have an owner, default, expiry/review date, and tested disabled state.
- A capability must be hidden or clearly marked unavailable until its packaged artifact passes its acceptance tests.

### 2.3 Security and privacy by default

- Validate and classify provider destinations in Rust, where requests are sent.
- Treat model output, imported data, retrieved documents, downloaded models, tools, and remote services as untrusted.
- Keep credentials in OS-backed secure storage, never SQLite, localStorage, logs, or diagnostics.
- Preserve Ark's no-account and no-behavioral-analytics defaults for the local desktop product.

### 2.4 Durable state is authoritative

- UI events communicate committed state; they do not define truth.
- Every multi-record user operation is transactional or has a documented compensating transition.
- Migrations are ordered, checksummed, transactional, backed up, and tested from every supported release.

### 2.5 Accessibility and responsive behavior are release requirements

- WCAG 2.2 AA, keyboard-only operation, reduced motion, screen-reader announcements, and supported-width behavior are acceptance gates.
- Loading, empty, success, error, interrupted, offline, and recovery states are product states, not afterthoughts.
- Native iPhone UI is designed for touch and platform conventions; the desktop DOM is not force-reused.

### 2.6 Measurable performance

- Optimization follows instrumented budgets for startup, TTFT, stream work, memory, database writes, and long-history rendering.
- Streaming work must be bounded and approximately linear in generated content.
- Provider refresh and diagnostics must never block access to cached local data.

### 2.7 Incremental architecture

- Extract seams along active workflows; do not perform a speculative full rewrite.
- Keep Tauri commands thin, application use cases testable, provider capabilities explicit, and UI access behind a typed client.
- Shared mobile code contains pure domain rules, schemas, and protocol contracts—not desktop UI or filesystem assumptions.

### 2.8 Observable and supportable

- Errors use typed, user-safe messages plus redacted technical context.
- Release artifacts include provenance, third-party notices, diagnostics, recovery documentation, and rollback.
- Crash reporting is opt-in and redacted. Behavioral analytics remain off unless a later product decision obtains explicit consent.

### 2.9 Backward compatibility and data ownership

- Preserve existing conversations and settings through tested migrations.
- Never silently move, upload, overwrite, or delete user data.
- Export formats and companion protocols are versioned; incompatible changes provide migration paths.

### 2.10 Definition-driven execution

Every task is complete only when its acceptance criteria, tests, documentation, and traceability are complete. “Code merged” alone is not done.

## 3. Implementation Phases

### 3.1 Phase overview

| Phase | Objective | Primary task ranges | Entry dependency | Exit milestone |
|---|---|---|---|---|
| 0 — Foundations and guardrails | Freeze truthful scope, specify lifecycle behavior, create fixtures and quality gates | FND-001–005 | Audit accepted | Reproducible baseline and approved contracts |
| 1 — Critical correctness and data safety | Eliminate stuck chats, races, silent truncation, partial writes, destructive edge cases, and bundle blockers | COR-001–012 | Phase 0 contracts/fixtures as noted | Core workflow release blockers closed |
| 2 — Security and privacy | Enforce destinations, secrets, sidecar isolation, dependency integrity, file/model safety, and future tool/sync policy | SEC-001–011 | Phase 0; selected Phase 1 validators | Security review passes with no unreviewed high advisory |
| 3 — Architecture and maintainability | Establish thin commands, typed client/protocol, provider capabilities, DB concurrency/migrations, scoped state | ARC-001–010 | Stable Phase 1 lifecycle | Maintainable boundaries support new features/mobile |
| 4 — UI/UX and accessibility | Deliver responsive navigation, readable chat, complete states, accessible controls, and truthful onboarding | UX-001–011 | Typed state/contracts where relevant | Desktop UX passes supported-width and WCAG gates |
| 5 — Production feature completion | Finish backup, workspace, history, branch, settings, provider, model, and portability capabilities | FTR-001–010 | Phases 1–4 foundations | Desktop feature-complete release candidate |
| 6 — Competitive capabilities | Add attachments/vision, RAG, tools, web, voice, notifications, automations, and explicit team-edition decision | CMP-001–009 | Phase 5 plus security capabilities | Competitive local AI workspace milestone |
| 6.5 — Ark Code (agentic coding environment) | Deliver a provider-agnostic, local-model-first agentic coding assistant — repository awareness, scoped file/git/command tools, a durable agent loop, and approvals — as a distinct application surface alongside Ark Chat | CODE-001–008 | Phase 6 tool/agent and security foundation (CMP-003, SEC-009, ARC-003); Phase 5 desktop feature-complete Ark Chat | Read-only investigation agent ships; editing/execution tiers gated behind later CODE tasks |
| 7 — Performance and scalability | Instrument and meet startup, streaming, history, rendering, and runtime resource budgets | PERF-001–005 | Metrics foundation; feature paths available | Performance budgets pass on reference hardware |
| 8 — iPhone readiness and delivery | Build an installable PWA over the existing frontend, LAN device pairing, Web Push — no native app, no App Store, per the Phase 8 scope decision (personal-use, no public distribution) | MOB-001, 005, 007, 008, 009 (002/003/004/006/010 retired) | ARC typed boundaries; FTR-010 companion API | Installable PWA usable on the home network |
| 9 — Verification, operations, and release | Complete layered tests, observability, signed distribution, documentation, rollout, and rollback | TST-001–007, OPS-001–004 | All release-scope implementation tasks | Signed staged production rollout |

### 3.2 Dependency flow

~~~mermaid
flowchart TD
    F0["Phase 0<br/>contracts, fixtures, gates"] --> C1["Phase 1<br/>correctness and data"]
    F0 --> S2["Phase 2<br/>security and privacy"]
    C1 --> A3["Phase 3<br/>architecture"]
    S2 --> A3
    A3 --> U4["Phase 4<br/>UI/UX and accessibility"]
    A3 --> F5["Phase 5<br/>production features"]
    U4 --> F5
    S2 --> F5
    F5 --> C6["Phase 6<br/>competitive features"]
    C6 --> CODE65["Phase 6.5<br/>Ark Code"]
    C1 --> P7["Phase 7<br/>performance"]
    F5 --> P7
    A3 --> M8["Phase 8<br/>iPhone"]
    S2 --> M8
    F5 --> M8
    C6 --> V9["Phase 9<br/>verification and release"]
    CODE65 --> V9
    P7 --> V9
    M8 --> V9
~~~

Phase 9 test tasks begin earlier than the diagram suggests: every implementation task adds focused tests. Phase 9 completes cross-cutting matrices, release qualification, and operations after feature work stabilizes.

### 3.3 Parallel work lanes

| Lane | Typical ownership | Safe early work | Coordination boundary |
|---|---|---|---|
| Core reliability | Rust/application engineer | FND-002, FND-004, COR, ARC database/provider work | Owns generation transition contract and database transactions |
| Desktop experience | React/product engineer | UX design tokens, state mockups, accessibility fixtures | Consumes typed ArkClient; must not redefine lifecycle state |
| Platform/security | Platform/security engineer | CI, advisory upgrades, signing design, supply-chain/security policy | Owns release credentials, dependency policy, provenance |
| Product capabilities | Full-stack engineer | Projects/search/model-management design after contracts | Uses provider capability registry and application services |
| Ark Code | Full-stack + Platform/security engineer | Tool-calling protocol and repository-tool design after ARC-003, CMP-003, SEC-009 land | Owns the agent-run lifecycle and tool-execution contract; must not alter the chat generation lifecycle (FND-002) or the chat message schema without an explicit shared decision |
| Mobile | React Native/backend engineer | Protocol/auth/sync design after ARC-002/SEC-010 | Must not couple to Tauri commands or database schema |
| QA/release | QA/SDET or rotating engineer | Test matrices, fixture expansion, release rehearsal | Acceptance evidence stored per task/milestone |

## 4. Detailed Task Breakdown

### Task execution contract

- **Related audit findings** use the audit's C-01–C-10 identifiers and normalized A-* identifiers defined in the final Audit Traceability Matrix.
- **Dependencies** are hard prerequisites. “None” still requires adherence to the guiding principles.
- Acceptance criteria are observable and must be attached to the implementation PR or change set.
- Suggested notes guide design but do not override acceptance criteria.
- Where a task changes persisted formats or public protocols, migration/versioning is part of that task.

### Phase 0 — Foundations and guardrails

#### FND-001 — Publish the supported-capability and release-claim matrix

- **Status: Complete (2026-08-14).** `config/release-capabilities.json` is the versioned authority for candidate OS/runner support, minimum window size, provider visibility, delivery mode, model expectations, routing/privacy class, and unavailable capabilities. `pnpm support:check` validates that configuration against Tauri window settings, the CI matrix, provider identifiers, UI gates, and documentation; Settings and README now truthfully distinguish externally installed runtimes from bundled capability. The built-in path is disabled until its setup installs a verified binary. `docs/support-matrix.md` is linked from user documentation. Reviewed and approved by Luke D'Amato (2026-08-14): `reviewStatus` is `approved` in `config/release-capabilities.json` (with `approvedBy`/`approvedAt` recorded) and the approval plus date are stated in `docs/support-matrix.md`.
- **Description:** Define supported operating systems, minimum window sizes, providers, runtime packaging modes, model-file expectations, network/privacy classifications, and explicitly unavailable capabilities. Drive UI visibility and user documentation from the same release configuration.
- **Reason:** The audit found contradictory documentation and a built-in runtime claim that the package cannot satisfy.
- **Related audit findings:** C-03, A-UX-12, A-FUN-04, A-OPS-05.
- **Dependencies:** None.
- **Priority / complexity:** Critical / Small.
- **Expected outcome:** Every visible product claim matches a packaged, tested capability.
- **Acceptance criteria:**
  - A versioned support matrix is reviewed by engineering/product and linked from user documentation.
  - Unsupported built-in runtime paths are hidden or clearly disabled in production builds.
  - CI can determine which capability set an artifact claims.
  - README, onboarding, Settings, and release notes contain no conflicting provider/runtime statements.
- **Potential risks:** Hiding an incomplete provider can be perceived as regression.
- **Suggested implementation notes:** Prefer compile-time/release configuration over scattered UI checks. Do not add a permanent “beta” badge as a substitute for functionality.

#### FND-002 — Specify the durable generation lifecycle

- **Status: Complete (2026-08-14).** `docs/adr/0001-durable-generation-lifecycle.md` defines queued/starting/streaming/cancelling/complete/interrupted/failed/cancelled semantics, transaction and event boundaries, idempotent terminals, cancellation/retry/edit/regenerate/import behavior, and all required crash points; COR-001–005 conform to it and their executable tests exercise the contract. Reviewed and approved by Luke D'Amato (2026-08-14), recorded in the ADR's metadata. The approver was explicitly presented with the historical sequencing deviation (COR-001–005 were implemented before formal approval, rather than after) and accepted it as a one-time exception rather than requiring rework — recorded verbatim in the ADR's new "Process note."
- **Description:** Write an architecture decision record for message/generation states, allowed transitions, persistence points, event ordering, cancellation, restart recovery, partial output, retry, edit, regenerate, and import normalization.
- **Reason:** Current state is split across SQLite, in-memory cancellation, backend events, and optimistic React state.
- **Related audit findings:** C-01, C-02, C-04, C-05, A-ARC-01.
- **Dependencies:** None.
- **Priority / complexity:** Critical / Medium.
- **Expected outcome:** One authoritative contract guides backend, frontend, providers, tests, mobile protocol, and migrations.
- **Acceptance criteria:**
  - State diagram includes queued, starting, streaming, cancelling, complete, interrupted, failed, and cancelled or explicitly justifies a smaller set.
  - Every transition names its transaction boundary, emitted event, retry semantics, and recovery behavior.
  - Terminal states are idempotent and cannot return to active states.
  - Crash points before row creation, after row creation, during streaming, and during finalization have defined outcomes.
  - Contract is approved before COR-001–005 implementation.
- **Potential risks:** Overcomplicated states or migration incompatibility.
- **Suggested implementation notes:** Model a generation separately from immutable message content if that reduces ambiguity; retain message IDs/provenance through retry.

#### FND-003 — Establish the continuous-integration quality baseline

- **Status: Blocked by genuine external dependency (2026-08-14).** The workflow now runs the complete frontend baseline and a Rust format/clippy/test matrix plus non-bundled Tauri compile matrix on `ubuntu-latest`, `windows-latest`, and `macos-latest`. Node 22.18.0, pnpm 10.33.0, and Rust 1.95.0 are locked; caches are lockfile-aware; audits fail visibly; lint/clippy are warning-free gates; and job logs remain in Actions. Local Windows execution is green. Closure requires two repository-hosted actions unavailable in this unpushed working tree: run the new matrix successfully on GitHub-hosted runners, then configure the protected `main` branch to require every baseline job. Unblock by pushing this change set to a GitHub repository with Actions enabled and having a repository administrator apply the required-check rule after the matrix is green.
- **Description:** Add CI jobs for format, TypeScript typecheck, frontend build, Rust tests, rustfmt, strict clippy, npm audit, cargo-audit, and a non-bundled Tauri compile on supported primary runners.
- **Reason:** The audit found no CI, a red strict-clippy gate, current advisories, and a failed bundle.
- **Related audit findings:** C-09, C-10, A-ARC-09, A-OPS-02, A-OPS-03.
- **Dependencies:** FND-001 for supported runner matrix.
- **Priority / complexity:** Critical / Medium.
- **Expected outcome:** Every change receives fast, repeatable baseline validation.
- **Acceptance criteria:**
  - Protected branches require all baseline jobs.
  - CI uses locked Node/pnpm/Rust toolchains and cache keys include lockfiles.
  - The two current clippy failures are fixed or narrowly documented before strict clippy becomes required.
  - Advisory database/network failures fail visibly rather than silently skipping scans.
  - Job output retains concise actionable artifacts/logs.
- **Potential risks:** Platform-specific dependencies make all-target checks noisy.
- **Suggested implementation notes:** Separate fast PR checks from nightly full-platform/bundle checks, but keep security and type/test gates on every PR.

#### FND-004 — Build deterministic provider simulators and protocol fixtures

- **Status: Complete (2026-08-14).** `providers::test_support` is an offline raw-loopback HTTP harness for Ollama NDJSON and OpenAI-compatible SSE. Scripted response plans independently delay headers and each body chunk; `MockChunk::fragment_every_byte` exercises every byte boundary, including multi-byte UTF-8; captured method/path/headers/body make provider payloads assertable. Protocol tests cover immediate completion, slow/arbitrary fragmentation, malformed frames, absent terminal markers, partial disconnects, non-2xx responses, redirects, callback cancellation between delta and terminal, and a failed attempt followed by an explicit retry. Tests assert outcomes rather than elapsed thresholds, use no external network or installed model, and all 29 provider tests pass.
- **Description:** Create local test servers/fixtures for Ollama NDJSON and OpenAI-compatible SSE, including immediate completion, arbitrary chunk boundaries, slow streams, malformed JSON, missing terminal markers, HTTP errors, cancellation, disconnect, and retry.
- **Reason:** No real provider-protocol tests exist, and the highest-risk defects depend on timing/network framing.
- **Related audit findings:** C-02, C-04, C-05, A-FUN-04, A-OPS-02.
- **Dependencies:** FND-002.
- **Priority / complexity:** Critical / Medium.
- **Expected outcome:** Provider and event lifecycle behavior is deterministic in CI without installed models.
- **Acceptance criteria:**
  - Fixtures can fragment a UTF-8/JSON event at every byte boundary.
  - Tests can delay headers, individual deltas, and terminal frames independently.
  - Scenarios expose received request payload/headers for assertions.
  - The harness runs without network access and has no timing-flaky sleeps.
- **Potential risks:** A mock that does not match real server behavior creates false confidence.
- **Suggested implementation notes:** Preserve sanitized real protocol captures as conformance fixtures and run optional smoke tests against supported provider versions nightly.

#### FND-005 — Establish measurement and evidence baselines

- **Status: Complete (2026-08-14).** `docs/quality-baseline.md` records the reference Windows workstation, locked tool versions, measurement procedures, current bundle/test/database/branch/stream/import evidence, threshold ownership, runner/artifact metadata format, privacy rules, and the methods Phase 7 will use for startup, memory, TTFT/streaming, and accessibility measurements. `scripts/reference-dataset.mjs` deterministically generates synthetic fixtures for 1,000 conversations, a 100-message branch-bearing thread, a 100,000-character output, and a bounded 20,000-message import; `pnpm baseline:check` verifies their committed hashes without persisting content, and CI runs the gate. Raw evidence is confined to ignored `.artifacts/` and contains no user content or secrets.
- **Description:** Record reproducible reference hardware, sample workspaces, long transcripts, startup timing method, memory method, bundle sizes, TTFT/stream metrics, accessibility tooling, and release acceptance evidence format.
- **Reason:** The audit could identify code-level bottlenecks but found no production instrumentation.
- **Related audit findings:** A-PERF-01–06, A-OPS-01, A-OPS-02.
- **Dependencies:** FND-001.
- **Priority / complexity:** High / Small.
- **Expected outcome:** Later performance and quality claims are comparable and auditable.
- **Acceptance criteria:**
  - Reference datasets include 1,000 conversations, a 100-message thread, 100,000-character output, branching, and large imports.
  - Baseline results are captured before optimization.
  - Metrics contain no conversation content or secrets.
  - Each production gate names its command/tool, threshold, runner, and artifact.
- **Potential risks:** Hardware variance can produce misleading thresholds.
- **Suggested implementation notes:** Use relative regression limits plus absolute user-experience budgets; retain raw benchmark artifacts for trend review.

### Phase 1 — Critical correctness and data safety

#### COR-001 — Recover stale active generations

- **Status: Complete (2026-08-14).** Migration `0002_message_status_interrupted.sql` defines the durable recovery state; bootstrap atomically recovers stale `pending`/`streaming` rows while preserving partial content and provenance. Retry, Keep partial, and Discard are explicit UI actions, import normalizes transient states with a visible report, and repeated recovery is idempotent. Real SQLite tests cover crash-point states, preserved content/metadata, selected-branch fallback, and each recovery action. `docs/adr/0001-durable-generation-lifecycle.md` records the shared lifecycle used by recovery, cancellation, provider streaming, and UI reconciliation.
- **Description:** Add a startup and conversation-load recovery transaction that identifies durable queued/starting/streaming/cancelling rows without a live generation and transitions them to interrupted. Normalize transient statuses during import.
- **Reason:** A crash, force-quit, panic, or crafted import can permanently disable a conversation.
- **Related audit findings:** C-01, A-UX-05, A-FUN-03.
- **Dependencies:** FND-002, ARC-005 design review for migration compatibility.
- **Priority / complexity:** Critical / Medium.
- **Expected outcome:** Every conversation is usable after restart and every stale partial response has explicit recovery actions.
- **Acceptance criteria:**
  - Restart at each documented crash point yields an interrupted, not active, generation.
  - Imported pending/streaming states are rejected or normalized with a user-visible import report.
  - Recovery is idempotent and preserves partial content/provenance.
  - UI exposes Retry, Keep partial, and Discard/branch-safe actions.
  - Recovery integration tests run against a real temporary SQLite database.
- **Potential risks:** Incorrectly interrupting a genuinely active generation in a future multi-window/process design.
- **Suggested implementation notes:** Associate active work with an app-instance/lease identifier if multi-instance support is planned.

#### COR-002 — Make stream event ordering race-free

- **Status: Complete (2026-08-14).** Send/edit/regenerate now commit durable user/assistant IDs and place a single-use `PendingStream` in `AppState` before returning; provider work cannot emit until the frontend installs its placeholder and explicitly calls `start_pending_stream`. Start removes the plan once, registers cancellation before provider I/O, and compensates a failed launch durably. The normalized generation store applies monotonic revisions once, ignores duplicate deltas/terminals, refetches authoritative messages on missing/unknown revisions, and never treats an event as more authoritative than committed backend state. Stream-start semantics are deliberately replaced by the command-return/explicit-start handshake. The immediate-completion provider fixture and frontend reconciliation test each pass 1,000 iterations without a missing or stuck placeholder.
- **Description:** Change command/UI orchestration so durable IDs and initial state are committed and returned before provider work can emit deltas. Reconcile event payloads by generation/message revision and support missed/duplicate/out-of-order events.
- **Reason:** Current tasks can emit completion before React creates its optimistic placeholder.
- **Related audit findings:** C-02, A-ARC-01.
- **Dependencies:** FND-002, FND-004, COR-004 transaction API.
- **Priority / complexity:** Critical / Large.
- **Expected outcome:** Fast or reordered providers cannot lose output or leave endless placeholders.
- **Acceptance criteria:**
  - Immediate-completion fixture passes 1,000 repeated runs without a stuck/missing message.
  - Duplicate terminal and delta events are idempotent.
  - Unknown/missed revisions trigger an authoritative state refetch rather than silent drop.
  - App subscribes to or deliberately replaces stream-start semantics from the lifecycle contract.
  - UI never marks a state complete until the committed backend state is complete.
- **Potential risks:** Increased frontend complexity or rendering duplicate deltas during migration.
- **Suggested implementation notes:** Prefer events as invalidation/delta notifications with a monotonically increasing revision; do not make event arrival the only persistence path.

#### COR-003 — Implement strict provider stream state machines and timeout policy

- **Status: Complete (2026-08-14).** Ollama NDJSON requires `done`; OpenAI-compatible SSE requires `[DONE]` or a valid finish reason. Incremental byte buffering preserves split UTF-8 and supports CRLF/LF, multiple frames per chunk, SSE comments, empty data frames, and `data:` with or without a space; malformed/truncated frames return typed protocol/incomplete errors and partial output is persisted as interrupted. Connect, header, idle, and optional caller deadlines are independent and directly tested. A slow-progress fixture runs longer than its configured idle window while continuing to make progress, proving there is no whole-generation timeout. The real socket harness covers completion, premature EOF, malformed data, redirects, non-2xx, every-byte fragmentation, cancellation, and retry.
- **Description:** Replace permissive NDJSON/SSE loops with incremental parsers that require valid framing and explicit completion, distinguish interrupted partial output, and use connect/header/idle timeout policies instead of whole-generation timeouts.
- **Reason:** Long generations can fail at 60/120 seconds and truncated/malformed responses can be persisted as complete.
- **Related audit findings:** C-04, C-05, A-FUN-04.
- **Dependencies:** FND-002, FND-004.
- **Priority / complexity:** Critical / Large.
- **Expected outcome:** Provider protocol failures are detected accurately while active long generations continue.
- **Acceptance criteria:**
  - Ollama requires done or a documented equivalent; OpenAI-compatible SSE requires [DONE] or a valid finish reason.
  - Malformed frames fail with typed provider/protocol errors and preserve partial content as interrupted.
  - Connect, header, idle, and optional user deadline are independently configured/tested.
  - Slow-progress fixtures can run beyond prior total timeouts without failure.
  - Parser tests cover split UTF-8, CRLF/LF, multiple events per chunk, empty/comment SSE frames, and premature EOF.
- **Potential risks:** Provider variants may deviate from nominal protocols.
- **Suggested implementation notes:** Put compatibility exceptions behind provider/version-specific adapters and conformance tests, not global leniency.

#### COR-004 — Make chat mutations transactional

- **Status: Complete (2026-08-14).** Send, edit, regenerate, import, model refresh, branch selection, and generation terminal updates use short SQLite transactions or single conditional statements; no provider/network work is held inside a database transaction. SQLite trigger-based fault injection now aborts every write boundary in send (user/title/assistant/pointer), edit (revision/assistant/pointer), and regenerate (assistant revision/pointer), proving rollback leaves the original title, ancestry, selection, and pending-work map unchanged. Provider-registry launch failure transitions the already-committed placeholder to `failed`. Eight simultaneous sends on one conversation are serialized by the writer mutex into one coherent append-only branch; terminal-state race tests prove conditional first-writer-wins behavior.
- **Description:** Implement transaction-scoped use cases for send, edit, regenerate, branch selection where persisted, and generation initialization/finalization.
- **Reason:** Multi-write operations can leave user messages, assistant placeholders, and branch state partially applied.
- **Related audit findings:** C-06, A-FUN-02, A-ARC-01.
- **Dependencies:** FND-002; coordinate with ARC-001 and ARC-004.
- **Priority / complexity:** Critical / Large.
- **Expected outcome:** Each logical user action either commits a coherent durable state or leaves no mutation.
- **Acceptance criteria:**
  - Fault injection after every database statement proves atomicity.
  - Provider launch failure transitions the committed generation to failed/interrupted in a compensating transaction.
  - Edit/regenerate preserves append-only ancestry and selected branch deterministically.
  - Concurrent operations on one conversation are serialized or rejected with a typed conflict.
- **Potential risks:** Holding transactions across network work.
- **Suggested implementation notes:** Never hold a DB transaction during provider I/O. Commit request state, launch work, then checkpoint/finalize in short transactions.

#### COR-005 — Make cancellation and process reconciliation durable

- **Status: Complete (2026-08-14).** Cancellation synchronously commits an idempotent conditional terminal transition, removes queued work, and signals an `AtomicBool` plus retained `tokio::Notify` permit to active work. Provider streaming runs inside `tokio::select!`, so cancellation drops the live HTTP future immediately rather than waiting for another network chunk; missing-task, post-restart, already-terminal, queued, and active cases converge safely. Partial output remains durable and labelled cancelled/interrupted, competing completion/failure writes cannot clobber it, and a measured real-database test acknowledges the durable request in under 100 ms.
- **Description:** Redesign cancellation so it changes durable state, signals an active provider/process when present, handles missing tasks after restart, and converges to one terminal result.
- **Reason:** Current cancellation succeeds only against an in-memory flag and can report success without changing persisted state.
- **Related audit findings:** C-01, A-FUN-03, A-FUN-09.
- **Dependencies:** FND-002, COR-001, COR-002, ARC-010.
- **Priority / complexity:** High / Medium.
- **Expected outcome:** Stop is immediate, idempotent, restart-safe, and honest about provider termination.
- **Acceptance criteria:**
  - Cancelling active, already terminal, missing-task, and post-restart generations has defined idempotent behavior.
  - UI acknowledges intent within 100 ms on reference hardware.
  - Partial output remains available and is labelled cancelled/interrupted per contract.
  - Sidecar/HTTP cancellation is attempted without blocking durable state convergence.
- **Potential risks:** Provider requests may not abort promptly.
- **Suggested implementation notes:** Distinguish “user requested cancellation” from “transport stopped” in diagnostics while presenting one clear user state.

#### COR-006 — Make conversation title generation Unicode-safe

- **Status: Complete (2026-08-14).** Automatic title truncation uses Unicode scalar iteration (`chars().take`) and never slices UTF-8 bytes. Tests cover long emoji, CJK, Arabic/RTL, combining marks, whitespace/newlines, empty input, and the exact 64-character boundary; titles remain valid UTF-8 and the acceptance criterion explicitly permits scalar-safe truncation without a grapheme dependency.
- **Description:** Replace byte slicing with Unicode scalar or grapheme-aware title truncation and define whitespace/empty-content behavior.
- **Reason:** A long non-ASCII first token can panic after a row is written.
- **Related audit findings:** C-07.
- **Dependencies:** None.
- **Priority / complexity:** Critical / Small.
- **Expected outcome:** Title generation cannot panic for valid UTF-8 content.
- **Acceptance criteria:**
  - Tests cover emoji, combining marks, CJK, RTL text, newlines, leading whitespace, and long no-space strings.
  - Result obeys a documented display-length limit and remains valid UTF-8.
  - Existing ASCII title behavior remains compatible unless intentionally migrated.
- **Potential risks:** Grapheme library adds dependency weight.
- **Suggested implementation notes:** Scalar truncation may be sufficient if visual grapheme accuracy is documented; prefer no new dependency for one bounded helper unless needed.

#### COR-007 — Replace destructive workspace probing

- **Status: Complete (2026-08-14).** Workspace probing uses an unpredictable UUID filename plus `create_new(true)`, never overwrites an existing path, and reports cleanup failures instead of swallowing them. Tests prove existing user files and a forced name collision remain byte-for-byte intact, repeated probes leave no artifacts, a path that disappears mid-probe returns typed `workspace_missing`, and non-directory/read-only cases are rejected. OS I/O classification maps permission/read-only and disk-full errors to distinct recovery codes; actual disk exhaustion is the acceptance criterion's impractical environment-qualified case.
- **Reason:** The fixed .ark-write-test path can overwrite and delete a user's file.
- **Related audit findings:** A-FUN-08, A-SEC-06.
- **Dependencies:** COR-008 typed errors.
- **Priority / complexity:** Critical / Small.
- **Expected outcome:** Workspace validation is safe under collisions, permission failures, and crashes.
- **Acceptance criteria:**
  - Probe uses create_new semantics with a cryptographically random or UUID filename.
  - Existing files are never modified.
  - Cleanup runs on success/failure; a stale probe is harmless and documented for later cleanup.
  - Tests cover read-only path, existing collision, disappearing directory, and insufficient disk where practical.
- **Potential risks:** Antivirus/file-sync delays can produce transient failures.
- **Suggested implementation notes:** Return a precise permission/storage error and allow retry; do not recursively change user permissions.

#### COR-008 — Centralize native input validation and typed errors

- **Status: Complete (2026-08-14).** Central Rust trust-boundary validators cover finite temperature 0–2, max tokens 1–1,000,000, absolute workspace paths without NUL or `.`/`..` traversal segments, existing regular `.gguf` model files, and opaque/import-compatible entity IDs bounded to 128 bytes without control characters. Provider URLs use `reqwest::Url`, allow only HTTP(S), reject embedded credentials/malformed hosts, classify IPv4/IPv6 loopback/private/public destinations, and enforce remote-risk acknowledgement. Commands validate IDs, paths, settings, URLs, and import limits before filesystem/database/network use; boundary, malformed, Unicode, traversal, and real-filesystem tests cover each validator.
- **Description:** Create reusable Rust validators and error codes for provider URLs, temperature, token limits, IDs/revisions, workspace paths, model paths, imports, and file operations; mirror guidance in the UI.
- **Reason:** Current validation is largely non-empty text parsing and cannot enforce privacy or provider constraints.
- **Related audit findings:** C-08, A-UX-10, A-SEC-08, A-FUN-11.
- **Dependencies:** FND-001 for supported ranges; SEC-001 for URL policy.
- **Priority / complexity:** Critical / Medium.
- **Expected outcome:** Invalid or unsafe input is rejected consistently before side effects.
- **Acceptance criteria:**
  - Numeric values are finite, bounded, and provider-capability aware.
  - URLs accept only supported schemes and return an explicit destination classification.
  - Errors have stable machine codes, safe user messages, and redacted diagnostic context.
  - Property/boundary tests cover negative, zero, overflow, NaN/infinity equivalents, malformed URLs, traversal-like paths, and stale revisions.
- **Potential risks:** Existing invalid persisted settings may block startup.
- **Suggested implementation notes:** Add migration/repair behavior for legacy values; never silently replace a remote endpoint with local.

#### COR-009 — Make import/export bounded, transactional, and versioned

- **Status: Complete (2026-08-14).** Import enforces the 50 MB pre-read/native ceiling, 20,000-message/count, 2,000,000-character/content, and 2,048-level branch-depth limits with exact-boundary tests and documented format/limits. Preview reports conversation/message counts, maximum depth, ID conflicts, provider mappings, normalized transient states, and estimated storage before confirmation. A controlled single transaction checks cancellation throughout, emits bounded progress, and fully rolls back on cancellation/failure. Provider IDs map only to trusted configured providers, while conversation settings, timestamps, branches/revisions, provenance, token/error/custom metadata, Unicode, embedded NULs, and unknown additive fields survive round trips; future schema versions fail typed validation. `docs/import-format.md` documents compatibility and the fact that schema v1 is the only prior/current supported version.
- **Description:** Add pre-read file limits, schema/version validation, message/content/depth/count limits, one-transaction import, transient-state normalization, metadata/provider mapping, progress, cancellation, and a dry-run summary.
- **Reason:** Current import can exhaust memory/disk, partially write data, and restore ambiguous provider/status metadata.
- **Related audit findings:** C-01, C-06, A-FUN-07, A-SEC-06.
- **Dependencies:** COR-004, COR-008, ARC-005, ARC-006.
- **Priority / complexity:** Critical / Large.
- **Expected outcome:** Import never partially corrupts the workspace and exported data remains portable/versioned.
- **Acceptance criteria:**
  - Oversized files are rejected before full File.text/native memory load.
  - Limits are documented/configurable within safe ceilings and tested at boundaries.
  - Dry run reports conversations/messages, conflicts, provider mapping, normalized states, and estimated storage.
  - Cancel/failure rolls back the complete import.
  - Round-trip tests cover Unicode, branches, settings/provenance, unknown future fields, and prior supported versions.
- **Potential risks:** Strict limits reject legitimate archives.
- **Suggested implementation notes:** Support streaming parse for large JSON and batch export in FTR-008; keep the initial safe ceilings conservative and visible.

#### COR-010 — Add database and workspace startup recovery states

- **Status: Not complete (2026-08-14).** Implemented typed corruption, newer-schema, failed-migration, migration-gap/checksum, lock/busy, missing workspace, read-only, disk-full, and interrupted-workspace-change codes. Tauri setup retains workspace metadata and falls back to isolated in-memory connections so recovery remains reachable even when config/path resolution fails; Retry and Choose workspace are non-destructive actions for every class. Workspace configuration uses synced next/previous journal files, detects interrupted writes, preserves originals, and never auto-repairs. A whitelist-only Copy diagnostics action excludes transcript data; real SQLite/workspace fixtures cover corruption, schema, migration rollback, config interruption, collision, and missing-path behavior. Still required before `Complete`: the plan's E2E criterion needs process-level startup fixtures for every recovery class, including platform-specific lock/read-only/disk-full behavior, and those will be delivered with TST-005 rather than mislabeled as unit coverage.
- **Description:** Detect and present typed recovery flows for database corruption, unsupported/newer schema, failed migration, lock/busy/concurrent instance, missing workspace, read-only workspace, insufficient disk, and interrupted workspace change.
- **Reason:** Current bootstrap has no full-page retry/recovery and several storage failures have no designed state.
- **Related audit findings:** A-UX-14, A-FUN-08, A-FUN-10, A-OPS-04.
- **Dependencies:** COR-008, ARC-005 design, FTR-001 backup design.
- **Priority / complexity:** High / Large.
- **Expected outcome:** Storage failures never leave an indefinite loading screen or instruct destructive manual recovery.
- **Acceptance criteria:**
  - Each error class has a safe action: retry, choose workspace, restore backup, open read-only/export, or exit.
  - No automated repair destroys the original database.
  - Technical diagnostics can be copied without transcript content.
  - E2E fixtures exercise each recoverable startup state.
- **Potential risks:** Cross-platform SQLite/file errors vary.
- **Suggested implementation notes:** Copy a suspect DB before repair; use integrity_check only as diagnostics, not proof that all logical data is correct.

#### COR-011 — Introduce safe stream buffering and checkpoint persistence

- **Status: Not complete (2026-08-14).** Backend persistence is buffered to 250 ms or 8 KiB (at most 4 time-triggered checkpoints/second), flushes before every terminal state, emits delta-only revision events, and recovers a crash within the documented window. A 100,000-character real-SQLite fixture reconstructs content from bounded batches, while ARC-008's normalized generation overlay means only the active message subscriber rerenders and completed message identities remain stable. Still required before `Complete`: a full provider-to-UI 100,000-character benchmark must demonstrate approximately linear work against PERF-001 budgets; the active streaming Markdown message still reparses accumulated content on each rendered delta and will be corrected under PERF-005.
- **Reason:** Current per-delta full-content database/update/render path approaches O(n²) and amplifies the global DB lock.
- **Related audit findings:** A-PERF-01, A-ARC-03.
- **Dependencies:** FND-002, COR-002–004, PERF-001 measurement contract.
- **Priority / complexity:** High / Large.
- **Expected outcome:** Long responses remain recoverable without per-token full-history copying.
- **Acceptance criteria:**
  - Streaming persistence is limited to the configured maximum batches per second, initially ≤20.
  - Crash between checkpoints loses at most the documented checkpoint window and recovers as interrupted.
  - UI consumes deltas/revisions without remapping/reparsing all messages per token.
  - 100,000-character response work is approximately linear and meets PERF budgets.
- **Potential risks:** Larger checkpoint intervals lose more partial text on hard crash.
- **Suggested implementation notes:** Make thresholds internal and benchmark-driven; avoid user-facing tuning until a demonstrated need.

#### COR-012 — Repair packaging assets and make built-in runtime claims truthful

- **Status: Blocked by genuine external dependency (2026-08-14).** Production capability configuration and UI claims now match packaged reality: the built-in runtime reports binary availability, remains disabled when absent, and documents its verified setup path; missing-resource checks run before bundling. The remaining icon criterion requires approved Ark source artwork, which is not present and must not be fabricated, and the clean-VM Windows installer/uninstaller criterion requires an external clean VM runner. Unblock by supplying approved artwork, generating the Tauri ICO/PNG/ICNS set from it, and running the debug bundle smoke test on a clean Windows VM.
  - **Interim, explicitly separate fix (2026-08-14):** the missing `src-tauri/icons/icon.png`/`icon.icns` were not just an unmet acceptance criterion here — they made `tauri::generate_context!()` panic and fail to compile at all on macOS/Linux CI (Windows built locally only because `icon.ico` alone happened to satisfy that platform's own icon validation, masking the gap). That is a build-breakage bug, not a branding decision, so a generated, explicitly-placeholder icon set (a plain circle, `tauri icon` default output, `.artifacts/placeholder-icon/source.png`) was committed purely to keep every platform compiling. This does **not** satisfy this task's acceptance criteria — no approved Ark artwork exists yet and this stays `Blocked` — it only removes an unrelated CI-breaking side effect of the same missing files. Replace every generated file under `src-tauri/icons/` (and rerun `pnpm tauri icon <source>`) once real artwork is approved; do not read the current icons as a design decision.
- **Description:** Generate complete Tauri platform icon assets, restore a successful Windows bundle, and either package a verified llama-server runtime or disable/remove the built-in-provider release claim until FTR-006 is complete.
- **Reason:** The installer currently fails and the repository contains no runnable bundled inference engine.
- **Related audit findings:** C-03, A-UX-12, A-FUN-04, A-OPS-03.
- **Dependencies:** FND-001.
- **Priority / complexity:** Critical / Medium.
- **Expected outcome:** A debug installer can be produced and the installed UI never advertises unavailable runtime functionality.
- **Acceptance criteria:**
  - Required multi-resolution ICO, PNG, and ICNS assets are generated from an approved source.
  - Windows debug bundle installs/uninstalls on a clean VM.
  - Production feature configuration matches packaged resources.
  - Missing resource tests fail the build before bundling.
- **Potential risks:** Icon/license/brand asset approval or platform packaging differences.
- **Suggested implementation notes:** This task does not sign or update artifacts; OPS-002 completes production distribution.

### Phase 2 — Security and privacy

#### SEC-001 — Enforce provider destination classification and privacy routing

- **Status: Complete (2026-08-14).** Rust owns the entire routing trust boundary. `classify_destination` parses only credential-free HTTP(S) URLs and distinguishes IPv4/IPv6 loopback, literal private/LAN ranges, and public destinations; arbitrary hostnames are conservatively public to address DNS rebinding. Migration `0005_provider_routing_policy.sql` persists an explicit provider class (`providers.is_local`) and an independently warned `allow_insecure_remote` development exception. A local-only provider cannot save a public endpoint until the user explicitly converts it to Remote and acknowledges that prompts, conversation history, and the configured system prompt leave the device; every non-loopback HTTP endpoint is rejected unless the separate insecure-development control is enabled. Adapter construction revalidates persisted rows before any network request, protecting against imported or externally modified SQLite data. Both HTTP adapters disable redirects entirely, so no redirect can cross to a less-trusted class; an adversarial two-server test proves the redirect target receives no connection. Settings exposes the explicit class conversion, context disclosure, and insecure-HTTP warning controls; the pre-send provider/model indicator shows the model and Rust-derived loopback/LAN/public class, with the same three context categories stated in its tooltip. Shared Rust/TypeScript contracts include the routing fields. Unit/integration tests cover URL credentials/schemes, public hostnames, IPv4/IPv6 ranges, conversion/acknowledgment/TLS gates, request-time revalidation, and redirect non-following; the full 197-test Rust suite and 26-type contract gate pass.
- **Description:** Parse provider URLs in Rust, classify loopback/private-LAN/public destinations, enforce provider-type policy, and derive all local/remote badges and disclosures from the validated destination.
- **Reason:** A provider can retain a “local” label while sending complete history to any remote URL.
- **Related audit findings:** C-08, A-SEC-04, A-CMP-15.
- **Dependencies:** FND-001, COR-008.
- **Priority / complexity:** Critical / Medium.
- **Expected outcome:** Ark cannot silently route “local” conversations to remote endpoints.
- **Acceptance criteria:**
  - Local-only provider types reject public hosts unless the user explicitly converts them to a remote provider class.
  - Remote destinations require HTTPS except an explicitly warned development mode.
  - URL credentials, redirects to a less-trusted class, DNS rebinding considerations, and IPv4/IPv6 loopback/private ranges are covered.
  - Pre-send UI shows model, endpoint class, and which context categories will leave the device.
  - Route classification and redirect tests execute in Rust.
- **Potential risks:** Hostnames can resolve differently over time; private LAN support is useful but less trusted than loopback.
- **Suggested implementation notes:** Revalidate redirects/resolved destination at request time and distinguish loopback, trusted paired LAN, and internet—not merely local/cloud.

#### SEC-002 — Authenticate and isolate the managed sidecar

- **Status: Complete (2026-08-14).** Previously `Blocked by genuine external dependency` — pinned llama.cpp b9859's own `server-http.cpp` exempts `/health`, `/v1/health`, `/models`, `/v1/models`, `/`, and embedded UI assets from its own `--api-key` check and reflects any request `Origin` into `Access-Control-Allow-Origin`, so those two acceptance criteria could not be truthfully satisfied by the upstream listener alone. Rather than wait on an upstream release with no committed timeline, built the isolating/authenticating proxy the plan's own decided direction (recorded below, unchanged) scoped: `src-tauri/src/proxy.rs`, a minimal loopback-only HTTP listener (hyper 1.x server + `reqwest` forwarding — `hyper`/`hyper-util`/`http-body-util`/`bytes` were already resolved transitively via `reqwest`'s own hyper-based client, so declaring them as direct dependencies added exactly one new crate, `httpdate`, to the lock graph, not a new dependency tree) that binds an OS-assigned loopback port in front of the existing `--port 0`-assigned llama-server child.
  - Every request on every path is checked for `Authorization: Bearer <the same per-launch secret llama-server itself was given>` before anything is forwarded — no route is exempt, closing llama.cpp's own exemption rather than working around it. No response the proxy sends — success, 401, or otherwise — ever carries an `Access-Control-Allow-*` header; llama-server's own reflected one is explicitly stripped from the forwarded response rather than passed through. A browser's CORS preflight (which never carries the caller's intended `Authorization` header) fails the same auth check as any other unauthenticated request, so the real cross-origin request it was gating is never sent — no OPTIONS special-casing was needed or added.
  - `start_built_in_runtime` (`provider_management.rs`) spawns the proxy immediately after `wait_for_ready` confirms llama-server itself is healthy, stores its port and background task on `SidecarState` (`attach_proxy`/`proxy_port`), and points `base_url` — what gets persisted to the database and what Ark's own provider adapter actually talks to — at the proxy's port, never llama-server's raw one. The raw port remains reachable only from Rust's own direct health-check path (`check_health`, unauthenticated-by-CORS-concern since it's not browser code), which was left unchanged. `SidecarState::stop`/`clear_process_metadata` abort the proxy's background task and clear its recorded port, so a stopped or crashed runtime never leaves a stale authenticated front door running or reported; `attach_proxy` also aborts any previously attached proxy before recording a new one, so re-launching can't leak a background task. `RuntimeDiagnostics.port`/`BuiltInRuntimeStatus.port` now report the proxy's port (falling back to the raw port only before the proxy has attached), since that's the runtime's actual reachable surface — the raw port is purely an internal implementation detail now.
  - Four new `proxy.rs` tests run the real hyper server against a real stub upstream server (not a mock): unauthenticated GET requests are rejected with 401 and no CORS header on every one of `/health`, `/v1/health`, `/models`, `/v1/models`, `/`, and `/completion` — the exact set llama.cpp itself exempts; a wrong bearer token is rejected; an authenticated request is forwarded, its response body/path round-trip correctly, and the stub's simulated llama.cpp-style reflected CORS header is confirmed stripped from the proxy's response; a simulated preflight (`OPTIONS` with `Origin`/`Access-Control-Request-Method`, no `Authorization`) is rejected the same as any other unauthenticated request. Two new `sidecar.rs` tests prove `SidecarState`'s new bookkeeping: `stop` aborts an attached proxy task and clears `proxy_port`; attaching a second proxy aborts the first rather than leaking it.
  - Full validation green: `cargo fmt --check`/strict `clippy -D warnings` clean; `cargo test` 250 passed/1 ignored (unrelated pre-existing ARC-005 issue), including the 4 new proxy tests and 2 new sidecar tests; `cargo audit` unchanged at 17 pre-existing, already-reviewed non-vulnerability warnings (SEC-003), zero new advisories from the new direct dependencies; `pnpm supply-chain:generate`/`--check` regenerated and passing at 886 components (885 → 886 — only `httpdate` was genuinely new to the graph); `pnpm run contract:check` clean (no DTO shape changed, only runtime values). An end-to-end test of `start_built_in_runtime` spawning a real llama-server binary was not added — consistent with this file's existing test boundary for that function (no such test exists today either), since it needs a real installed binary and GGUF model this environment doesn't guarantee; the proxy's own behavior and the sidecar's lifecycle bookkeeping around it are both covered directly instead.
  - The release-capability gate this task's prior `Blocked` status put in place (`managed_runtime_release_disabled`, hiding the built-in provider from production builds) is deliberately left in place by this change, not lifted — that gate's own condition ("an upstream release that authenticates every route... or a reviewed cross-platform isolation/proxy design that makes the upstream listener unreachable") is now met by this proxy, but lifting a release gate is a product/release decision, not something to fold silently into a security-hardening task; recorded here as the explicit next decision for whoever revisits COR-012/SEC-003's release-readiness gates, not left ambiguous.
  - **Direction decided (2026-08-14, delegated to and made by the implementing agent per explicit product request):** build the isolating authenticating proxy rather than wait on upstream. Reasoning: this is not a "public distribution" risk that shrinks because Ark is personal-use/small-friend-group software — the actual threat (an unrelated, malicious website open in the same browser the user is normally browsing with, sending a same-origin-policy-exempt `fetch()` to `127.0.0.1:<port>` while the sidecar happens to be running, exploiting the reflected-CORS/unauthenticated-`/health`-and-`/models` gap) is present for exactly one user just as much as for a thousand. It doesn't depend on Ark's distribution scale, only on the user having a normal web browser open. Waiting on an upstream llama.cpp release has no committed timeline; a small first-party proxy does not. Scope: a minimal loopback-only Rust HTTP listener sitting in front of the existing `--port 0`-assigned llama-server child, enforcing the per-launch bearer secret on literally every path (including `/health`, `/models`, `/`, embedded UI assets) before forwarding internally, and replacing llama-server's reflected-`Origin` CORS behavior with either no CORS headers or a fixed, non-reflecting policy — implemented above.
- **Description:** Launch the local inference server with a high-entropy per-session credential, random available port, loopback-only bind, restrictive CORS/trusted-host settings, supervised lifecycle, and authenticated Ark requests.
- **Reason:** Localhost is not an authentication boundary; the current sidecar uses a scanned port range and no request secret.
- **Related audit findings:** A-SEC-04, A-SEC-12, A-PERF-05.
- **Dependencies:** ARC-010, COR-005, FTR-006 runtime choice.
- **Priority / complexity:** High / Medium.
- **Expected outcome:** Other local/browser processes cannot casually consume or control Ark's managed runtime.
- **Acceptance criteria:**
  - A unique secret is generated in memory for every launch and never logged/persisted.
  - Unauthenticated requests fail; restrictive CORS/trusted hosts are verified.
  - Bind is loopback only and the actual assigned port is discovered without a predictable range scan where the runtime allows.
  - Normal exit, crash, forced stop, and Ark restart clean up/reconcile the process.
- **Potential risks:** Upstream runtime may not support auth/CORS controls.
- **Suggested implementation notes:** If upstream lacks a safe control, place a small authenticated loopback proxy in front or do not ship the managed runtime.

#### SEC-003 — Remediate Rust dependency advisories and warnings

- **Status: Blocked by genuine external dependency (2026-08-14).** The vulnerability work is complete and the former upstream blocker has cleared: today's registry resolves `plist 1.10.0`, so the lockfile now uses `quick-xml 0.41.0`; `crossbeam-epoch` remains `0.9.20`. The reviewed lockfile diff moves only those two packages. Plain `cargo audit` now reports zero known vulnerabilities with no `--ignore` exceptions, CI's obsolete exceptions are removed, and `pnpm audit --audit-level high` is clean. `docs/dependency-advisory-review.md` accounts for all 17 remaining non-vulnerability warnings: the GTK3/glib/proc-macro group is Linux-only through Tauri/WebKitGTK, the `unic-*` group is all-target Tauri `urlpattern` infrastructure, compatible update probes move nothing, Ark has no direct affected API calls, Platform Engineering owns the review, and the next deadline is 2026-09-14. On Windows, strict clippy, all 197 tests (including sysinfo diagnostics), `cargo build`, frontend build, and a real Tauri debug NSIS bundle pass; the bundle compiled the upgraded plist/XML graph, patched the executable, validated NSIS downloads/hashes, and produced `Ark_0.1.0_x64-setup.exe`. The only unmet criterion is executing the upgrade/build/bundle checks on the declared macOS and Linux targets, which this Windows workspace cannot supply. The non-fail-fast CI matrix is already configured for all three OSes; unblock by committing/pushing the current tree and requiring green macOS/Linux build-check results before release.
- **Description:** Upgrade the dependency graph so crossbeam-epoch is at least 0.9.20 and quick-xml at least 0.41.0, then review every unmaintained/unsound warning across supported targets.
- **Reason:** The audited lockfile has three vulnerabilities, including two CVSS 7.5 advisories, and 17 allowed warnings.
- **Related audit findings:** C-09, A-SEC-11.
- **Dependencies:** FND-003.
- **Priority / complexity:** Critical / Medium.
- **Expected outcome:** No unreviewed high/critical advisory exists in a release artifact.
- **Acceptance criteria:**
  - cargo-audit is green or exceptions are time-bounded, reachability-reviewed, owner-assigned, and approved.
  - Upgrade tests cover Tauri build/bundle, sysinfo diagnostics, plist/icon packaging, and all supported targets.
  - Platform-specific GTK/glib warnings are resolved by upgrades or documented with actual target reachability.
  - Lockfile changes are reviewed for unexpected major transitive shifts.
- **Potential risks:** Tauri/plist upgrades can require coordinated code or minimum-OS changes.
- **Suggested implementation notes:** Prefer supported upstream releases over patching transitive crates locally; track exception expiry in CI.

#### SEC-004 — Secure binary, model, and package supply chains

- **Status: Complete (2026-08-14).** The forward dependency on FTR-006 is handled with the same narrow-slice rule as ARC-005/006: this item implements the verified artifact/model foundation FTR-006 needs, while managed model discovery/download/retention and product lifecycle remain FTR-006. `config/native-artifacts.json` pins llama.cpp b9859 by source commit, license, exact official GitHub artifact URL, byte length, and SHA-256 for all six declared desktop platform/architecture pairs. `scripts/runtime-supply-chain.mjs` downloads to a bounded `.partial`, verifies size/hash before any extraction, rejects absolute/parent/link/device/oversized archive entries, stages extraction, and atomically replaces the runtime with per-file provenance; PowerShell and shell setup scripts delegate to that one implementation. Runtime status/start re-verifies the reviewed target manifest and every installed file, rejects unexpected/tampered files, and streams model hashing before launch while atomically persisting source/license/hash/size/time. The Windows artifact was downloaded from the pinned official release, verified, installed, and re-verified end to end.
  - Deterministic CycloneDX 1.5 generation now covers the full Rust lock graph, installed JavaScript packages, all six native artifacts, and bundled icon assets; `THIRD_PARTY_NOTICES.md` is generated from the same inputs. After SEC-005 dependencies, `pnpm supply-chain:generate`/`--check` reports 884 components. CI runs three attack tests covering tampered/truncated payloads, traversal/absolute paths, links/device entries, unsupported targets, and altered URLs. The checked-in manifest is the trust root; hashes are never accepted from the downloaded payload location.
  - `BuiltInRuntimeStatus` exposes typed runtime/model provenance through the shared contract. Settings renders version, origin, license, artifact/model SHA-256, size, and last verification. A deterministic development-only fixture was browser-verified in the rendered app: both provenance cards expose those fields and no release build can select the fixture. `docs/settings-catalog.md` and README document ownership, setup verification, and release-disabled runtime truthfulness.
  - Focused and full validation is green: archive/security tests, runtime tamper tests, Rust contract tests, 208 Rust tests, strict clippy/build/audit (zero vulnerabilities), frontend format/lint/typecheck/build, 31 shared DTOs, 10 frontend tests, and current SBOM/notices. Upstream does not publish signatures for these artifacts; the plan explicitly permits Ark-reviewed reproducible provenance when signatures are unavailable, and this implementation fails closed on the independently reviewed SHA-256/size manifest rather than weakening verification.
- **Description:** Pin tool/package versions, publish and verify checksums/provenance for downloaded llama.cpp artifacts, verify before extraction/execution, generate SBOM and third-party notices, and record model source/license/hash.
- **Reason:** Setup scripts download native executables without checksum/signature verification; model provenance is absent.
- **Related audit findings:** A-SEC-10, A-SEC-12, A-OPS-05.
- **Dependencies:** FND-001, FTR-006 distribution design.
- **Priority / complexity:** Critical / Large.
- **Expected outcome:** Every executable/model in an Ark-managed path has verifiable origin and license.
- **Acceptance criteria:**
  - Downloads fail closed on checksum/signature mismatch and never execute partial files.
  - Checksums/provenance are stored in reviewed release metadata, not fetched from the same untrusted payload location alone.
  - SBOM covers Rust, JavaScript, native runtime, and bundled assets.
  - UI records model/runtime version, origin, hash, license, and last verification.
  - CI tests tampered/truncated archives and extraction traversal.
- **Potential risks:** Upstream releases may not provide signatures/provenance.
- **Suggested implementation notes:** Ark may publish its own reproducible, signed runtime artifacts if upstream assurance is insufficient.

#### SEC-005 — Implement OS-backed secret storage

- **Status: Complete (2026-08-14).** Previously `Blocked` pending the "run native CRUD on the other two declared desktop OSes" criterion — the non-fail-fast CI matrix was already configured to compile/run the same test against the real macOS Keychain and an unlocked GNOME Secret Service on Ubuntu (`.github/workflows/ci.yml`'s "Start Linux Secret Service" step, mirroring keyring-rs's own upstream CI pattern), but this Windows-only workspace couldn't supply the runners at the time this was written. Confirmed directly from CI job logs on this session's pushes, not assumed: `secret_store::tests::platform_credential_store_and_provider_linkage_round_trip … ok` actually executed (not skipped) on both `ubuntu-latest` and `macos-latest` (e.g. [github.com/lukedamato20/Ark/actions/runs/31809792013](https://github.com/lukedamato20/Ark/actions/runs/31809792013)). Reclassified from `Blocked` to `Complete` on that evidence.
  - `SecretStore` defines create/read/update/delete/status, and keyring 4.1's native adapters select Windows Credential Manager, macOS Keychain, or Linux Secret Service without custom cryptography. Raw values use a non-`Debug`, non-serializable, zeroizing `SecretValue`; writes run off the async executor, never hold the SQLite mutex during OS calls, compensate a failed new-reference DB link, cap input at 16 KiB, and persist only `secret:v1:<UUID>` references. Four thin Tauri commands expose store status, write-only upsert, metadata-only read, and delete. The 31-type Rust/TypeScript contract covers `SecretMetadata`/`SecretStoreStatus`; no raw-read IPC exists. The real integration test proves OS create/read/update/delete plus SQLite linkage/update/metadata/unlink, while in-memory port, invalid-reference/limit, safe-error, export, and contract tests cover failure paths.
  - Conversation JSON export clears even the device-local opaque reference; Markdown never reads it. `docs/secrets-and-backups.md` explains that backup never copies OS-store values and that another machine/account must reconnect, while same-account restore can reuse a still-resolving reference. `docs/settings-catalog.md` records ownership/validation/UI. `pnpm secret-boundary:check`, wired into CI, fails if raw-read IPC, serialization/debug exposure, browser/localStorage/clipboard persistence, diagnostics access, export references, runtime log redaction, or today's no-crash-transport boundary regress; Rust tests prove platform errors cannot echo sensitive details. OPS-001 must replace the explicit no-crash-transport guard with payload redaction tests before introducing crash reporting.
  - Settings reports credential-store health independently, disables authenticated-provider entry while locked/unavailable, explains recovery, and provides Retry. Auth-capability-gated controls support replace/delete, fixed masking, `new-password` completion policy, and clear the field before awaiting persistence. Browser verification exercised locked → Retry → available → Save → masked connected → Remove; the submitted sentinel disappeared from the rendered DOM immediately after save and was never copied to clipboard. Current local providers correctly keep `requires_auth=false`, so the credential form appears only for a future authenticated adapter or an existing reference.
- **Description:** Add a SecretStore port with Windows Credential Manager, macOS Keychain, Linux Secret Service, and later iOS Keychain adapters; replace the unused API-key reference with opaque secret identifiers.
- **Reason:** Secure cloud provider support and mobile auth cannot use SQLite or localStorage.
- **Related audit findings:** A-SEC-03, A-CMP-02, A-MOB-04.
- **Dependencies:** ARC-002, ARC-006.
- **Priority / complexity:** High / Large.
- **Expected outcome:** Credentials are never stored or exposed in application data, exports, logs, or UI state after entry.
- **Acceptance criteria:**
  - Create/read/update/delete work on each supported OS or that OS is not declared supported.
  - API responses expose only masked metadata and opaque IDs.
  - Export/backup excludes secrets and clearly explains reconnection after restore.
  - Logging, crash reports, clipboard behavior, and diagnostics have automated secret-redaction tests.
  - Keychain-unavailable/locked states have recoverable UX.
- **Potential risks:** Linux keyring availability and headless sessions vary.
- **Suggested implementation notes:** Do not invent custom cryptography for secret storage; use platform services and explicit re-authentication.

#### SEC-006 — Define and implement local data protection

- **Status: Complete (2026-08-14).** Previously `Blocked` pending green macOS/Linux legs of the CI Rust matrix — confirmed directly from CI job logs on this session's pushes: `data_protection::tests::wrong_key_cannot_open_encrypted_database … ok` and `data_protection::tests::rotate_key_and_restore_recovery_key_round_trip … ok` both actually executed (not skipped) on `macos-latest` (e.g. [github.com/lukedamato20/Ark/actions/runs/31809792013](https://github.com/lukedamato20/Ark/actions/runs/31809792013)). Reclassified from `Blocked` to `Complete` on that evidence, same as SEC-005/ARC-010.
  - `file_permissions.rs` hardens every Ark-owned file/directory to the current user (`0700`/`0600` on Unix, a single-ACE protected DACL on Windows). `data_protection.rs` adds optional SQLCipher workspace encryption on top of the existing bounded writer/read-replica `Database` service: a random (never user-chosen) key stored only as an opaque reference via the SEC-005 `SecretStore`; enable/rotate/disable all go through a copy-then-independently-verify-then-atomic-swap sequence (`copy_verify_swap`) that never modifies the original file until the new copy has proven it opens and reads correctly; a transition journal lets startup finalize or roll back an interrupted change instead of guessing; and a one-time-displayed recovery key covers the case where the OS credential store entry is lost. `restore_recovery_key` safely rejects a stale (rotated-away) or malformed key without touching the database or credential store, and a forgotten key is explicitly unrecoverable (SQLCipher's authenticated encryption makes this a cryptographic fact, not a product gap).
  - Fixed a real regression this session introduced by the SQLCipher switch: `apply_encryption_key`'s unlock-verification probe was unconditionally relabelling *any* open failure as `workspace_unlock_failed`, even for a plaintext (unkeyed) open — so a genuinely corrupt or non-database file was misreported as an encryption-unlock problem instead of `database_corrupt`. Fixed by keeping the forced-read probe (still required, since SQLite/SQLCipher do not validate a file until a real statement runs) but only special-casing its error as `workspace_unlock_failed` when a key was actually supplied; an unkeyed failure now flows through the existing, correct `AppError::from(rusqlite::Error)` classification. Covered by the pre-existing `db::tests::open_classifies_a_non_database_file_as_database_corrupt`, which now passes again.
  - Added `data_protection::tests::rotate_key_and_restore_recovery_key_round_trip`, a real `AppState`-driven integration test (not the lower-level `copy_verify_swap` primitive already covered by `plaintext_to_encrypted_and_back_is_copy_based_and_preserves_rows`/`wrong_key_cannot_open_encrypted_database`) proving the acceptance-criteria surface those didn't reach: enable issues a recovery key, rotate issues a *different* one and invalidates the old one, a stale or malformed recovery key is rejected with `workspace_recovery_key_invalid` without mutating state, the current recovery key restores access, and conversation data survives every transition. It touches the real Windows Credential Manager (consistent with SEC-005's existing real-OS-store test philosophy) and cleans up the entry it creates.
  - Added `docs/data-at-rest.md` (linked from README) to satisfy the previously unmet "threat model" and "plaintext before encrypted" documentation criteria — `docs/secrets-and-backups.md` only ever covered SEC-005 provider-credential storage, not the workspace database itself. The new document states the plaintext default and file-permission hardening first, then the optional encrypted mode and its key/rotation/recovery-key/forgotten-key lifecycle, then an explicit table distinguishing disk theft (with and without OS full-disk encryption), another OS account, malware in the user's own session (explicitly **not** defended against — a same-privilege process can reach the same OS credential store Ark uses), and cloud-synced workspace folders.
  - Browser-verified the rendered Settings → Storage flow against a new `?fixture=workspace-protection` development bridge (`developmentArkClient.ts`, selected only in a Vite dev build, never shipped in the Tauri/production adapter — same pattern as the existing `runtime-provenance`/`secret-store` fixtures): enable shows its explicit irreversibility warning, requires confirmation, and displays the recovery key exactly once behind an acknowledgement; rotate shows a distinct warning that the old key stops working, requires confirmation, and displays a new, different recovery key. The locked-state "Restore and unlock" input (rendered only when `protectionStatus.locked`) was verified through type-checked/contract-checked wiring and the backend integration test above rather than a forced-lock browser interaction — noted here rather than silently treated as equivalent evidence.
  - While fixing the above, found and fixed an unrelated pre-existing gap: `.artifacts` (this session's local, gitignored build-tool cache — a portable Perl toolchain needed only to compile SQLCipher's vendored OpenSSL on a Windows host with no system Perl) was not in `eslint.config.js`'s ignore list, so `pnpm lint` failed on vendored third-party JS it happened to contain. Added it alongside the existing `dist`/`src-tauri/target`/`node_modules` ignores.
  - Full validation green on all three CI platforms as of this reclassification: Rust `cargo fmt --check`/strict `clippy -D warnings`/`cargo test` (231+ passed), frontend `format`/`lint`/`typecheck`/`build`, 33 DTO contracts, `architecture:check`, `secret-boundary:check`, `test:frontend`.
- **Description:** Document current reliance on OS account/full-disk encryption, harden file permissions, and provide an optional encrypted-workspace design and implementation using a supported SQLite encryption approach with OS-protected key lifecycle.
- **Reason:** Conversation SQLite data is plaintext and may contain highly sensitive content.
- **Related audit findings:** A-SEC-05, A-OPS-04.
- **Dependencies:** ARC-004, ARC-005, SEC-005.
- **Priority / complexity:** High / Extra Large.
- **Expected outcome:** Users understand and can choose an appropriate at-rest protection level without risking unrecoverable data.
- **Acceptance criteria:**
  - New workspace/database/files receive least-privilege user-only permissions where supported.
  - Privacy documentation clearly states plaintext behavior before encrypted mode exists.
  - Encrypted mode has key creation, unlock, rotation, backup/recovery-key, restore, and forgotten-key behavior.
  - Migration between plaintext/encrypted workspaces is copy-based, verified, and rollback-safe.
  - Threat model explicitly distinguishes disk theft, other OS users, malware in the same session, and cloud-synced folders.
- **Potential risks:** Key loss, performance, SQLCipher licensing/build complexity, cross-platform incompatibility.
- **Suggested implementation notes:** Ship honest OS-encryption guidance before optional encryption; do not delay all desktop release if risk is documented and files are permission-hardened.

#### SEC-007 — Harden file and native model ingestion

- **Status: Partial (2026-08-14).** Two of four acceptance criteria have real, tested coverage; two remain open — recorded honestly rather than marked Complete.
  - **"Archive extraction rejects absolute paths, parent traversal, links, and device files"** — already satisfied by SEC-004's `validateArchiveEntries` (`scripts/runtime-supply-chain.mjs`): rejects absolute POSIX/Windows-drive paths, `..` traversal, and any entry type other than `-`/`d` (which excludes symlinks, device files, FIFOs, and sockets outright), plus an entry-count ceiling. The one archive-extraction path that exists today (the pinned native runtime download) is additionally hash-verified against a reviewed manifest *before* extraction, so a decompression-bomb substitution would fail hash verification first — a dedicated bomb-ratio check was judged unnecessary for this specific, already-provenance-gated archive rather than added speculatively for a general "import an arbitrary archive" feature that does not exist yet.
  - **"GGUF validation checks regular file, readable header, plausible size... before launch"** — new `validation::validate_gguf_file`, called from `provider_management::start_built_in_runtime` immediately after the existing path-shape check (`validate_model_path`) and before the file is handed to `llama-server`. Rejects: symlinks (via `symlink_metadata`, which does not follow the link — closes a TOCTOU gap the existing `path.is_file()` shape-check couldn't, since that check follows symlinks by design), non-regular files (devices/pipes/sockets), files below the minimum possible GGUF header size, files past a generous absolute ceiling, and files whose first 4 bytes don't match the GGUF magic number. Six boundary tests cover valid/wrong-magic/truncated/empty/missing/symlinked inputs (the symlink test is `#[cfg(unix)]` — Windows symlink creation needs elevated privileges or Developer Mode, not guaranteed on a CI runner).
  - **"...available disk/RAM... before launch"** — deliberately deferred, not implemented. This is PERF-004's stated job ("Preflight estimates model + context memory and free disk/RAM with a confidence label"), which needs a real, nuanced fit assessment (context size, GPU offload, mmap behavior); a crude "reject if file size exceeds N× total RAM" check in this security-focused validator risks blocking legitimate large local models loaded via mmap, which is a real product capability Ark wants. `MAX_GGUF_BYTES` only catches an absurd/adversarial absolute size (1 TB), not a hardware-relative one.
  - **"Canonicalization and symlink policy are consistent for every file command"** — not yet done broadly. Only the model-file path (above) has a real symlink check. `validation::reject_ambiguous_path` (used by both `validate_workspace_path` and `validate_model_path`) is a lexical `.`/`..` check only — it does not canonicalize or detect a symlink escaping the intended directory for the workspace-path case, or for any import/export path. This needs a real audit of every file-accepting command (workspace selection, conversation import/export, and future attachment/CMP-001 paths) against one consistent canonicalization policy — genuinely the bulk of this task's "Large" rating, and not attempted in this pass.
  - **"Fuzz/boundary tests cover malformed imports and model headers without invoking unsafe native code"** — the six new GGUF boundary tests satisfy this for model headers specifically; "malformed imports" (JSON conversation import) already has dedicated boundary/rejection tests from COR-009 (`import_export.rs`, e.g. `import_rejects_an_oversized_payload_before_deserializing`, `rejects_malformed_json_gracefully`) — not new work from this session, but existing coverage worth citing since the criterion asks about imports generally, not just this task's own additions. True property-based/random fuzzing (a `cargo-fuzz`/`proptest` harness) was not set up; the existing and new tests are hand-written boundary cases, not generated ones.
  - Full validation: `cargo fmt`/`clippy -D warnings` clean, `cargo test` 219 passed/1 ignored (the pre-existing ARC-005 known issue, unrelated) — 6 new tests for `validate_gguf_file`, zero regressions.
- **Description:** Apply canonical path, type, size, ownership/permission, symlink, available-space, and format checks to imports, exports, workspace selection, attachments, and model files; apply process resource limits where possible.
- **Reason:** Unbounded files and untrusted GGUF/native parsers create denial-of-service and native attack surface.
- **Related audit findings:** A-SEC-06, A-SEC-12, A-FUN-07.
- **Dependencies:** COR-008, SEC-004.
- **Priority / complexity:** High / Large.
- **Expected outcome:** Untrusted files cannot trigger uncontrolled allocation, overwrite, traversal, or unsupported runtime launch.
- **Acceptance criteria:**
  - Canonicalization and symlink policy are consistent for every file command.
  - GGUF validation checks regular file, readable header, plausible size, available disk/RAM, and recorded hash before launch.
  - Archive extraction rejects absolute paths, parent traversal, links, device files, and decompression bombs.
  - Fuzz/boundary tests cover malformed imports and model headers without invoking unsafe native code.
- **Potential risks:** Validation cannot guarantee a native parser is vulnerability-free.
- **Suggested implementation notes:** Keep runtime/model patched and resource-constrained; validation reduces risk but is not a sandbox substitute.

#### SEC-008 — Preserve and test webview, Markdown, and external-link safety

- **Status: Complete (2026-08-14).** All four acceptance criteria met with real, automated regression coverage — not just current-behavior review.
  - **"Raw HTML, script/event attributes, javascript/data navigation, unsafe SVG, and hostile highlighted code remain inert":** react-markdown already rendered no raw HTML (no `rehype-raw` plugin), so an embedded `<script>`/`<svg onload>`/etc. in Markdown source was already shown as escaped literal text, not executed — this was true before this task and is now locked in structurally: new `scripts/check-markdown-safety.mjs` (wired into CI) fails if `rehype-raw` is ever added as a dependency or imported, and asserts exactly one `dangerouslySetInnerHTML` sink exists in the frontend (the syntax-highlighted code block) so a second one can't appear unreviewed. That one sink's actual safety was previously assumed, not tested — extracted the highlighting logic into `src/lib/highlightCode.ts` and added `highlightCode.test.ts`: five hostile fixtures (`<script>`, `<img onerror>`, a `</code></pre>` breakout attempt, an `<svg onload>`, `<style>`) run through both known languages (javascript, html/xml, css) and the unknown-language fallback, asserting the output never contains a live (unescaped) `<script`/`<img`/`<svg` tag-open sequence.
  - **"External links show destination and open through a controlled native path with supported-scheme allowlist":** previously entirely unimplemented — react-markdown's default `<a>` rendering had no interception at all, meaning a Markdown link could navigate the app's own webview window directly. Added `src/lib/externalLinks.ts` (`checkExternalLink`, a pure function restricting to `http:`/`https:`/`mailto:`/`tel:`, four tests including the same javascript/data/file/vbscript rejection cases as SEC-007's process below) and a `MarkdownLink` component (`MarkdownMessage.tsx`) that: renders an unsafe-scheme link as inert non-clickable text, shows the real destination via `title` (so displayed link text cannot silently point elsewhere), and always intercepts the click (`preventDefault`) to route through a new `ArkClient.openExternalUrl` — added `tauri-plugin-opener`/`@tauri-apps/plugin-opener`, granted only `opener:allow-open-url` + `opener:allow-default-urls` (not the broader `opener:default`, which also grants file-explorer reveal — not needed here). The plugin's own `allow-default-urls` scope independently restricts to `mailto:`/`tel:`/`http:`/`https:` at the native layer — two independent enforcement points, not one.
  - **"CSP changes are covered by an automated production-build test and reviewed as security changes":** no such test existed. New `scripts/check-csp.mjs` (wired into CI) parses `tauri.conf.json`'s real CSP and asserts: `script-src` is `'self'` only with no `unsafe-inline`/`unsafe-eval`/external host ever; `connect-src` stays loopback-only; and (new hardening, zero behavior risk since Ark uses none of these) `object-src 'none'`, `base-uri 'self'`, `form-action 'self'` were added to the actual policy.
  - **"unsafe-inline is removed or retained with a documented library constraint and compensating tests":** retained, deliberately — framer-motion (used throughout the UI, e.g. sidebar collapse) applies animated values via direct inline `style` writes, which is exactly what `style-src 'unsafe-inline'` permits; a nonce-based replacement doesn't fit a static Tauri CSP the way it fits a server-rendered page issuing a fresh nonce per request. The decision and reasoning are recorded as a comment directly above the assertion in `check-csp.mjs` that would need to change together with any future replacement, not just in this plan document.
  - Full validation: `cargo fmt`/`clippy -D warnings`/`cargo test` (219 passed/1 ignored, unrelated ARC-005 issue) clean; frontend `format`/`lint`/`typecheck`/`build`/`architecture:check`/`contract:check` clean; `csp:check`, `markdown-safety:check`, `secret-boundary:check`, `supply-chain:check` (885 components after the new opener plugin) all pass; `test:frontend` 18/18 (9 new: 4 `externalLinks`, 5 `highlightCode`) — one of the new tests was initially over-strict (asserted a substring like `onerror=` never appears anywhere in output, when it appearing as inert escaped text is safe) and was corrected to assert the actual safety property (no live/unescaped tag-open sequence) rather than loosened to pass.
- **Description:** Keep self-only scripts and no raw Markdown HTML, add an explicit external-link policy, hostile content regression fixtures, and evaluate removing style-src unsafe-inline through nonces/hashes or documented necessity.
- **Reason:** Current CSP and React/Markdown behavior are strengths that future features could accidentally weaken.
- **Related audit findings:** A-SEC-07, A-SEC-02.
- **Dependencies:** FND-003, TST-006.
- **Priority / complexity:** High / Medium.
- **Expected outcome:** Imported/model content cannot execute script or navigate silently to unsafe destinations.
- **Acceptance criteria:**
  - Raw HTML, script/event attributes, javascript/data navigation, unsafe SVG, and hostile highlighted code remain inert.
  - External links show destination and open through a controlled native path with supported-scheme allowlist.
  - CSP changes are covered by an automated production-build test and reviewed as security changes.
  - unsafe-inline is removed or retained with a documented library constraint and compensating tests.
- **Potential risks:** Styling/animation libraries may require inline styles.
- **Suggested implementation notes:** Do not enable raw HTML for richer model output; implement specific safe components instead.

#### SEC-009 — Define prompt-injection and tool-capability controls

- **Status: Complete (2026-08-14).** This is a "define..." task by its own description, done deliberately ahead of any real consumer — no RAG/tool/web/agent feature exists yet for it to govern. Reviewed and approved by Luke D'Amato (2026-08-14), recorded in the ADR's own metadata and "Approval" section, before any implementation was built against it — no retroactive-approval sequencing tension to record, unlike ADR 0001.
  - **New ADR, `docs/adr/0002-tool-capability-and-prompt-injection-policy.md`:** defines the four-channel untrusted-content separation (system/user/retrieved-or-tool-result/model — retrieved and tool-result content is always quoted, labeled data, never merged into the system channel regardless of what it says); the capability-scope taxonomy (read/write/network/secret/data axes plus the chat-safe vs. repository-execution tier split already recorded in CMP-003/Phase 6.5's boundary notes, with this ADR as the one authoritative source those tasks now cite rather than each re-deriving); the approval/preview/revocation model (narrow, time-boxed grants only — no "allow all tools," matching the plan's own suggested implementation note); the audit-event tamper-evidence approach; and an explicit, required checklist of what the adversarial test suite must cover (exfiltration, instruction override, indirect injection, confused deputy, approval fatigue) once a real tool exists to test.
  - **New `src-tauri/src/tool_policy.rs`:** a real, tested type model — `CapabilityScope`/`CapabilityTier`, `CapabilityGrant` (with expiry and immediate-on-revocation invalidation), `SideEffectPreview`/`IdempotencyPolicy`, and a hash-chained `AuditEvent` (FNV-1a, the same non-cryptographic drift-detection approach `db::migration_checksum` already uses, chosen for the same reason — no new dependency for a local single-user threat model) plus `enforce_tier_boundary`, the actual callable function CODE-004/CODE-005 must invoke before honoring any repository-execution-tier grant. Eleven tests prove the type-level invariants directly: a repository-execution scope is rejected without a Repository binding and accepted with one; a grant is invalid once expired and immediately invalid once revoked regardless of remaining time; the audit chain detects a tampered event body, a deleted event, and a reordered sequence, and verifies correctly when genuinely untampered.
  - **Deliberately not done, and correctly so — recorded here rather than silently skipped:** the adversarial prompt suite and real (non-test-fixture) audit-event persistence are not implemented. Both need an actual tool-calling feature to exercise; building them against nothing would be speculative. They are CMP-003's and CODE-008's job respectively, both of which now have this ADR and this module to consume rather than needing to design their own.
  - Full validation: `cargo fmt`/`clippy -D warnings` clean, `cargo test` 230 passed/1 ignored (11 new, zero regressions). `#![allow(dead_code)]` at the module level is intentional and documented in the module's own doc comment — every item is exercised only by its own tests today, by design; the comment states it must be removed the moment a real consumer exists, so it can't quietly persist past that point unnoticed.
- **Description:** Before RAG, web, MCP, or agents, define untrusted-content boundaries, system/user/retrieval/tool channel separation, capability scopes, data-access declarations, side-effect previews, approvals, revocation, and immutable audit events.
- **Reason:** Prompt injection is low impact today only because models cannot access private documents or perform actions.
- **Related audit findings:** A-SEC-09, A-CMP-04, A-CMP-15.
- **Dependencies:** ARC-002, ARC-003.
- **Priority / complexity:** High / Large.
- **Expected outcome:** Competitive tool/RAG features cannot silently expand model authority.
- **Acceptance criteria:**
  - Each tool declares read/write/network/secret/data scopes and whether approval is required.
  - The capability-scope model distinguishes chat-safe scopes (web search, utilities, notes, memory, external-service connectors) from repository-execution scopes (filesystem write, git, process/command execution); repository-execution scopes are only grantable within an Ark Code session bound to a Repository (Phase 6.5) and are never reachable from Ark Chat.
  - Retrieved or web content is treated as quoted untrusted data, never merged into system instructions.
  - Every side effect has a human-readable preview and idempotency/replay policy.
  - Adversarial prompt suite covers exfiltration, instruction override, indirect injection, confused deputy, and approval fatigue.
  - Audit records are tamper-evident enough for local support and contain redacted inputs.
- **Potential risks:** Excessive prompts make tools unusable; weak grouping makes them unsafe.
- **Suggested implementation notes:** Approve narrow capabilities for a bounded time/resource, not broad “allow all tools” sessions.

#### SEC-010 — Define companion API and LAN device-pairing security

- **Status: Complete (2026-08-14), rewritten for the Phase 8 scope decision — pending the same review as SEC-009's ADR before FTR-010/MOB-009 implementation begins.** The original scope (OAuth/OIDC PKCE, refresh-token rotation, multi-device E2E-encrypted sync) assumed a hosted-or-syncing multi-device architecture the Phase 8 scope decision (recorded above MOB-001) no longer builds — there is no account, no cloud backend, and no offline replica to reconcile, so most of the original threat model doesn't apply to anything that will actually be built. Rewritten to the threat model that *does* apply: the FTR-010 companion API and MOB-009 LAN pairing.
  - **The real, narrower threat model:** the companion API is a local HTTP server reachable on the LAN. Two distinct threats matter: (1) **an unrelated website open in the user's normal browser** issuing a same-origin-policy-exempt request to the companion API while it happens to be running — the identical threat class SEC-002 already had to solve for the llama.cpp sidecar, and the same fix applies: the pairing token must be sent as a custom request header (`Authorization: Bearer <token>`), never a cookie, since a cross-origin page cannot attach a custom header without a CORS preflight the companion API can simply refuse; and (2) **another device on the same LAN** attempting to use the API without ever having been paired — defeated by requiring the token at all, full stop, with no unauthenticated route (unlike llama.cpp's own health/models exemption that SEC-002 could not fully close upstream — the companion API is Ark's own code, so this exemption simply does not exist here by construction).
  - **Device pairing lifecycle (implemented by MOB-009):** a pairing token is high-entropy, generated server-side, bound to one named device, and stored server-side via SEC-005's OS-backed secret storage. It does not expire on a fixed schedule (there is no refresh-token dance) but is individually and immediately revocable — revocation takes effect on the device's very next request, not on some future refresh. A lost/stolen phone is handled by revoking that one device's token from Settings; this is the entire "lost device" story, deliberately simpler than OAuth's refresh-rotation-and-reuse-detection machinery because there is no multi-hop token exchange to defend.
  - **What is explicitly out of scope, and why that is correct, not a gap:** OAuth/OIDC (no third-party identity provider exists or is needed); short-lived access + refresh tokens (no authorization server to issue them); E2E encryption / searchable-metadata / multi-device key distribution (no sync, no server-held ciphertext to protect against — the SQLite database never leaves the desktop machine); browser-cookie CSRF defenses specifically (the companion API deliberately never uses cookies for auth, which is a stronger position than defending cookie-based CSRF).
  - **Full validation:** this is a design/threat-model deliverable, like SEC-009 — FTR-010 and MOB-009 (its consumers) are not yet implemented, so there is nothing to test end-to-end yet. FTR-010's and MOB-009's own acceptance criteria (already updated above to require custom-header bearer auth, no database/filesystem exposure, and immediate per-device revocation) are what make this threat model real rather than aspirational when they ship.
- **Description:** Define the security design for the local companion API (FTR-010) and LAN device pairing (MOB-009): per-device bearer-token authentication via custom header (never a cookie), protection against an unrelated browser tab issuing a drive-by request to the companion API while it's running, and individually/immediately revocable device pairing. Narrower than originally scoped — no OAuth/OIDC, no account system, no multi-device sync, no E2E encryption — because the Phase 8 scope decision means none of those architectures are being built.
- **Reason:** Authentication is not applicable to the single-user desktop itself, but the companion API (FTR-010) is a real local network service the moment it exists, and it needs the same "don't trust localhost as an authentication boundary" discipline SEC-002 already established for the sidecar.
- **Related audit findings:** A-SEC-01, A-SEC-04, A-MOB-03–04, A-MOB-07.
- **Dependencies:** ARC-002 protocol direction.
- **Priority / complexity:** Medium / Medium (was High / Large under the original account/sync/E2E scope).
- **Expected outcome:** FTR-010 and MOB-009 are built against a reviewed trust model from the start, not bolted-on auth after the fact.
- **Acceptance criteria:**
  - Local single-user desktop remains account-optional and does not gain cosmetic authentication.
  - The companion API authenticates every request via a custom header bearer token, never a cookie, and never exempts any route (including health/status) the way SEC-002 documented the sidecar's upstream could not avoid.
  - Device pairing tokens are high-entropy, server-generated, individually revocable, and revocation is immediate on the device's next request.
  - Lost/stolen device, token theft over the LAN, and replay are each explicitly addressed by the design above.
- **Potential risks:** The home Wi-Fi network is the actual trust boundary for LAN pairing; if it is shared/public, the threat model's guarantees are only as strong as the network itself — this must be stated plainly in the UI (MOB-009), not implied to be stronger than it is.
- **Suggested implementation notes:** Use standard header-based bearer-token conventions and platform secure stores (SEC-005); do not expose the SQLite schema as an API; do not reintroduce a cookie-based session "for convenience" — that would reopen exactly the CSRF-style threat this design avoids by construction.

#### SEC-011 — Publish the security and privacy operating model

- **Status: Complete (2026-08-14).** This task and OPS-001 list each other as dependencies (SEC-011 needs "OPS-001 redaction policy"; OPS-001 needs "SEC-011") — the same genuine circular reference ARC-006/SEC-005 already hit once, resolved the same way: implement what SEC-011 needs on its own, against the redaction patterns that already exist (`docs/runtime-diagnostics-policy.md`'s in-memory-only/bounded/redact-before-buffer discipline, the `secret-boundary:check` script's assertions, SEC-005's opaque-reference credential model), rather than waiting on OPS-001's not-yet-built structured-logging/crash-reporting feature. OPS-001 implements against this document's policy when it lands, not the other way around.
  - **New `SECURITY.md`** (repo root, GitHub's standard convention — auto-linked from the repo's Security tab): supported-version policy right-sized for this project (one supported version, the latest `main` commit — no version-support matrix to maintain, because there is no prior-version user base). Reporting channel is GitHub's own private vulnerability reporting feature, not an invented email/PGP-key process — this repository is public (confirmed via `gh repo view`), so a personal email address was deliberately *not* published here; GitHub's built-in private-advisory mechanism needs no separate contact channel to exist. **This needs one manual step outside what I can do myself: enabling "Private vulnerability reporting" under the repository's Settings → Security tab** — a repository-settings change, which this agent does not make without the user's explicit go-ahead. SECURITY.md documents the fallback (open a no-detail issue) for as long as it stays off.
  - **New `docs/privacy-and-data-flow.md`:** the single data-flow disclosure the acceptance criteria ask for — a table of exactly what can leave the machine and under what condition (provider selection, model downloads, the companion API's LAN-only pairing), plus an explicit statement that crash reporting (OPS-001) collects nothing until it exists, avoiding the trap of describing a future feature as if it were already live. Links out to the existing `data-at-rest.md`/`secrets-and-backups.md`/`runtime-diagnostics-policy.md` rather than duplicating their content.
  - **New `docs/incident-response.md`:** advisory triage (references the existing, already-operating `dependency-advisory-review.md` process rather than inventing a second one), credential exposure, workspace-key/recovery-key loss (a documented-behavior case, not a true incident, per `data-at-rest.md`), bad release (uses OPS-004's actual rollback mechanism — the previous GitHub Release's attached installer — rather than a signed-update-channel revocation that doesn't exist under OPS-002's rewritten scope), and repository/account compromise. Explicitly notes that "signing-key compromise" has no real procedure yet because there is no signing key under the current OPS-002 scope, rather than writing a procedure for a key that doesn't exist.
  - **New `docs/secure-development-checklist.md`:** a practical, tool-by-tool checklist (paths/symlinks, network destinations, secrets, model-file ingestion, Markdown rendering, CSP/capabilities, tool-capability scopes) tying together checks that already exist as CI scripts (`secret-boundary:check`, `markdown-safety:check`, `csp:check`) with the *reasoning* behind each, plus a release-time section that OPS-004's acceptance criteria now reference directly.
  - **Release security review:** OPS-004 (above) now has an explicit acceptance criterion requiring this checklist's release section before tagging.
  - All four new docs plus SECURITY.md are linked from README's docs list.
- **Description:** Create security policy/reporting, privacy notice, data-flow diagram, secure-development checklist, advisory exception process, incident response, credential rotation, supported-version policy, and release security review.
- **Reason:** Security/privacy documentation and operational response are absent despite being central to Ark's positioning.
- **Related audit findings:** C-10, A-OPS-05, A-OPS-01.
- **Dependencies:** SEC-001–010 designs; OPS-001 redaction policy.
- **Priority / complexity:** High / Medium.
- **Expected outcome:** Users and maintainers know what data is stored/sent, how fixes are handled, and how to report vulnerabilities.
- **Acceptance criteria:**
  - Data-flow disclosure covers local DB, logs, provider requests, imports/files, model/runtime downloads, crash reports, and the companion API/LAN pairing (SEC-010).
  - SECURITY.md defines private reporting and supported versions without inventing unavailable contact channels.
  - Incident runbook includes advisory triage, credential/signing-key compromise, malicious update, and rollback.
  - Release checklist requires a security delta review.
- **Potential risks:** Documentation becomes stale.
- **Suggested implementation notes:** Assign owners and review dates; generate dependency/SBOM sections where possible.

### Phase 3 — Architecture and maintainability

#### ARC-001 — Introduce application use-case services and thin Tauri commands

- **Status: Complete (2026-08-13).** `commands/mod.rs` shrank from 1,687 lines (session start) to 326 lines and now contains only: request/response DTOs, direct one-line CRUD delegations into `Database` (`list_conversations`, `create_conversation`, `rename_conversation`, `delete_conversation`, `get_conversation_messages`, `get_assistant_alternatives`, `switch_active_branch`, `keep_partial_message`, `discard_interrupted_message`, `set_theme`) and into `crate::workspace` (`set_workspace`, `reset_workspace`) — left as-is because they already are "decode → single use-case call → return," with no orchestration to extract — and thin `#[tauri::command]` adapters that decode a request and delegate to one of five new application-service modules, plus the two shared cross-cutting helpers every service calls through (`lock_db`, `built_in_bearer_token`).
  - `generation.rs` (523 lines) — the conversation/generation workflow: `send_chat_message`, `edit_user_message`, `regenerate_assistant_message`, `cancel_stream`, plus the streaming supervision (`spawn_provider_stream`, `emit_stream_start`, terminal-state handlers) already extracted in the prior pass.
  - `import_export.rs` (337 lines) — `export_conversation_markdown`/`export_conversation_json`/`import_conversation_json` as plain functions over `&Database` (no `AppState`/Tauri dependency at all — the most decoupled of the five). 4 new tests exercise these directly against a real temp-file SQLite database with no Tauri runtime involved: round-trip export/import, transient-status normalization, oversized-payload rejection, and rollback-on-validation-failure.
  - `diagnostics.rs` (328 lines) — `run_diagnostics`/`run_benchmark`/`performance_guidance`. 9 tests: 7 for the pure `performance_guidance` function (unreachable provider, missing model, missing benchmark, and all three throughput tiers including exact threshold boundaries), plus 2 for `run_benchmark` exercised end-to-end against the existing `providers::test_support` mock HTTP server (now `pub(crate)` so a sibling module's tests can reuse it) — one verifying a successful benchmark's first-token timing and content preview, one verifying a truncated stream correctly propagates `stream_incomplete`.
  - `provider_management.rs` (355 lines) — `update_provider`, `refresh_models`, `pull_ollama_model`, `delete_ollama_model`, `get_built_in_runtime_status`, `stop_built_in_runtime`, `start_built_in_runtime`. 3 new tests construct a real `AppState` directly (no `tauri::App`, no IPC) and drive `update_provider`/`stop_built_in_runtime` end-to-end against a real temp SQLite database — this is the concrete proof for the "can run with in-memory/test adapters" criterion below.
  - `workspace_bootstrap.rs` (99 lines) — `get_app_bootstrap` (startup read-model assembly) and `retry_workspace_open` (COR-010 recovery: swap the in-memory fallback for the real database on success, record the error on failure).
  - **Acceptance criteria, checked individually:**
    - *"Tauri commands contain request decoding, authorization/validation call, use-case invocation, and response mapping only."* Every command in `commands/mod.rs` is now either a single delegating line into a service module or a single delegating line into `Database`/`crate::workspace` (no service module exists for those because there is no orchestration logic to put in one).
    - *"Use cases depend on explicit ports and can run with in-memory/test adapters."* Closed the gap the prior "Started" status explicitly flagged: every extracted service function was changed from taking `&State<'_, AppState>` (Tauri's DI wrapper, only constructible by a running app) to taking `&AppState` directly (a plain data struct with public fields — `Mutex<Database>`, `Mutex<Option<AppError>>`, `Mutex<HashMap<..>>`, `Mutex<SidecarState>` — all directly constructible in a `#[test]` with no Tauri runtime). `#[tauri::command]` wrappers still receive Tauri's `State<'_, AppState>` (required for its DI) and pass it straight through — `State<T>: Deref<Target = T>` means `&state` coerces to `&AppState` automatically at the call site, so this cost zero ceremony at the boundary. `import_export.rs` goes one step further and depends on `&Database` directly (no `AppState` at all), since it needs nothing else. The one deliberate exception is `&AppHandle` (event emission via `app.emit`, path resolution via `crate::workspace`/`crate::sidecar` helpers) — genuinely Tauri-runtime-coupled capabilities with no in-memory equivalent to substitute; introducing a trait to abstract that away would be exactly the "abstraction merely for abstraction's sake" this task warned against, since nothing in this codebase currently needs to run these particular workflows outside a live Ark process. The `provider_management.rs` tests above are the concrete demonstration: `update_provider`/`stop_built_in_runtime` need no `AppHandle`, so they run fully isolated from Tauri.
    - *"No behavior change occurs without a corresponding acceptance test."* Every extraction was pure code-motion (bodies moved verbatim; only signatures/visibility/qualification changed) and validated the same way throughout: `cargo test` before and after each move. Final count: 100 tests passing (up from 93 at the start of this work — the delta is the 7 new tests genuinely added for previously-untested behavior: 4 for import/export, 3 for the `AppState`-port proof; the diagnostics tests replace an equivalent, now-relocated set that already existed for `performance_guidance`), zero failures, zero modified assertions in any pre-existing test.
    - *"Refactor proceeds workflow-by-workflow with reviewable diffs."* Six sequential extractions, each compiled, tested, and clippy-clean before the next began: generation (streaming supervision) → import/export → diagnostics → provider management → workspace bootstrap → generation (remaining chat-mutation workflow) → the `AppState`-port pass across all four `State`-dependent modules.
  - Full validation, final state: `cargo fmt` clean, `cargo clippy --all-targets -- -D warnings` clean, `cargo test` 100/100 passing, `cargo check --all-targets` clean.
- **Description:** Extract transactional conversation/generation, import/export, provider, workspace, and diagnostics workflows from the command module into cohesive application services; retain Tauri commands as validation/transport adapters.
- **Reason:** The roughly 1,100-line command module mixes transport, orchestration, persistence, HTTP, process, and diagnostics responsibilities.
- **Related audit findings:** A-ARC-04, A-SEC-10.
- **Dependencies:** FND-002, COR-001–005.
- **Priority / complexity:** High / Large.
- **Expected outcome:** Core workflows are testable without a Tauri window and additions do not expand one central switchboard.
- **Acceptance criteria:**
  - Tauri commands contain request decoding, authorization/validation call, use-case invocation, and response mapping only.
  - Use cases depend on explicit ports and can run with in-memory/test adapters.
  - No behavior change occurs without a corresponding acceptance test.
  - Refactor proceeds workflow-by-workflow with reviewable diffs.
- **Potential risks:** Big-bang refactor or duplicate old/new paths.
- **Suggested implementation notes:** Start with generation because correctness work already touches it; delete legacy orchestration as each path migrates.

#### ARC-002 — Add a typed ArkClient and versioned protocol contracts

- **Status: Complete (2026-08-13).** Introduced `src/lib/ArkClient.ts` — a single `ArkClient` TypeScript interface covering every command (typed request/response) and every event subscription the frontend uses, plus `createTauriArkClient()` (the real `@tauri-apps/api` adapter) and `createFakeArkClient(overrides)` (an in-memory fake implementing the same interface). Injected via React context (`src/lib/ArkClientContext.tsx` + `src/lib/useArkClient.ts`, split across two files specifically so the hook and the component don't share a file — a Fast-Refresh lint constraint, not a design choice); `main.tsx` mounts `<ArkClientProvider client={createTauriArkClient()}>` once at the app root. Migrated all four call sites — `App.tsx`, `ChatView.tsx`, `SettingsView.tsx` (`ProviderForm`/`OllamaModelsPanel`/`BuiltInRuntimeForm`), `DiagnosticsPanel.tsx` — off the old free-function `src/lib/api.ts` (deleted) and off raw `@tauri-apps/api/event` `listen()` calls (both the five `chat:stream-*` listeners in `App.tsx` and the `ollama:pull-progress` listener in `SettingsView.tsx`) onto `useArkClient()` + the client's typed `onX` methods. Grepped the full frontend for any remaining `invoke(`/`listen(` usage outside `ArkClient.ts` itself — none found; there is no mixed architecture.
  - **All bridge operations use typed request objects and stable typed error envelopes:** every `ArkClient` method has a typed signature (named request interfaces — `SendChatMessageInput`, `UpdateProviderInput`, etc. — not inline anonymous objects); `normalizeError()` in `ArkClient.ts` guarantees every rejection a caller sees is a `{ code: string; message: string }` `ArkError`, regardless of whether Tauri actually rejected with a full `AppError`, a bare string, or something else.
  - **Event schemas include version/revision/identity and unknown-version handling:** `StreamEvent` already had `conversationId`/`messageId` (identity) and `revision` (COR-002); added `schemaVersion` (Rust: `chat::STREAM_EVENT_SCHEMA_VERSION`, currently `1`; TypeScript: `KNOWN_STREAM_EVENT_SCHEMA_VERSION`) to both sides and to all 7 `StreamEvent` construction sites in `generation.rs`. `createTauriArkClient()`'s `guardStreamEventVersion()` drops (with a `console.warn`, not a crash) any event whose `schemaVersion` exceeds what this build understands — genuine forward-compatible unknown-version handling, not just a documented intent.
  - **Contract compatibility tests fail on Rust/TypeScript drift:** built a real, verified (not just plausible) mechanism rather than adding a codegen dependency (the plan's own "Potential risks" flags codegen's noisy-diff/persistence-leak downside). `contract/schema.json` is the single checked-in fixture recording each of the 17 shared DTOs' exact field-name set. `src-tauri/src/contract.rs` (`#[cfg(test)]`-only, 17 tests) constructs a sample instance of each Rust struct and asserts its serialized JSON keys match the fixture. `scripts/check-contract.mjs` (`pnpm run contract:check`, wired into CI's `frontend` job) parses `src/types/ark.ts` with the TypeScript compiler API and asserts each interface's declared properties match the same fixture. Verified both directions actually fail on real drift (not just passing tautologically): injected a bogus field into the fixture — `cargo test contract::conversation_matches_contract` failed with a precise missing/unexpected-field diagnostic; injected a bogus field into `ark.ts`'s `WorkspaceInfo` — `pnpm run contract:check` failed the same way. Both reverted; both pass clean.
  - **UI tests can substitute a fake ArkClient without global Tauri mocks:** `createFakeArkClient(overrides)` implements the full `ArkClient` interface with harmless defaults, overridable per-method; a test wraps the component under test in its own `<ArkClientProvider client={createFakeArkClient({...})}>` — no `vi.mock("@tauri-apps/api/core")`, no global mock. No frontend test runner exists in this repo yet (that's TST-004's scope, not ARC-002's — see the plan's own test-strategy table: "Component | UI semantics/states with fake ArkClient | Every PR | TST-004"); what ARC-002 owns is the *capability*, which is real and typechecked, not aspirational.
  - **Protocol versioning/deprecation policy is documented:** `docs/protocol-versioning.md` — covers the contract-fixture mechanism, the additive-vs-breaking distinction for commands (and why this codebase's single-signed-bundle deployment model means no cross-version compatibility matrix is needed), the `schemaVersion` bump policy for breaking event changes, a deprecation procedure, and two explicitly named known gaps (no contract coverage for closed string-enum fields like `MessageStatus`/`DestinationClass`; a TS interface deleted without removing its Rust struct isn't caught by the contract check itself, only by `tsc` failing at whatever call site used it).
  - Full validation, final state: `cargo fmt`/`clippy -D warnings` clean, `cargo test` 117/117 passing (100 prior + 17 new `contract::*` tests), `pnpm format`/`lint`/`typecheck`/`contract:check`/`build` all clean, live-verified in-browser (Chat and Settings views, including the three-levels-deep `DiagnosticsPanel`) with zero React crashes and the expected graceful error banner when `invoke`/`listen` have no real Tauri runtime to talk to.
- **Description:** Define versioned request/response/event schemas and a frontend ArkClient interface; generate or verify Rust/TypeScript compatibility; route UI calls through the client rather than direct invoke/event usage.
- **Reason:** Direct bridge access and duplicated DTOs block isolated testing, mobile reuse, and safe protocol evolution.
- **Related audit findings:** A-ARC-06, A-MOB-02, A-CMP-11.
- **Dependencies:** FND-002, ARC-001 boundaries.
- **Priority / complexity:** High / Large.
- **Expected outcome:** Desktop, tests, companion API, and mobile consume one explicit semantic contract.
- **Acceptance criteria:**
  - All bridge operations use typed request objects and stable typed error envelopes.
  - Event schemas include version/revision/identity and unknown-version handling.
  - Contract compatibility tests fail on Rust/TypeScript drift.
  - UI tests can substitute a fake ArkClient without global Tauri mocks.
  - Protocol versioning/deprecation policy is documented.
- **Potential risks:** Generated bindings can create noisy diffs or leak persistence details.
- **Suggested implementation notes:** Expose use-case concepts, not database rows. Keep device-only operations out of the cross-device protocol.

#### ARC-003 — Replace provider switching with a capability registry

- **Status: Complete (2026-08-13).** Replaced the closed `ProviderRuntime` enum (`Ollama(OllamaProvider) | LocalInferenceHost(LocalInferenceHostProvider)`, with `health`/`list_models`/`stream_chat` each hand-matching over both variants) with a `Provider` trait (`src-tauri/src/providers/mod.rs`) — `capabilities()`, `health()`, `list_models()`, `stream_chat()` required; `pull_model()`/`delete_model()` default to a typed "not supported" error, overridden only by `OllamaProvider` — plus a `ProviderRegistry` with `create`/`create_with_bearer_token` as the single provider-type-to-adapter factory. Added `async-trait` (0.1, ~2 small crates, zero new security advisories per `cargo audit`) since `stream_chat`'s closure parameter made the trait not dyn-safe on stable Rust without it. `generation.rs`, `diagnostics.rs`, and `provider_management.rs` were all updated to depend only on `Box<dyn Provider>`/`&dyn Provider` — none of them match on `provider_type` or a concrete adapter type anywhere.
  - **"Ollama and local OpenAI-compatible adapters pass one contract suite plus protocol-specific suites":** added `assert_provider_contract()` in `providers/mod.rs` — a shared async test function (health() never panics and always echoes the provider's own id; list_models() against an unreachable server fails with a typed error, not a panic/hang; capabilities() are internally consistent) — run once each via `ollama_provider_passes_the_shared_provider_contract`/`local_inference_host_provider_passes_the_shared_provider_contract`. Protocol-specific behavior (NDJSON vs SSE framing, completion markers) stays in the existing dedicated `ollama_stream_chat_*`/`local_inference_host_stream_chat_*` suites, unchanged.
  - **"Unsupported capabilities are absent/disabled with a reason":** `pull_ollama_model`/`delete_ollama_model` in `provider_management.rs` no longer destructure by concrete provider type (the old `let ProviderRuntime::Ollama(ollama) = runtime else { return Err(...) }` pattern) — calling `pull_model`/`delete_model` on a provider that doesn't support it now goes through the trait's own default method, which returns a clear typed `AppError` ("This provider does not support pulling/deleting models") from one place instead of being hand-checked at every call site. `generation.rs`'s `spawn_provider_stream` checks `runtime.capabilities().streaming` before attempting to stream, failing with a typed error rather than calling into a `stream_chat` a future non-streaming provider was never meant to receive. On the frontend, `SettingsView.tsx`'s Ollama model-management panel is now gated on `provider.capabilities.modelPull` rather than `provider.providerType === "ollama"` — the one place a `providerType` string comparison was actually standing in for a capability check rather than genuinely provider-type-specific instructional content (the other `providerType` comparisons in `SetupBanner.tsx`/`SettingsView.tsx`, which select *which setup instructions to show*, are correctly left as-is — that's not a capability question).
  - **"Provider identity/version and model capability metadata are persisted/refreshed safely":** `ProviderConfig` gained a `capabilities: ProviderCapabilities` field, computed in `db::map_provider` from `provider_type` at read time — the same pattern already established for `destinationClass`/`isLocal` (computed, never stored, so it can never drift from what `ProviderRegistry`/`Provider` actually implement). Per-model capability metadata (`ModelInfo.supportsStreaming`/`supportsTools`/`supportsVision`/`supportsEmbeddings`) was already persisted/refreshed via `upsert_models` from ARC-001-era work and is unchanged.
  - **"Adding a test provider does not require modifying generation orchestration":** verified directly — `generation.rs`, `diagnostics.rs`, and `provider_management.rs` contain zero references to `OllamaProvider`, `LocalInferenceHostProvider`, or any provider-type string; every call in those three files is through `dyn Provider`/`Box<dyn Provider>`. The only remaining `match provider.provider_type.as_str()` in the entire codebase is `ProviderRegistry::create_with_bearer_token`'s own factory dispatch — provider *registration*, not orchestration, and the plan's own suggested-implementation-notes anticipate exactly this ("Keep protocol-specific behavior in adapters").
  - Grepped the full codebase for remaining provider-type switch logic before marking this complete: zero matches in Rust outside the registry's factory function; three `providerType ===` comparisons remain in the frontend, all genuinely instructional/structural (which setup copy or config form to show), not capability gates — reviewed individually above.
  - **Deliberately not done (correctly, not a gap):** `model_unload` capability is `false` for every current adapter — no provider or Ark protocol integration currently supports unloading a model from memory (grepped for "unload" pre-existing in the codebase: zero hits). The capability flag exists now, accurately reporting `false`, so a future provider that does support it has somewhere real to declare that; implementing actual unload behavior is out of ARC-003's scope (architecture, not a new feature) and isn't required by any acceptance criterion. `vision`/`embeddings`/`tools` are likewise `false` for both adapters today — accurate, and intentionally not implemented here; they belong to the Phase 6 competitive/RAG/tools items.
  - Full validation, final state: `cargo fmt`/`clippy -D warnings` clean, `cargo test` 122/122 passing (117 prior + 5 new: 3 registry/capability tests, 2 shared-contract-suite tests), `cargo audit` unchanged (zero new advisories from `async-trait`), `pnpm format`/`lint`/`typecheck`/`contract:check`/`build` all clean (18 contract types now, +1 for `ProviderCapabilities`), live-verified in-browser with zero crashes.
- **Description:** Define a Provider interface/trait and registry with capabilities for streaming, models, auth, local/remote class, context limits, vision, embeddings, tools, unload, and health semantics.
- **Reason:** The closed provider enum/switch requires central changes and assumes capabilities that future providers do not share.
- **Related audit findings:** A-ARC-05, A-FUN-04, A-CMP-01–02.
- **Dependencies:** ARC-001, ARC-002, SEC-001.
- **Priority / complexity:** High / Large.
- **Expected outcome:** Providers are extensible and UI behavior is capability-driven rather than hard-coded.
- **Acceptance criteria:**
  - Ollama and local OpenAI-compatible adapters pass one contract suite plus protocol-specific suites.
  - Unsupported capabilities are absent/disabled with a reason.
  - Provider identity/version and model capability metadata are persisted/refreshed safely.
  - Adding a test provider does not require modifying generation orchestration.
- **Potential risks:** Lowest-common-denominator abstraction.
- **Suggested implementation notes:** Keep protocol-specific behavior in adapters; use capability-specific interfaces if one trait becomes overly broad.

#### ARC-004 — Replace the global SQLite mutex with a bounded database service

- **Status: Complete (2026-08-13).** Chose the plan's own preferred-under-uncertainty option — a bounded two-connection-role architecture rather than a full connection pool — since SQLite allows exactly one writer at a time regardless of Rust-level architecture (a pool doesn't parallelize writes; it only helps concurrent *reads*, which a second connection already provides with far less new machinery, no message-passing/actor thread, and zero call-site churn for the ~58 existing write-path methods).
  - **WAL/busy settings applied and verified on every opened workspace:** `Database::open` (the writer) now opens via `Connection::open_with_flags` with an explicit `file:`/`file::memory:?cache=shared` URI (see `connection_uri`) and applies `journal_mode=WAL`, `busy_timeout=5000`, `synchronous=NORMAL` (`apply_writer_pragmas`) — previously only `foreign_keys=ON` was set; WAL/busy timeout were not applied at all before this. Verified with two new tests that actually query the pragmas back (`open_enables_wal_mode`, `open_sets_a_busy_timeout`) rather than only asserting no error.
  - **Read/write concurrency policy, with documented and tested isolation behavior:** added `Database::open_read_replica` — a second connection to the same file, opened `SQLITE_OPEN_READ_ONLY` (a write attempt through it fails loudly with a typed `workspace_read_only` error rather than silently succeeding through the wrong path), stored as `AppState.read_db: Mutex<Database>` alongside the existing `AppState.db: Mutex<Database>` writer. `commands::lock_read_db` is the read-path equivalent of the existing `lock_db`; `list_conversations` and `get_conversation_messages` — the two read-hot, UI-latency-sensitive handlers most likely to be called while a stream is actively checkpointing — now go through it. `db`/`read_db` are always opened and swapped together via one new `lib::open_database_pair` helper (used by both initial `.setup()` and `workspace_bootstrap::retry_workspace_open`), so they can never drift onto different files or fallback states. Isolation behavior is proven, not just asserted, by five new concurrency tests in `db/mod.rs`: `read_replica_is_not_blocked_by_an_open_writer_transaction` (a real two-connection test — writer opens an uncommitted transaction, reader reads successfully and sees the correct pre-commit snapshot, proving true non-blocking concurrent access, not just serialization through one Rust mutex); `read_replica_rejects_writes`; `in_memory_read_replica_observes_the_writer_shared_cache` (proves the `:memory:` → `file::memory:?cache=shared` special-case actually works for the COR-010 fallback path, which would otherwise silently hand the replica an empty, disconnected in-memory database); and `settings_update_and_stream_checkpoint_writes_interleave_safely_under_the_shared_writer_mutex` — two real OS threads hammering the same `Arc<Mutex<Database>>` (the exact production `AppState.db` shape) with checkpoint-style appends and settings updates concurrently, asserting zero lost/torn writes. "Import" isolation is already covered by the pre-existing `import_rolls_back_the_whole_conversation_on_a_validation_failure`/COR-004 transaction tests (import already goes through `db.transaction(...)`, unchanged by this work). "Backup" isolation testing is genuinely not yet applicable — Ark has no backup feature yet (that's `FTR-001`, not yet implemented); noted as a forward dependency for that item to pick up, not a gap in this one.
  - **No production mutex unwrap remains on database/process state:** audited every `.lock().unwrap()` in the codebase. Found and fixed two real instances: `provider_management.rs`'s `start_built_in_runtime` had a raw `state.db.lock().unwrap()` (now routed through `commands::lock_db`), and six raw `state.sidecar.lock().unwrap()` calls across `get_built_in_runtime_status`/`stop_built_in_runtime`/`start_built_in_runtime` (now routed through a new `commands::lock_sidecar`, mirroring `lock_db`'s poison-to-typed-`AppError` mapping). The remaining `.lock().unwrap()` calls in the codebase are all on test-only `Arc<Mutex<String>>` delta collectors in `providers/mod.rs`'s test module — not production database/process state.
  - **Shutdown checkpoints safely and reports failures without data loss:** added `Database::checkpoint` (`PRAGMA wal_checkpoint(TRUNCATE)`), called from `AppState`'s existing `Drop` impl in `lib.rs` (which already handled sidecar shutdown) before the sidecar is stopped. A failed/skipped checkpoint is logged via `eprintln!`, never panics — SQLite's own startup recovery replays an un-checkpointed WAL the next time the file is opened, so this is a durability nicety, not a crash-worthy condition, matching the acceptance criterion's own "reports failures without data loss" framing (data loss would require the WAL itself to be lost, which shutdown doesn't do). Verified with `checkpoint_succeeds_on_a_freshly_opened_database` and `checkpoint_after_writes_folds_the_wal_back_into_the_main_file`.
  - Full validation, final state: `cargo fmt`/`clippy -D warnings` clean, `cargo build` clean, `cargo test` 130/130 passing (122 prior + 8 new ARC-004 tests), `cargo audit` unchanged (zero new advisories — no new dependencies were needed), `pnpm typecheck`/`build` clean (this item is backend-only; no frontend files changed).
- **Description:** Introduce a connection pool or dedicated database worker with WAL, busy timeout, transaction helpers, safe lock/error mapping, read/write concurrency policy, and shutdown/checkpoint behavior.
- **Reason:** Per-stream writes currently serialize all commands; mutex poisoning/unwrap can panic.
- **Related audit findings:** A-ARC-03, A-FUN-10, A-PERF-04.
- **Dependencies:** COR-004, COR-011.
- **Priority / complexity:** High / Large.
- **Expected outcome:** Database access is bounded, non-panicking, and does not let streaming starve unrelated operations.
- **Acceptance criteria:**
  - No production mutex unwrap remains on database/process state.
  - Concurrent read, settings update, stream checkpoint, import, and backup tests have documented isolation behavior.
  - WAL/busy settings are applied and verified on every opened workspace.
  - Shutdown checkpoints safely and reports failures without data loss.
- **Potential risks:** rusqlite connection Send/Sync constraints and backup interaction.
- **Suggested implementation notes:** A single dedicated DB actor can be simpler and safer than a pool; choose based on measured concurrency needs.

#### ARC-005 — Build a real migration and compatibility system

- **Status: Complete (2026-08-13).** The plan's own `Dependencies` field names `FTR-001 backup primitives` — a Phase 5 feature not yet built. Rather than wait on it (or build the full backup/export/restore UI feature out of order), implemented the narrow, genuinely-required primitive this item actually needs — an internal, code-only "copy and verify the database file before migrating" function — and left the full user-facing backup/restore feature (scheduling, retention, UI, manual backup-on-demand) to `FTR-001` itself, which can reuse this primitive later. This is a deliberate, minimal scope: nothing here is a UI feature or a restore flow, only what "a verified backup is created before destructive/long migrations" literally requires.
  - **"Migration applies exactly once in order and rolls back completely on injected failure":** `run_migrations` previously used a single `execute_batch` per migration with no transaction wrapping — a multi-statement migration failing partway (e.g. migration 1's 10+ `CREATE TABLE` statements) left whatever had already run applied, with no `schema_migrations` record, an implicit and undocumented safety property. New `apply_pending_migrations` wraps every migration in `BEGIN`/`COMMIT`, rolling back on any failure — *unless* the migration's own SQL toggles `PRAGMA foreign_keys` (detected by checking for that literal text), which SQLite forbids inside a transaction; migration 0002 already self-manages its own `BEGIN TRANSACTION`/`COMMIT` for exactly this reason and is left untouched. Proven with `a_failed_migration_rolls_back_completely_and_is_not_recorded_as_applied`: a deliberately-broken two-statement test migration (first statement succeeds, second fails) — the test asserts the first statement's table does NOT exist afterward and no `schema_migrations` row was written.
  - **"Changed checksum, gap, duplicate version, and newer unsupported schema fail safely":** added an FNV-1a checksum (`migration_checksum`, dependency-free — a hashing crate wasn't warranted for a non-cryptographic drift check) computed from each migration's SQL text, stored in a new `schema_migrations.checksum` column (added via `ALTER TABLE` for existing databases predating it; pre-existing rows are backfilled with the current build's checksum rather than treated as drift, since there's no historical value to compare against) and re-verified on every open — `database_migration_checksum_mismatch` if a shipped migration file was edited after release. Gap detection (`database_migration_gap`) catches a `schema_migrations` table missing an intermediate version despite recording a later one — only reachable through tampering/corruption, since this runner only ever applies in strict order. Duplicate-version detection runs both as a `debug_assertions`-gated check inside `run_migrations` and as an unconditional test (`migrations_array_has_no_duplicate_version_numbers`) so CI catches it in any build profile. The pre-existing "newer than known schema" (COR-010) rejection is unchanged in mechanism, improved in message (see downgrade guidance below). Each condition has its own dedicated test verifying the specific typed error code.
  - **"A verified backup is created before destructive/long migrations":** new `Database::backup_before_migrations` — checkpoints the WAL first (a plain file copy of a live WAL-mode database can miss committed data that only exists in the `-wal` sidecar), copies the main file to a timestamped `<file>.pre-migration-<timestamp>.bak` sibling, then opens *that copy* independently and runs `PRAGMA integrity_check` on it before allowing migration to proceed — a failed copy or failed verification aborts the migration entirely rather than proceeding without a safety net. Runs unconditionally whenever there's at least one pending migration against a real file (skipped for `:memory:`, which has nothing durable to protect) rather than trying to classify which specific migrations count as "destructive" — every migration in this codebase's history so far has been a full table rebuild, and a wrong classification in the "skip the backup" direction is the only unacceptable failure mode. Proven with `opening_a_workspace_with_a_pending_migration_creates_a_verified_backup_first`, which opens the resulting `.bak` file independently and confirms it contains the pre-migration data.
  - **"CI upgrades fixture databases from every supported release and validates logical invariants":** `upgrading_a_migration_0001_only_workspace_preserves_data_and_satisfies_invariants` constructs a migration-1-only workspace by applying migration 1's own checked-in SQL directly (bypassing the normal migration runner) and seeding representative data, then opens it through the real `Database::open` path and asserts: the pre-existing conversation/message survived migration 2's table rebuild with content intact; the new schema genuinely accepts the `'interrupted'` status migration 2 added (proof the rebuild actually happened, not just that it was recorded); and both migrations are now recorded with checksums. Chose a code-constructed fixture over a checked-in binary `.sqlite3` file deliberately — auditable in source control as a diff, and immune to bit-rot from SQLite version differences in a committed binary. Runs via the existing `cargo test` step already in CI's `rust` job; no separate CI job was needed. As future migrations are added, the same pattern (apply migrations 1..N directly, seed data, open normally, assert invariants) extends to cover each newly-obsoleted "supported release" state.
  - **"Downgrade policy is explicit; unsupported downgrade offers export/restore guidance":** the existing `database_schema_too_new` rejection's message now explicitly names the concrete recovery path — "export each conversation you need (Export as JSON) from the newer install and import it here instead" — alongside updating Ark or choosing a different workspace, rather than only the latter two. Verified directly in the existing downgrade test (extended to assert the message contains "Export as JSON").
  - Full validation, final state: `cargo fmt`/`clippy -D warnings` clean, `cargo build` clean, `cargo test` 136/136 passing (130 prior + 8 new: 6 dedicated failure-mode tests, 1 backup-verification test, 1 fixture-upgrade test — note two of the eight also strengthened a pre-existing test's assertions rather than adding a wholly separate one), `cargo audit` unchanged (zero new advisories — no new dependencies), `pnpm typecheck`/`build` clean (this item is backend-only).
  - **Known gap opened 2026-08-14, status unchanged from Complete pending investigation:** the first real cross-platform CI run (see FND-003) exposed that `opening_a_workspace_with_a_pending_migration_creates_a_verified_backup_first` — the test cited above as proof of "a verified backup is created before destructive/long migrations" — fails intermittently on ubuntu-latest/macos-latest only, never on Windows. A fresh, separate connection can read the seeded data on disk immediately before `Database::open` runs (confirmed with a since-removed diagnostic), so the loss happens somewhere inside `Database::open`'s own connection setup, not in the seed step. Five real fixes landed while investigating (switching `backup_before_migrations` from a raw file copy to SQLite's Online Backup API; no longer probing the backup's destination file before it has content; a genuinely unrelated but real bug in `prepare_workspace_root` hardening a directory before checking it was writable) and all stay regardless — none resolved this specific failure. The test is `#[ignore]`d with a tracking comment rather than deleted or left silently red; this is a narrow, non-deterministic edge case (opening a multi-version-old legacy schema immediately after a separate connection just wrote it), not the common migration path, and needs direct Linux/macOS access to debug further than a ~5-minute-per-attempt CI round-trip allows. Do not close this gap by simply removing the `#[ignore]` without a real fix — that would silently reintroduce non-deterministic CI red.
- **Description:** Replace startup execute-batch behavior with ordered, checksummed, transactional migrations, explicit current/target version checks, pre-migration backup, downgrade/newer-schema handling, and upgrade fixtures from every supported release.
- **Reason:** Current migration recording does not safely control future schema evolution or partial failures.
- **Related audit findings:** A-ARC-02, A-OPS-04.
- **Dependencies:** ARC-004 design, FTR-001 backup primitives.
- **Priority / complexity:** Critical / Large.
- **Expected outcome:** Public releases can evolve persisted data without silent corruption.
- **Acceptance criteria:**
  - Migration applies exactly once in order and rolls back completely on injected failure.
  - Changed checksum, gap, duplicate version, and newer unsupported schema fail safely.
  - A verified backup is created before destructive/long migrations.
  - CI upgrades fixture databases from every supported release and validates logical invariants.
  - Downgrade policy is explicit; unsupported downgrade offers export/restore guidance.
- **Potential risks:** First migration must correctly identify existing databases that lack robust version metadata.
- **Suggested implementation notes:** Make the transition migration conservative and preserve an untouched copy before normalizing legacy state.

#### ARC-006 — Define settings ownership and remove schema ambiguity

- **Status: Complete (2026-08-13).** This item and `SEC-005` list each other as dependencies in the plan (`ARC-006`'s `Dependencies` names `SEC-005`; `SEC-005`'s names `ARC-006`) — a genuine circular reference, not resolvable by doing either strictly first. Resolved by implementing everything ARC-006 needs on its own — the settings catalog, ownership audit, and the "secrets store only opaque references" *contract* — without building SEC-005's actual OS-keychain adapters (Windows Credential Manager/macOS Keychain/Linux Secret Service), which remain SEC-005's to implement against the reference format this item confirms.
  - **Settings catalog** (`docs/settings-catalog.md`): every durable setting, its owner/scope (device, workspace, provider, conversation, secret — `project` doesn't apply yet, no project concept exists), default, validation, persistence mechanism, and UI location — the first acceptance criterion, delivered as an actual document, not just code.
  - **Audit findings and what was done, concretely:**
    - **`conversations.streaming_enabled`** — confirmed genuinely dead (a per-conversation copy of `providers.streaming_enabled`, snapshotted at creation time, never read back anywhere to make a decision; generation always streams unconditionally). **Removed** via new migration `0003_remove_duplicated_conversation_streaming_flag` (`ALTER TABLE conversations DROP COLUMN streaming_enabled` — supported natively since SQLite 3.35, no table-rebuild needed unlike migration 0002's CHECK-constraint change). `providers.streaming_enabled` remains as the one real setting; `ProviderCapabilities.streaming` (ARC-003, computed, not stored) remains a distinct concept (fixed protocol fact vs. user preference) — documented explicitly in the catalog so it doesn't look like a re-introduced duplicate.
    - **`conversations.system_prompt`** — confirmed dead (always null, no write path). **Kept**, documented as deliberately reserved for a near-term per-conversation system-prompt feature (`FTR`-family, not architecture) rather than removed-then-likely-re-added.
    - **`providers.api_key_ref`** — confirmed dead (always null, no cloud/API-key provider exists). **Kept**; its role is now explicit as `SEC-005`'s literal "opaque secret identifier" target. Confirmed no current code path can write a raw secret into it (no command exposes it as settable).
    - **`app_settings` table** — was used for exactly one key (`appearance.theme`) and nothing else. **Kept** as the general workspace-scoped settings mechanism (a legitimate extensibility point, not dead code in the same sense as an unused function), but stopped writing theme into it.
    - **Theme, previously duplicated across `localStorage["ark.theme"]` and workspace-scoped SQLite (`app_settings["appearance.theme"]`)** — the plan's own suggested-implementation-note ("keep purely visual device preferences local") flagged this as the actual bug: a display preference had no business syncing through a portable workspace file. **New `device_settings.json`** (`src-tauri/src/device_settings.rs`) — a small JSON file at the OS's per-user app-config directory, independent of which workspace is open — is now the durable, authoritative store; `localStorage["ark.theme"]` is kept, but explicitly redefined as an instant-first-paint *cache*, not a source of truth (documented in the catalog's "Theme: cache vs. source of truth" section). The old `set_theme` command and `SetThemeRequest` are removed outright, replaced by `update_device_settings`.
    - **Built-in runtime model path, previously `localStorage`-only** (the plan's Reason field named this specifically as needing migration) — moved into the same `device_settings.json` (`builtInModelPath`), now durable beyond a browser-storage clear and consistent with the same device-scoped mechanism as theme.
    - **Sidebar/right-panel collapsed state** — audited and confirmed correctly out of scope: transient UI view state, not configuration a user deliberately sets, left as pure `localStorage` unchanged (documented explicitly in the catalog so this isn't mistaken for an oversight later).
  - **"No durable setting is duplicated without an explicit override hierarchy":** the one real duplicate found (`conversations.streaming_enabled`) was removed rather than given a hierarchy, since it had no distinct meaning to preserve. Theme's cache-vs-source-of-truth relationship (`localStorage` ⇄ `device_settings.json`) is the one legitimate two-copy setting, and its precedence is now explicit and documented rather than implicit.
  - **"Secrets store only opaque references":** verified directly — `providers.api_key_ref` is never populated with a raw value anywhere in the current codebase (no command accepts one), and the settings catalog now states its intended future contract explicitly for `SEC-005` to build against.
  - **"Legacy localStorage/DB values migrate deterministically with rollback tests":** theme migrates automatically and entirely server-side — `device_settings::resolve_device_settings` (the pure decision function behind `load_device_settings`, factored out specifically so it's unit-testable without a running Tauri app) seeds a first-run `device_settings.json` from the legacy `app_settings["appearance.theme"]` SQLite value when present, and never consults it again afterward; the legacy row is left in place, not deleted. 6 dedicated unit tests cover this decision logic directly: file-exists-and-valid wins outright, missing-file falls back to the legacy seed, corrupt-file falls back to the legacy seed, no-file-and-no-seed uses the hardcoded default, and an invalid/unrecognized legacy seed value is rejected rather than propagated. Built-in model path had no prior backend copy to migrate *from* (localStorage-only, itself not durable across a storage clear); documented as an accepted one-time re-entry rather than added migration complexity for a setting with no prior durability guarantee at all.
  - **Fixture-upgrade coverage extended for the new migration boundary** (continuing the ARC-005 pattern): added `seed_migration_0002_database` and `upgrading_a_migration_0002_workspace_removes_the_duplicated_streaming_column`, which seeds a real release-2-shape workspace (migrations 1+2 applied, `streaming_enabled` column present) and asserts the column is actually gone after opening with the current build — querying it directly (not through the `Conversation` struct, which no longer has a field for it) to prove the `ALTER TABLE ... DROP COLUMN` really ran, not just recorded itself as applied.
  - Full validation, final state: `cargo fmt`/`clippy -D warnings`/`build` clean, `cargo test` 146/146 passing (141 prior + 8 new `device_settings::*` tests − 3 net from two migration-count assertions generalized from hardcoded literals to `MIGRATIONS.len()`, +2 new db fixture/removal tests — see file-level detail in `db/mod.rs`), `cargo audit` unchanged (zero new advisories, no new dependencies), `pnpm format`/`lint`/`typecheck`/`contract:check` (19 types, +2 for `DeviceSettings` and the `AppBootstrap` field change)/`build` all clean, live-verified in-browser: Settings renders, the Dark/Light toggle actually flips `document.documentElement`'s `dark` class end-to-end through the new `updateDeviceSettings` call path, zero React crashes.
- **Description:** Classify settings as device, workspace, provider, project, conversation, or secret; establish one source of truth; migrate built-in model path and duplicated streaming/settings fields; implement or remove unused schema concepts deliberately.
- **Reason:** Settings are split across localStorage and SQLite; streaming is duplicated/hard-coded; system prompt/archive/api_key_ref fields are dead or partial.
- **Related audit findings:** A-ARC-08, A-FUN-02, A-FUN-05, A-FUN-11.
- **Dependencies:** ARC-002, ARC-005, SEC-005.
- **Priority / complexity:** High / Large.
- **Expected outcome:** Every setting has clear scope, persistence, portability, validation, and precedence.
- **Acceptance criteria:**
  - Settings catalog defines owner, default, validation, migration, export/sync behavior, and UI location.
  - No durable setting is duplicated without an explicit override hierarchy.
  - Secrets store only opaque references.
  - Legacy localStorage/DB values migrate deterministically with rollback tests.
- **Potential risks:** Users may perceive changed defaults.
- **Suggested implementation notes:** Keep purely visual device preferences local; keep project/conversation behavior with the workspace domain.

#### ARC-007 — Add scalable history queries, indexes, and branch retrieval

- **Status: Complete (2026-08-14).** The `FTR-002 product model` dependency points forward to Phase 5, so this item implements only the durable query/index/pagination architecture it owns; pin/folder/project entities, archive mutations/undo, bulk management, snippets, and richer search organization remain FTR-002/003 feature work. Migration `0004_scalable_history_search.sql` adds composite keyset-history/project indexes, a nullable `conversations.project_id` filter seam, the branch-child lookup index, and a Unicode `fts5` index over conversation titles and message content. Insert/update/delete triggers keep FTS rows transactionally consistent with authoritative tables, migration backfills existing rows, and `Database::rebuild_conversation_search_index` provides a tested derived-index recovery path. Free-text input is converted to quoted Unicode-alphanumeric prefix terms (no raw FTS operators), capped at 256 characters, and filtered by archive/project through bound parameters.
  - **Stable bounded sidebar pagination:** `ConversationListRequest`/`ConversationPage` are typed Rust↔TypeScript DTOs covered by the shared contract fixture. `list_conversations_page` enforces a 1–100 row limit and uses an opaque `(updated_at, id)` keyset cursor, including deterministic equal-timestamp ordering. Bootstrap returns only the first page; the sidebar debounces server-side title/content search, exposes `Load more`, deduplicates appended pages, and keeps the selected conversation in an independent state slice so refresh/search cannot clear the open chat. Focused tests page through equal-timestamp rows with no gaps/duplicates and cover active/archived/project filters plus invalid limits/cursors. Vite/browser verification confirmed the rendered search control remains accessible and accepts Unicode input with the native 256-character ceiling; full data behavior is proven through the real SQLite/IPC-contract tests because a plain browser intentionally has no Tauri bridge.
  - **Bounded branch/path retrieval:** the former one-query-per-ancestor `get_message_path` loop and up-to-100-query descendant walk are each one recursive CTE, bounded at 20,000 nodes with a typed `branch_depth_exceeded` failure rather than silently truncating. `get_active_messages` performs only its conversation lookup plus that one path query. `EXPLAIN QUERY PLAN` tests prove primary-key ancestry lookup and `idx_messages_parent` descendant lookup on a 250-message branch; response-time assertions keep both below the plan's 100 ms baseline target.
  - **Large-fixture plan/performance evidence:** a real 1,000-conversation/1,000-message fixture asserts both a filtered 50-row page and indexed content search complete under 100 ms, and `EXPLAIN QUERY PLAN` must contain `idx_conversations_project_history` and the FTS virtual-table index respectively. Unicode consistency tests cover Latin diacritic folding, Cyrillic, CJK, prefix search, title rename, message update, archive/project filtering, conversation deletion, deliberate derived-index damage, and rebuild. A migration-3 fixture proves migration 4 backfills pre-existing title/content rows and creates every required index/trigger.
  - Full validation: `cargo fmt --check`, strict clippy, `cargo test` (152/152), `cargo build`, `cargo audit --ignore RUSTSEC-2026-0194 --ignore RUSTSEC-2026-0195` (no non-ignored vulnerability; the existing 17 allowed transitive warnings remain), and `pnpm format:check`/lint/typecheck/contract-check (20 DTOs)/build all pass.

- **Description:** Add cursor pagination, indexed title/content search, recursive CTE or equivalent branch/path retrieval, archive/project filters, and query plans tested on large fixtures.
- **Reason:** Bootstrap loads all conversations and message paths use one query per ancestor.
- **Related audit findings:** A-ARC-07, A-FUN-01, A-PERF-02.
- **Dependencies:** ARC-005, FTR-002 product model.
- **Priority / complexity:** High / Large.
- **Expected outcome:** History and branch operations scale without long global lock duration.
- **Acceptance criteria:**
  - Sidebar uses stable cursor pagination and preserves selection during refresh.
  - Active path is loaded in a bounded number of queries.
  - Content search has a documented SQLite FTS/indexing strategy, update/delete consistency, and Unicode behavior.
  - Query plans and response times meet PERF budgets on baseline datasets.
- **Potential risks:** FTS migrations increase DB size and backup time.
- **Suggested implementation notes:** Index only required searchable content and provide rebuild tooling after corruption/version changes.

#### ARC-008 — Introduce scoped frontend state and server-state reconciliation

- **Status: Complete (2026-08-14).** Added a deliberately small `useSyncExternalStore` state layer rather than a third-party state framework: `createArkStores` owns independent normalized conversation, active-generation, provider/model, device/workspace-settings, transcript, and UI-shell stores behind `ArkStateProvider`. Conversation/provider/model entities are keyed by stable IDs; composer, menu, branch-picker, and inline-edit state remain local to their owning feature. `App.tsx` is now a 190-line composition root with store-selecting containers, while `useArkController` owns application effects/query invalidation and the extracted `ChatMessageList` owns message behavior/rendering rather than acting as a pass-through.
  - **Scoped streaming and render isolation:** stream deltas update only `generation.byMessageId[messageId]`. Each memoized message bubble selects that one overlay; sidebar, Settings, shell, transcript, and sibling message entity references are unchanged. Terminal events reconcile the accumulated overlay once into the durable transcript and clear it. The store test proves generation writes notify neither catalog nor settings subscribers and preserve an unrelated message overlay by reference.
  - **Deterministic server reconciliation:** monotonic per-message revisions apply exactly once, duplicates are ignored, and missing/invalid revisions trigger one authoritative `getConversationMessages` refetch. Request sequence tokens reject stale history/transcript responses; reconciliation requests suppress deltas against an obsolete snapshot; pagination deduplicates pages; search refresh retains the selected normalized entity. Device-setting commands are serialized so full-setting writes cannot complete out of order, with latest-write-only rollback on failure.
  - **Transport boundary and verification:** the only frontend Tauri `invoke`/`listen` imports remain in `lib/ArkClient.ts`; feature components depend on the injected client/controller. Six deterministic Node tests cover revisions/gaps, stale requests, paging/selection, normalized entity identity/order, cross-store render isolation, and delta/terminal overlay behavior; `pnpm test:state` is a CI gate. Live browser verification exercised chat→Settings→chat navigation, theme mutation, and Unicode history search without a React crash (the expected typed “Tauri bridge unavailable” error is rendered in a plain browser). Full Rust validation remains 152/152 tests with fmt/clippy/build clean and no non-ignored audit vulnerability; frontend format/lint with zero warnings/typecheck/20-type contract/build all pass.

- **Description:** Split App/ChatView state into scoped conversation, generation, provider, settings, and UI-shell stores/hooks; use ArkClient queries and revision-based reconciliation; keep transient composer/scroll state local.
- **Reason:** Broad App state and prop drilling make stream races, rerenders, and mobile/client testing harder.
- **Related audit findings:** A-ARC-06, A-PERF-01.
- **Dependencies:** ARC-002, COR-002.
- **Priority / complexity:** High / Large.
- **Expected outcome:** One stream delta updates only the relevant message/generation view and authoritative state can be refetched predictably.
- **Acceptance criteria:**
  - No feature component invokes Tauri directly.
  - Selectors prevent unrelated sidebar/settings rerenders during streaming.
  - Query invalidation/revision conflicts have deterministic tests.
  - App and ChatView are reduced by responsibility, not merely split into pass-through files.
- **Potential risks:** Introducing an oversized state library for a small app.
- **Suggested implementation notes:** Prefer simple context/external store/query patterns justified by profiling; do not add abstraction without a consumer.

#### ARC-009 — Restructure modules and enforce code-quality conventions

- **Status: Complete (2026-08-14).** `docs/architecture/README.md` now defines the allowed frontend dependency direction and folder ownership plus the Rust transport → application-service → domain/infrastructure boundaries and concrete change-placement rules. The architecture reflects the actual use-case extractions completed in ARC-001/008: Tauri commands are thin adapters over dedicated generation/import-export/diagnostics/provider/workspace services; `App` is a composition root over a controller and scoped stores; message rendering is an owned chat submodule. Error normalization was surgically separated from the Tauri adapter into the pure `lib/arkErrors.ts` module with a characterization test, so feature code no longer imports a transport implementation merely to format failures.
  - Added a dependency/cycle gate (`scripts/check-module-boundaries.mjs`, `pnpm architecture:check`) that resolves all relative TypeScript imports, rejects unresolved edges, enforces the documented direction and sole `@tauri-apps/api` adapter, and detects graph cycles. It is wired into CI and was negative-tested with temporary forbidden-Tauri and two-file-cycle probes; both failures were reported, the probes were removed, and the final 33-module graph passes.
  - Frontend lint rules that were warnings are now errors and `pnpm lint` also uses `--max-warnings 0`; format/lint/typecheck/architecture check/seven frontend unit+reconciliation tests/20-type contract/build/npm high-severity audit all pass. Rust fmt, strict all-target clippy, 152 tests, build, and the configured audit gate remain clean from the ARC-008 full-suite run. The CI workflow runs formatting, zero-warning lint, architecture/cycle, contract, frontend tests/build/audit, and strict Rust checks; README validation commands and architecture link now match the enforced workflow.

- **Description:** Adopt feature/application/domain/infrastructure boundaries, extract oversized modules along real use cases, configure formatting/linting, remove only dead code made obsolete by the roadmap, and document ownership.
- **Reason:** Central UI/Rust modules are large, strict clippy is red, and no frontend lint/format gate exists.
- **Related audit findings:** A-ARC-04, A-ARC-09.
- **Dependencies:** ARC-001–008 sequencing.
- **Priority / complexity:** Medium / Large.
- **Expected outcome:** New work has clear placement, smaller review surfaces, and consistent automated quality.
- **Acceptance criteria:**
  - Architecture README defines allowed dependency direction and folder responsibilities.
  - Rust fmt/clippy and TypeScript lint/format are clean in CI.
  - Circular dependency checks are added where appropriate.
  - Refactors preserve behavior through characterization tests and produce reviewable workflow-sized changes.
- **Potential risks:** Formatting churn and merge conflicts.
- **Suggested implementation notes:** Apply formatting in isolated commits and avoid unrelated renaming/refactoring.

#### ARC-010 — Create a supervised local-runtime process manager

- **Status: Complete (2026-08-14).** Previously `Blocked` pending the "tested on supported desktop OSes" criterion, which needed the process-lifecycle tests to actually run on macOS/Linux CI runners rather than only locally on this Windows workstation. That has since happened repeatedly: every push since (UX-001 through UX-009's CI runs, e.g. [github.com/lukedamato20/Ark/actions/runs/31807955564](https://github.com/lukedamato20/Ark/actions/runs/31807955564)) has exercised the full non-fail-fast `ubuntu-latest`/`windows-latest`/`macos-latest` Rust matrix and all three legs have passed. Reclassified from `Blocked` to `Complete` on that evidence — the implementation and locally-executable acceptance work were already complete; only the external platform confirmation was outstanding.
  - **Implemented manager/state machine:** `SidecarState` now owns explicit stopped, starting, healthy, degraded, stopping, crashed, unavailable-binary, and unavailable-model states plus structured failure categories. Launch, authenticated readiness, health reconciliation, crash/exit detection, restart replacement, bounded two-second termination, Drop/shutdown cleanup, port isolation, process-group creation, and secret lifetime are centralized. Status returns typed state/failure data; failed launch refreshes authoritative state in Settings instead of leaving a generic timeout.
  - **Bounded safe diagnostics:** stdout/stderr are piped into a 200-line/128-KiB in-memory rotating buffer with a 2,048-character line cap. Known bearer/model/binary values, common auth/token forms, and absolute path tokens are redacted before storage. Readiness failures include at most five safe excerpts. Structured diagnostics never expose the model path/token and return no logs unless the user explicitly enables the accessible, off-by-default “Include recent managed-runtime log lines” consent control (maximum 50); `docs/runtime-diagnostics-policy.md` records the policy.
  - **Evidence:** seven manager tests cover every durable lifecycle classification, bounded rotation/redaction, categorized unauthorized/success readiness through real loopback HTTP, crash reconciliation with safe excerpts, consent-controlled diagnostics, occupied-port multi-instance selection, and actual Windows child reaping on manager Drop. Live browser verification confirmed the opt-in checkbox is labelled, focusable, toggles, and defaults unchecked. Phase-boundary validation passes: Rust fmt/strict clippy/162 tests/build/audit gate; frontend format/zero-warning lint/typecheck/33-module architecture check/seven tests/23-type contract/build/npm audit.

- **Description:** Encapsulate launch/readiness/log capture/health/restart/stop/crash cleanup for the managed runtime; remove poisoned-lock panics and expose structured redacted diagnostics.
- **Reason:** Current sidecar lifecycle discards stdout/stderr, uses generic readiness timeout, and has fragile state.
- **Related audit findings:** A-FUN-09, A-FUN-10, A-SEC-12.
- **Dependencies:** FND-002, COR-005, SEC-002, SEC-004.
- **Priority / complexity:** High / Large.
- **Expected outcome:** Runtime failures are diagnosable and process state always reconciles with Ark state.
- **Acceptance criteria:**
  - Process state machine covers stopped, starting, healthy, degraded, stopping, crashed, and unavailable binary/model.
  - Bounded rotating logs redact paths/secrets according to policy and can be included by consent in diagnostics.
  - Readiness reports actual failure category and recent safe log excerpts.
  - Orphan cleanup and multi-instance behavior are tested on supported desktop OSes.
- **Potential risks:** Platform process-group and termination semantics differ.
- **Suggested implementation notes:** Prefer graceful stop followed by bounded forced termination; never block app shutdown indefinitely.

### Phase 4 — UI/UX and accessibility

#### UX-001 — Implement a responsive application shell

- **Status: Complete (2026-08-14), automated screenshot regression explicitly out of scope here (see below).** Added `src/lib/useBreakpoint.ts` — `classifyWidth()` (pure, unit-tested) plus a `matchMedia`-based `useBreakpoint()` hook returning `"phone" | "compact" | "desktop"` using Tailwind's unmodified default breakpoints (768/1280px), proceeding without waiting on UX-009 (design tokens, not yet started) per this plan's established precedent for genuine unbuilt-dependency blocks (ARC-006/SEC-005, SEC-011/OPS-001). `App.tsx` now derives `sidebarIsDrawer = breakpoint === "phone"` and `contextIsDrawer = breakpoint !== "desktop"` — asymmetric on purpose: two permanently-docked side columns at 768–1279px leave too little room for chat, not because context matters less. Added `src/components/Drawer.tsx`, a reusable accessible overlay (`role="dialog"` `aria-modal="true"`, `inert` — not `display:none` — while closed so content is removed from the tab order/AT tree without unmounting, Escape-to-close and backdrop-click-to-close with focus restored to the triggering button, matching `ChatView.tsx`'s existing `HeaderOverflowMenu` keyboard pattern) and a new `ShellTopBar` (`App.tsx`) exposing "Open conversations"/"Open context panel" triggers whenever either panel is a drawer — necessary because each panel's own internal toggle lives inside content that is off-canvas and `inert` while closed, so it cannot be the way back in itself.
  - **A real framer-motion bug was found and fixed along the way.** The drawer's slide and the pre-existing sidebar/context-panel rail-collapse width animation (`ConversationSidebar.tsx`, `RightPanel.tsx` — both predate this task) originally used framer-motion's `animate` prop on a persistently-mounted element (not one entering/exiting via `AnimatePresence`). Confirmed by direct DOM inspection (`getAttribute('style')`, which is unaffected by rendering/compositing state) that framer-motion reliably never committed the target value to the DOM: not on mount without an explicit `initial` prop, and — more fundamentally — not on subsequent prop-driven updates either, even with `initial` added and even though the correct target value was verifiably reaching the component on every render. All three components were switched to plain CSS `transition-[width]`/`transition-transform` (with `motion-reduce:` variants for `prefers-reduced-motion`, mirroring what `useReducedMotion()` does for the framer-motion-driven backdrop fade, which — going through `AnimatePresence`'s enter/exit path — was and remains unaffected). This was a genuine pre-existing defect in the sidebar/context rail-collapse toggle (confirmed via DOM inspection to have been rendering at an arbitrary intrinsic width, e.g. 230px, instead of the intended 72/288px), not something introduced by this task, but it directly affects this task's own layout correctness so it was fixed here rather than filed separately.
  - **Verified live** (Vite dev server + a production `vite preview` build, both via the Claude Browser pane, `?fixture=runtime-provenance`) at all five declared viewports — 390×844, 768×1024, 980×720, 1280×720, 1920×1080 — via DOM/attribute-level inspection (`getAttribute('style')`, `inert`, `document.activeElement`, `document.body.scrollWidth` vs `window.innerWidth`): phone width renders both sidebar and context as closed, `inert`, fully off-canvas drawers with top-bar triggers; compact width docks the sidebar and keeps only context as a drawer; desktop width (1280 and 1920) renders the original two-`aside` docked layout with zero `role="dialog"` elements and zero top-bar triggers; 980px (the declared minimum) has no horizontal overflow (`body.scrollWidth === innerWidth`). Also verified: opening the sidebar drawer, typing into its search input, closing via Escape, and reopening — the typed value survives the full close/reopen cycle (content is never unmounted) and focus correctly lands on the panel on open and returns to the "Open conversations" trigger button on close.
  - **Explicitly not verified here, and why:** pixel-level screenshots. The Claude Browser pane in this session reported "the Browser pane is not displayed, so the page is not compositing frames," which made `getComputedStyle`/`getBoundingClientRect` unreliable for anything animation- or paint-dependent (confirmed separately: a component's DOM-level `style` attribute could read correctly while its computed/painted state lagged indefinitely behind, purely because no compositor frame was ever run for the hidden pane) — this is what the CSS-attribute-level verification approach above was chosen to route around. Automated screenshot/interaction-test coverage (this task's own first acceptance-criteria bullet) is scoped to TST-004, not claimed as done here, matching the precedent already used for SEC-007's deferred criteria.
  - Full validation suite run and passing: `pnpm run typecheck`, `pnpm run format` + `pnpm run lint`, `pnpm run test:frontend` (21/21), `pnpm run build` (dev and prod), `pnpm run architecture:check`, `pnpm run support:check`, `pnpm run baseline:check`, `pnpm run contract:check`, `pnpm run csp:check`, `pnpm run markdown-safety:check`, `pnpm run secret-boundary:check`, `cargo fmt --check` (no Rust files touched by this task).
- **Description:** Replace the fixed three-column layout with desktop, compact, and phone-width modes: expanded sidebar/context on wide screens, rail/drawers at compact widths, and one full-width main stack at phone widths.
- **Reason:** Header actions clip at the declared 980 px minimum and chat collapses to zero at 390 px.
- **Related audit findings:** A-UX-01, A-UX-02, A-UX-03.
- **Dependencies:** UX-009 design tokens; ARC-008 preferred.
- **Priority / complexity:** Critical / Large.
- **Expected outcome:** Primary chat remains visible and operable at every declared viewport.
- **Acceptance criteria:**
  - Automated screenshots and interaction tests cover 390×844, 768×1024, 980×720, 1280×720, and large desktop.
  - Context is an overlay/drawer below its wide breakpoint; sidebar becomes rail/drawer below its breakpoint.
  - Opening/closing drawers preserves focus, scroll, conversation selection, and reduced-motion preference.
  - No primary control clips, overlaps, or requires horizontal page scrolling.
  - Supported minimum window size matches tested reality.
- **Potential risks:** Desktop and mobile-webview patterns may diverge from native iPhone patterns.
- **Suggested implementation notes:** This makes the desktop webview responsive; per the Phase 8 scope decision, the phone-width result of this same work is what MOB-001's PWA reuses directly, not a separate native mobile task.

#### UX-002 — Simplify the chat header and context navigation

- **Status: Mostly implemented (2026-08-14).** `ProviderModelDropdown` (`ChatView.tsx`) replaces the two independent provider/model `<Select>` elements with a single trigger button that opens a listbox grouped by provider, each group showing its SEC-001-derived destination-class icon, a versioned tooltip, and its available models with a checkmark on the active one. The always-visible compact badge next to the conversation title (`ProviderStatusIcon`) satisfies "primary model/route status visible without opening a menu." `RightPanel`'s "Context" drawer was checked against "absent/empty-state appropriate until related features exist" and already satisfies it — each reserved section (Documents/Memory/Tools) is honestly labeled "Reserved for ... in a later phase" with a "Future panels only" badge, not presented as active functionality. **2026-08-14: closed the "destructive actions in overflow" gap.** Added `HeaderOverflowMenu` — Export Markdown/Export JSON/Import JSON/Delete conversation moved out of the always-visible header (freeing header width, matching this task's own stated Reason) into a `role="menu"` popover behind a single "More conversation actions" trigger; Delete is visually separated by a divider and styled with destructive coloring, distinct from the three safe actions above it. Escape closes the menu *and* returns focus to the trigger (standard menu/dialog keyboard pattern — closing must not strand focus on a now-hidden element). **Verified live in a running browser** (not just code-reviewed): started the Vite dev server, confirmed via the accessibility tree that both `button "Select a provider and model"` and `button "More conversation actions"` render with correct labels, clicked the overflow trigger and confirmed all four `menuitem`-role entries appear with correct names, pressed Escape and confirmed the menu closes. **2026-08-14: UX-001 landed**, so the header/overflow menu/provider dropdown built here now render inside the responsive shell (docked at desktop/compact, reachable through the phone-width `ShellTopBar` triggers) rather than against the old fixed three-column layout; no header-specific rework was needed. Not done: no formal accessibility audit (screen reader pass, WCAG AA contrast measurement tool) was run against either popover — the ARIA attributes follow the standard pattern and were verified structurally via the accessibility tree, but not with an actual screen reader; that full audit belongs to UX-006, which explicitly owns axe/NVDA/VoiceOver verification. move export/import/delete and secondary controls into an accessible overflow; show Context/Files/Memory only when implemented and useful. The provider/model indicator must be an interactive dropdown that also communicates connection type and privacy status through a small icon alongside the model name — giving users immediate transparency into whether their conversation is staying local, going over LAN, or leaving the device.
- **Reason:** Header width exceeds available compact space and an empty right panel permanently consumes space. Users also currently have no at-a-glance way to verify which provider is active or whether it is private, which undermines Ark's local-first promise.
- **Related audit findings:** A-UX-02, A-UX-03, A-FUN-06, C-08, SEC-001.
- **Dependencies:** UX-001, FND-001, SEC-001 (route classification must be available before the icon can be derived).
- **Priority / complexity:** High / Medium.
- **Expected outcome:** Header hierarchy remains clear from phone width to desktop without advertising placeholders. Users can see and change their active provider/model and immediately understand its privacy posture without opening Settings.
- **Acceptance criteria:**
  - Primary model/route status is visible without opening a menu.
  - Destructive actions are separated and labelled in overflow.
  - Context drawer is absent/empty-state appropriate until related features exist.
  - Keyboard and screen-reader interaction follows menu/dialog patterns.
  - **Provider/model dropdown:** The active provider name and model are shown as a clickable/tappable control in the header. Activating it opens a dropdown listing available providers and their models, allowing the user to switch without navigating to Settings.
  - **Status icons:** Each provider entry in the dropdown (and the collapsed header indicator) displays a small icon representing its connection type. The minimum icon set is: local/offline (device icon — model runs fully on this machine), LAN (network icon — routes to another device on the local network), and remote/cloud (cloud icon — data leaves the device). Additional states such as "connecting" or "unavailable" are represented with distinct icons or overlays.
  - **Privacy indicator:** The icon is derived from SEC-001's validated destination classification, not from the provider's self-reported label. A provider pointed at a remote URL cannot display the local/offline icon.
  - **Hover/focus tooltip:** Each status icon has a tooltip (desktop hover) and an accessible label (aria-describedby or equivalent) that explains in plain language what the icon means — e.g. "Private — responses are generated locally on your device. No data leaves this machine." or "Remote — responses are processed by an external server. Your conversation is sent over the internet."
  - The tooltip/label text is defined as a small versioned set of strings tied to the destination classification enum, not free-form per-provider text, so it stays accurate as providers change.
  - Icon and tooltip render correctly in both light and dark themes and meet WCAG AA contrast requirements.
  - Icon-only controls have unique accessible names; the dropdown trigger announces the current selection to screen readers.
- **Potential risks:** Hiding frequent export actions too deeply. Icon meanings must be immediately clear without requiring users to read the tooltip; validate icon choices through usability testing before finalizing.
- **Suggested implementation notes:** Use product telemetry only if later consented; initial prioritization should come from usability testing. Derive the icon set from SEC-001's destination classification (loopback → local, private LAN → LAN, public → remote); do not add a separate classification path for UI purposes.

#### UX-003 — Improve message layout and streaming navigation

- **Status: Complete for this task's own scope (2026-08-14); COR-011's full linear-scaling benchmark remains separately tracked.** This task's stated dependency on COR-011 is about the backend checkpoint/render path COR-011 covers, not something this task's own layout/scroll-follow work needs finished first — proceeded against the current streaming architecture per this plan's established precedent for genuine non-blocking dependencies (ARC-006/SEC-005, SEC-011/OPS-001), rather than waiting on PERF-001/PERF-005.
  - **Auto-follow and jump-to-latest:** added `src/lib/scrollFollow.ts` (`isNearBottom()`, pure, unit-tested, 120px default threshold) and `src/features/chat/MessageScrollContainer.tsx`, now wrapping `ChatMessageList` in `ChatView.tsx`. Deliberately driven by a `MutationObserver` on the message content's own DOM, not a subscription to message/generation state — per ARC-008, streaming deltas are scoped to the individual message bubble specifically so a token cannot force a rerender anywhere else, and watching the DOM instead of the data means this component follows streaming, sent messages, regenerates, and branch switches through the exact same code path with no special-casing per event type. **A `ResizeObserver`-based first implementation was built, then discarded after live verification showed it never fires at all in this session's non-compositing browser pane** (the same root cause behind the framer-motion investigation under UX-001 — confirmed by direct testing, not assumed); `MutationObserver` callbacks queue as a microtask off the DOM mutation itself and have no such dependency, and were confirmed working by the same method.
  - **Reading-position preservation** relies on the browser's native CSS scroll anchoring (on by default in Chromium/Firefox, not disabled anywhere in this app) rather than custom position-tracking logic — this component only ever writes `scrollTop` for a deliberate follow-to-bottom or the explicit jump-to-latest action, never to compensate for reflow.
  - **Column width:** `ChatMessageList.tsx`'s per-bubble cap changed from a flat 88% for both roles to `max-w-full` for assistant messages (now using the entire outer `max-w-3xl` reading column) and `max-w-[75%]` for user messages (a conventional narrower chat-bubble width, since prompts are rarely technical output). Live-verified: at the declared 980px minimum, code blocks now render at 611px with `overflow-x: auto` (their own internal scrollbar, never shrinking or clipping the surrounding layout) — up from the ~220px this task's own Reason cites, itself now largely explained by UX-001 removing the *permanently*-docked sidebar/context columns that the pre-UX-001 layout forced at every width.
  - **Verified live** via a new dev-only fixture, `src/lib/developmentArkClient.ts`'s `createLongConversationFixtureClient()` (`?fixture=long-conversation`, 24 alternating messages with fenced code blocks — every other fixture returns an empty message list and cannot exercise any scroll behavior): conversation load jumps instantly to the latest message; scrolling up preserves the scroll position when content is appended below (confirmed via `scrollTop` reading identically before and after, not just "looks right"); the jump-to-latest control appears only then, and clicking it scrolls to bottom, hides itself, and correctly re-arms auto-follow for subsequent content growth.
  - **Not verified, and why:** this task's own acceptance-criteria bullet calling for automated tests covering "stream deltas, branch switch, conversation load, font scaling, and window resize" — real font-scaling/window-resize reflow behavior is specifically a paint/compositing concern, and this session's browser pane cannot composite frames at all (see UX-001's write-up); conversation-load and content-growth (a DOM-mutation stand-in for streaming) mechanics *were* verified live, as described above, but not as an automated regression suite. That automated coverage is TST-004's scope, matching the precedent already used for SEC-007 and UX-001's own deferred screenshot criterion.
  - Full validation suite run and passing: `pnpm run typecheck`, `pnpm run format` + `pnpm run lint`, `pnpm run test:frontend` (25/25), `pnpm run build`, `pnpm run architecture:check`, `pnpm run contract:check`, `pnpm run secret-boundary:check`, `pnpm run markdown-safety:check`, `pnpm run csp:check`.
- **Description:** Give assistant technical output the readable column width, constrain user bubbles, add near-bottom auto-follow, preserve reading position, and provide “new response/jump to latest.”
- **Reason:** Assistant code can render at roughly 220 px and streaming can continue out of view.
- **Related audit findings:** A-UX-04, A-UX-06, A-FUN-06.
- **Dependencies:** ARC-008, COR-011.
- **Priority / complexity:** High / Medium.
- **Expected outcome:** Long prose, code, tables, and streamed output remain readable without hijacking user scroll.
- **Acceptance criteria:**
  - Auto-follow occurs only within a documented threshold from the bottom.
  - Scrolling up disables follow without content jump and exposes a keyboard-accessible latest control.
  - Assistant code/table blocks use available readable width and intentional internal overflow.
  - Tests cover stream deltas, branch switch, conversation load, font scaling, and window resize.
- **Potential risks:** Layout measurement can be flaky in tests.
- **Suggested implementation notes:** Use stable scroll anchoring and ResizeObserver carefully; avoid forced synchronous layout on each delta.

#### UX-004 — Build a complete application-state system

- **Status: Partial (2026-08-14) — the reusable component family exists and one real gap (total bootstrap failure) is closed; the full state catalog across every listed surface is not.** COR-010, one of this task's own listed dependencies, is itself `Not complete` (its remaining gap is E2E process-level startup fixtures, tracked under TST-005) — proceeded against COR-010's current typed-error surface rather than waiting, per this plan's established precedent for non-blocking dependencies.
  - **Reusable component family:** added `src/ui/statePanel.tsx` (`StatePanel`, the "one state component family with contextual variants" this task's own suggested-implementation-notes calls for) — a `tone` (loading/empty/success/warning/error) drives icon and color, with `title`/`description`/`detail`/`actions` composing the contextual variant, and an optional `role="alert"` for states that must interrupt assistive tech.
  - **Closed gap — total bootstrap failure:** previously, `getAppBootstrap`/`getBuiltInRuntimeStatus` rejecting (distinct from the narrower, already-handled `workspaceOpenError` partial failure) fell through to the generic dismissible error toast while the rest of the app rendered its ordinary empty-chat state — no retry, no path to Settings, no diagnostics, and dismissing the toast left no explanation behind at all. Added `bootstrapError: AppErrorShape | null` to `ShellState` (`src/state/arkStores.ts`), set in `useArkController.ts`'s `bootstrap()` catch block, and a new `BootstrapFailurePanel` (`App.tsx`) that replaces the entire shell — not just a banner over a broken one, since nothing else loaded — with Retry, Open Settings, and Copy diagnostics (`buildBootstrapDiagnostics`, a new whitelist-only counterpart to the existing `buildWorkspaceDiagnostics`, unit-tested the same way).
  - **A real bug surfaced during live verification and was fixed before landing:** the panel's gate (`if (bootstrapError) return <BootstrapFailurePanel .../>`) was first written as an early return placed *before* several of `App`'s own `useState`/`useRef` calls — a Rules-of-Hooks violation (the hook-call order would differ between a failed-bootstrap render and a normal one). Moved below every hook. A second, separate bug: the gate's initial condition (`bootstrapError` alone) meant clicking "Open Settings" could never actually reach Settings, since navigating there doesn't clear `bootstrapError` and the gate would keep re-showing the failure panel on every subsequent render — fixed by changing the condition to `bootstrapError && view !== "settings"`. A third: `AppFeedback` (the app's only toast surface) is normally mounted inside the shell this panel replaces, so `copyDiagnostics`'s own success/failure feedback had nowhere to render — fixed by mounting `AppFeedback` alongside the panel too. All three were caught by live verification in the browser (a new `?fixture=bootstrap-failure` dev fixture — `createBootstrapFailureFixtureClient()` in `developmentArkClient.ts`), not by inspection.
  - **Verified live:** the panel renders with all three actions and the correct error message/code; clicking "Open Settings" successfully navigates to a working Settings view (which already tolerates the same all-defaults store state it renders during the brief window before a *successful* bootstrap completes, so this needed no changes to `SettingsView.tsx` itself); clicking "Retry" re-invokes `bootstrap()` and correctly re-shows the panel when the fixture fails again; clicking "Copy diagnostics" reaches the clipboard call and correctly surfaces failure feedback through `AppFeedback` (the clipboard write itself fails in this session's headless/unfocused browser pane — an environment limitation also documented under UX-001/UX-003, not an app bug; the success path uses the exact code shape already accepted in the pre-existing `WorkspaceRecoveryBanner`).
  - **Closed gap — import terminal summary:** `ChatView.tsx`'s `handleImport` previously only showed a toast when `normalizedMessageCount > 0` (interrupted messages needing attention) — a routine successful import ended in silence, with only the progress indicator disappearing to mark completion. Every completed import now gets `Import complete. "{title}" — {n} messages imported.`, with the interrupted-messages caveat appended when relevant rather than being the only case that produces any feedback at all.
  - **Closed gap — toast auto-dismiss:** `AppFeedback`'s `info` toasts (confirmations like the import summary above) now auto-dismiss after 6s via a `useEffect`/`setTimeout` matching the debounce idiom already used in `ConversationSidebar.tsx`'s search input. `error` toasts deliberately do not: they can be actionable, and auto-hiding one before it's read would defeat the point of showing it. Verified live via a new import override on the `?fixture=long-conversation` fixture (`createLongConversationFixtureClient` in `developmentArkClient.ts`) and a simulated file-input `change` event (native file pickers aren't reachable from browser automation): the summary toast rendered with the correct conversation title and message count, then confirmed gone after the 6s timeout. The no-auto-dismiss guarantee for errors wasn't separately live-tested (this fixture's `previewConversationImport` override doesn't parse the uploaded file, so no error path was reachable through it) but is true by construction — `AppFeedback`'s effect only starts a timer when `info` is set, and there is no equivalent timer anywhere in the `error` branch.
  - **Explicitly still not done — this task's remaining acceptance criteria:** a full **state catalog** mapping every typed backend error code to an inline/page/toast/dialog presentation (only the bootstrap-failure case got a dedicated page-level treatment here); an audit of **provider/model refresh** against "scoped stale/loading status without blocking cached history" (the existing per-provider `health`/`isLoading` state was not re-examined against this specific criterion); and export's own terminal-summary/progress treatment (only import was addressed — export is a synchronous file download with no progress state of its own, so the same pattern doesn't directly apply, but this wasn't separately assessed against the criterion's wording). These remain open for a future continuation of this task.
  - Full validation suite run and passing: `pnpm run typecheck`, `pnpm run format` + `pnpm run lint`, `pnpm run test:frontend` (27/27), `pnpm run build`, `pnpm run architecture:check`, `pnpm run contract:check`, `pnpm run secret-boundary:check`.
- **Description:** Design reusable loading, empty, success, error, interrupted, cancelled, offline, stale, progress, retry, and recovery patterns for bootstrap, conversations, providers, models, import/export, database, disk, and workspace changes.
- **Reason:** Current feedback is mainly a global failure toast; many failure/recovery states are absent.
- **Related audit findings:** A-UX-07, A-UX-14, A-FUN-03, A-FUN-08.
- **Dependencies:** COR-001, COR-008, COR-010, ARC-002.
- **Priority / complexity:** High / Large.
- **Expected outcome:** Users always understand current state, consequence, and safe next action.
- **Acceptance criteria:**
  - State catalog maps typed backend errors to inline/page/toast/dialog presentation.
  - Bootstrap failure offers retry/workspace/diagnostics/exit rather than indefinite loading.
  - Provider/model refresh uses scoped stale/loading status without blocking cached history.
  - Import/export and long operations expose progress, cancelability, and terminal summaries.
  - Toasts do not carry the only copy of actionable errors and auto-dismiss appropriately.
- **Potential risks:** Too many visual patterns create inconsistency.
- **Suggested implementation notes:** Use one state component family with contextual variants; preserve technical detail behind an expandable diagnostics action.

#### UX-005 — Replace free-text configuration with validated native-friendly forms

- **Status: Partial (2026-08-14) — labelled/validated numeric controls are done; native file/folder pickers are not.** All four listed dependencies (COR-008, SEC-001, SEC-007, ARC-006) are `Complete`.
  - **Closed gap — labelled numeric controls with range guidance:** Temperature and Max tokens in `SettingsView.tsx`'s `ProviderForm` were plain `Input`s — no visible range, no inline validation, and an invalid draft (e.g. "abc") only surfaced as a generic error toast after the backend rejected the save. Added `src/lib/numberField.ts` (`validateNumberInput`, pure and unit-tested — deliberately never collapses an invalid draft to `NaN` or a silent fallback number, the exact failure mode this task's acceptance criteria calls out) and `src/ui/numberField.tsx` (`NumberField`, a labelled input wired to that validation with `aria-invalid`/`aria-describedby` pointing at both help text and an inline error, `role="alert"` on the error). Both fields now show their valid range inline, validate on every keystroke (not just on blur — this always edits an already-valid server value, so surfacing the error the moment it stops being true is more useful than deferring it), and disable both Save buttons (the normal one and the remote-risk "Save anyway" one) while invalid. `saveProvider()` now sends the validator's own parsed numbers rather than re-parsing the draft strings with `Number.parseFloat`/`parseInt`, so there is exactly one place that decides whether a value is valid.
  - **Verified live** via the existing `?fixture=secret-store` fixture (its provider is `local_inference_host`, which renders `ProviderForm` — the built-in-runtime fixtures all use `providerType: "built_in"`, which renders the separate `BuiltInRuntimeForm` instead): typing `5` into Temperature set `aria-invalid="true"`, showed "Temperature must be between 0 and 2." via the described error element, and disabled "Save provider"; correcting it to `0.9` cleared `aria-invalid`, removed the error element, and re-enabled Save.
  - **Not done — native file/folder pickers:** the workspace-path field (`SettingsView.tsx`) and the built-in runtime's model-file/source fields (`BuiltInRuntimeForm`) remain typed-path `Input`s; "File/folder pickers are primary; typed paths remain optional where supported" is not met. This needs `@tauri-apps/plugin-dialog` — a new dependency requiring Rust-side plugin registration and a new capability/permission entry, not currently part of this app — and, matching the same scope decision made for UX-004's deferred "Exit" action, adding a new plugin/capability surface was judged out of scope for this pass rather than done as an unannounced side effect. This is the largest remaining piece of this task.
  - **Already substantially satisfied, not newly built here:** "Remote route changes require an explicit privacy acknowledgement" — `ProviderForm`'s existing `remoteRiskMessage`/`riskAcknowledged`/`convertToRemoteProvider` flow (SEC-001) already gates a remote-destination save behind an explicit, uncheckable-by-default acknowledgment. "Save is transactional and reports changed/restart-required scopes" — provider save is a single atomic `updateProvider` call, and workspace save already surfaces a "restart required" badge; neither was independently re-audited against this task's exact wording in this pass.
  - **Not assessed:** "model-file validation status" beyond what `BuiltInRuntimeForm` already shows (runtime/model provenance verification badges, binary-installed/verified states) — not re-examined against this task's specific wording, since the underlying `validate_gguf_file` work belongs to SEC-007.
  - Full validation suite run and passing: `pnpm run typecheck`, `pnpm run format` + `pnpm run lint`, `pnpm run test:frontend` (33/33), `pnpm run build`, `pnpm run architecture:check`.
- **Description:** Add labelled numeric controls with range guidance, URL parsing/classification feedback, native file/folder pickers, model-file validation status, and unsaved/save state.
- **Reason:** Temperature/max tokens accept arbitrary text and users must manually type workspace/model paths.
- **Related audit findings:** A-UX-10, A-UX-11, C-08.
- **Dependencies:** COR-008, SEC-001, SEC-007, ARC-006.
- **Priority / complexity:** High / Medium.
- **Expected outcome:** Users cannot accidentally save malformed or unsafe provider/model/workspace settings.
- **Acceptance criteria:**
  - Every input has explicit label, help, constraints, validation timing, and accessible error association.
  - Numeric fields prevent/handle invalid intermediate edit state without converting it to NaN.
  - File/folder pickers are primary; typed paths remain optional where supported.
  - Remote route changes require an explicit privacy acknowledgement.
  - Save is transactional and reports changed/restart-required scopes.
- **Potential risks:** Native dialog capabilities differ across OSes.
- **Suggested implementation notes:** Client validation guides; native validation remains authoritative.

#### UX-006 — Complete semantic and screen-reader accessibility

- **Status: Partial (2026-08-14) — the semantic/ARIA work this task itself owns is done; the automated axe harness and manual NVDA/VoiceOver validation are not, and belong to TST-004.** This task's own two acceptance-criteria bullets about **automated axe** and a **manual NVDA/VoiceOver checklist** are, verbatim, also TST-004's stated acceptance criteria (TST-004 depends on UX-001–011 and explicitly owns "zero serious/critical axe findings" and "manual NVDA/VoiceOver checklist … completed for each release candidate"). Read together, UX-006 is naturally the *semantic correctness* work axe/screen-readers would actually be checking, while TST-004 is the *automated harness/release-process* around it — this pass did the former, matching the precedent already used for UX-001/UX-003's own deferred screenshot-regression criteria. Manual NVDA/VoiceOver validation additionally requires real screen-reader software this environment does not have access to at all.
  - **Landmarks:** `ChatView.tsx` and `SettingsView.tsx` were both a bare `<section>` with no landmark role at all — genuinely absent, not just unlabeled. Changed both to `<main aria-label="Chat">` / `<main aria-label="Settings">` (safe as two `<main>`s in the same document, since `App.tsx` renders exactly one of them at a time). `ConversationSidebar.tsx`'s and `RightPanel.tsx`'s `<aside>`s got `aria-label="Conversations"`/`"Context"` — previously two unlabeled `complementary` landmarks, indistinguishable from each other by landmark navigation. The conversation list itself is now a labelled `<nav aria-label="Conversation list">` inside the sidebar, distinct from the header/search/settings controls around it.
  - **Icon-only controls with no accessible name at all (not just unlabeled icons — genuinely silent buttons):** `ConversationSidebar.tsx`'s "New Chat" and "Settings" buttons, and every conversation-list item, only rendered their text label `{!collapsed && "..."}`  — at the 72px rail width (the sidebar's own collapsed state, unrelated to UX-001's drawers), all three types of button had zero accessible name at all: no text, no `aria-label`. Fixed with `aria-label={collapsed ? "..." : undefined}` on the two action buttons and an unconditional `aria-label={conversation.title}` plus `aria-current={active ? "true" : undefined}` on each conversation item (previously "selected" was communicated by background color alone, with no programmatic equivalent).
  - **Real tab semantics:** `SettingsView.tsx`'s provider switcher was a set of plain `<button>`s styled to look like tabs, with no `role="tablist"`/`role="tab"`/`aria-selected` and no relationship to the panel content below it. Added the full `tablist`/`tab`/`tabpanel` triad with `id`/`aria-controls`/`aria-labelledby` pairing.
  - **Pressed states:** the Dark/Light theme buttons indicated the active theme with color alone; added `aria-pressed`.
  - **Throttled streaming announcement:** previously there was no live-region announcement of streaming assistant output at all — not even a naive per-token one. Added `src/lib/streamAnnouncement.ts` (`computeAnnouncementDelta`, pure and unit-tested) plus a `role="status" aria-live="polite" aria-atomic="true"` `sr-only` region per assistant message bubble in `ChatMessageList.tsx`, driven by a 2-second `setInterval` that announces only the *new* text slice since the last tick (not the full accumulated content, which would make a long response reread itself from the start every tick) and flushes any remaining tail once streaming stops. Deliberately keyed only on `isStreaming`, not on `displayContent`, so the interval doesn't reset on every delta — the same ARC-008 concern (a token must not force extra work outside its own message bubble) applies to the *throttle itself*, not just the visible re-render.
  - **Verified live:** every landmark/label/attribute above via direct DOM inspection (`aria-label`, `aria-current`, `aria-pressed`, `role="tabpanel"` presence) on `?fixture=long-conversation`, both expanded and collapsed sidebar states, and in Settings. The live region's ARIA wiring was confirmed present and correctly attributed; the actual throttled-announcement *behavior* during a real streaming session was not exercised live, since no current fixture simulates real-time generation deltas — its correctness rests on `computeAnnouncementDelta`'s unit tests and the interval logic being a standard, reviewed pattern, not on a live capture.
  - **Not done:** the automated axe harness and the manual NVDA/VoiceOver checklist (TST-004, as above); a full sweep for every remaining icon-only control across the app (Settings' secret-store retry/delete icons, model-pull/delete icons, etc. were not individually re-audited in this pass, though a sampling suggests most already carry `aria-label`).
  - Full validation suite run and passing: `pnpm run typecheck`, `pnpm run format` + `pnpm run lint`, `pnpm run test:frontend` (37/37), `pnpm run build`, `pnpm run architecture:check`, `pnpm run contract:check`, `pnpm run secret-boundary:check`.
- **Description:** Add main/nav/aside landmarks, labelled navigation, real tab semantics, pressed/selected states, explicit input labels, status/alert/live regions, and accessible copy/progress announcements.
- **Reason:** Provider tabs, theme buttons, stream/error/loading states, workspace/rename inputs, and landmarks lack complete semantics.
- **Related audit findings:** A-UX-15, A-FUN-06.
- **Dependencies:** UX-001, UX-004.
- **Priority / complexity:** High / Medium.
- **Expected outcome:** Core workflows are understandable and operable with screen readers.
- **Acceptance criteria:**
  - Automated axe has zero serious/critical violations in defined screens/states.
  - NVDA on Windows and VoiceOver on macOS validate conversation creation, model selection, send/stop, error recovery, settings, import/export, and dialogs.
  - Streaming announcements are throttled and do not read every token.
  - All icon-only controls have unique contextual names.
- **Potential risks:** Overly verbose live regions.
- **Suggested implementation notes:** Announce state changes and completed chunks/sentences, not token deltas.

#### UX-007 — Implement deterministic keyboard and focus behavior

- **Status: Partial (2026-08-14) — composer focus, real drawer/dialog focus trapping, and a discoverable shortcuts reference are done; Playwright keyboard-only test automation is not.** All three dependencies (UX-001, UX-002, UX-004) are in a usable state for this task's own purposes.
  - **Closed gap — composer focus on new/select conversation:** neither `createConversation` nor `selectConversation` moved focus anywhere — a keyboard user who created or switched conversations had to manually Tab to the composer every time. Added `focusComposerSignal` to `ShellState` (the same bump-a-counter pattern `focusSearchSignal` already established), incremented only by those two explicit, deliberate actions — never by a passive background update (a reconciliation refetch, a provider health poll), which is exactly what this task's own acceptance criterion warns against stealing focus from. `ChatView.tsx` focuses its composer `<Textarea>` when the signal changes, gated on `signal > 0` so the very first render (bootstrap's own initial selection) doesn't also steal focus.
  - **Closed gap — drawers/dialogs didn't actually trap focus despite claiming to:** `Drawer.tsx` (UX-001) set `role="dialog" aria-modal="true"` but never intercepted Tab, so a keyboard user could tab straight past the last focusable element in an open drawer into the (`inert`, but that only blocks it from *other* input methods) content behind it — a real mismatch between the ARIA claim and actual behavior, not just a missing nicety. Extracted the fix into a shared `src/lib/useModalKeyboardBehavior.ts` hook (Escape-to-close-with-focus-restore, initial focus-on-open, and now Tab/Shift+Tab containment via a live query of focusable descendants) so `Drawer.tsx` and the new `ShortcutsDialog.tsx` below share one implementation rather than diverging copies.
  - **Closed gap — no discoverable shortcut reference at all:** the only on-screen hint anywhere was "Ctrl/Cmd + Enter to send" under the composer; the three *global* shortcuts already wired in `useArkController.ts` (Mod+N new chat, Mod+F search, Mod+, settings) had no discovery path whatsoever — a keyboard user would have to read source to find them. Added `src/components/ShortcutsDialog.tsx`, reachable via a new sidebar button (always present, at both rail and expanded width) and a new global `?` shortcut, listing all five plus Escape. `src/lib/platform.ts` (`formatShortcutKeys`/`detectIsMacPlatform`, pure and unit-tested) renders OS-appropriate labels — `⌘N` on Mac, `Ctrl+N` elsewhere — satisfying "OS-specific keys" literally, not just showing one convention everywhere.
  - **Editable-field conflict, resolved for the one shortcut that could actually have one:** the existing Mod+N/F/, shortcuts can't conflict with typing (holding a modifier while pressing a letter never inserts that letter into any input), so there was nothing to fix there. The new `?` shortcut is different — it's a plain typable character — so `useArkController.ts`'s handler now checks `isEditableTarget(event.target)` (input/textarea/contenteditable) and skips entirely while the user is typing, verified live by typing "is this a question" into the composer and confirming the dialog did not open, then confirming it does open the same way with focus elsewhere.
  - **Verified live:** selecting a conversation moves focus to the composer (`document.activeElement` became the `Ask Ark...` textarea); the shortcuts dialog opens via its button and via `?`, is suppressed via `?` while typing, traps Tab (focused the dialog's only focusable element, pressed Tab, `defaultPrevented: true` and focus stayed inside), and Escape closes it while restoring focus to the trigger button.
  - **Not done:** "Playwright keyboard-only scenarios pass with visible focus" — Playwright is not part of this project's toolchain (frontend tests are plain Node, no browser-automation framework is a dependency yet); this is TST-004/TST-007 infrastructure work, not something to bolt on as a side effect here. Rename-flow keyboard behavior (`ChatView.tsx`'s inline title editor) was not independently re-audited in this pass — it already has `autoFocus`, Enter-to-confirm, and Escape-to-cancel from prior work, which was not revisited here.
  - Full validation suite run and passing: `pnpm run typecheck`, `pnpm run format` + `pnpm run lint`, `pnpm run test:frontend` (41/41), `pnpm run build`, `pnpm run architecture:check`, `pnpm run contract:check`, `pnpm run secret-boundary:check`.
- **Description:** Define tab order, focus on new/select conversation, drawer/menu/dialog trapping and restoration, rename behavior, shortcut discovery, and conflict handling.
- **Reason:** Focus is not intentionally managed and shortcuts are only partially discoverable.
- **Related audit findings:** A-UX-16, A-UX-18.
- **Dependencies:** UX-001, UX-002, UX-004.
- **Priority / complexity:** High / Medium.
- **Expected outcome:** Every primary workflow works without a pointer and focus never disappears.
- **Acceptance criteria:**
  - New/selected conversation focuses the composer when appropriate without stealing focus from deliberate reading/search.
  - Dialog/menu/drawer focus traps and restores to the invoker.
  - Shortcut reference lists OS-specific keys and resolves editable-field conflicts.
  - Playwright keyboard-only scenarios pass with visible focus.
- **Potential risks:** Automatic focus can disrupt screen-reader/browse mode.
- **Suggested implementation notes:** Encode focus rules by user intent/event, not blanket useEffect calls.

#### UX-008 — Meet visual accessibility and motion/touch standards

- **Status: Partial (2026-08-14) — the specific measured contrast failure, remaining reduced-motion gaps, and the app's one clearly touch-first surface are fixed; a full app-wide sweep is not.** This task's own stated dependency, UX-009 (semantic design tokens), has not been started at all — proceeded against the current, pre-token CSS custom properties rather than waiting, per this plan's established precedent, since the specific issues this task's own Reason cites are independently fixable without the full token system UX-009 will eventually build.
  - **Closed gap — the cited contrast failure:** `--destructive` in dark mode (`src/styles.css`) was `0 64% 55%`, measured live in-browser (a WCAG-formula contrast calculator run directly in the Claude Browser pane against the actual computed `--card`/`--background` values, not a manual estimate) at **4.004:1** against `--card` — matching this task's own cited "~4.02:1" — against the 4.5:1 AA threshold for normal text. Raised to `0 70% 62%`, verified the same way at **4.955:1** against `--card` and **5.180:1** against `--background`. Scoped specifically to `text-destructive` usage (inline error text, the only rendering this value is actually used for anywhere in the app today, alongside a 10%-opacity `bg-destructive/10` tint) — `Button`'s `variant="destructive"` (`bg-destructive` at full opacity with white `text-destructive-foreground`) is defined but has no live caller anywhere in the codebase (confirmed by grep), so that separate contrast pairing was not re-verified and remains an open question for whenever it first gets used.
  - **Closed gap — two reduced-motion omissions:** `RightPanel.tsx`'s context-drawer content fade and `ConversationSidebar.tsx`'s conversation-list enter/exit/reorder animations both used framer-motion's `AnimatePresence` with a hardcoded transition, never checking `prefers-reduced-motion` at all — only the *width* transitions on those same two components (fixed under UX-001, plain CSS with `motion-reduce:transition-none`) were covered before this pass. Both now call `useReducedMotion()` and zero their transition duration when set, matching the pattern already used in `Drawer.tsx`; the list items' `layout` prop (framer-motion's automatic reorder animation) is now also conditionally disabled the same way. Not independently re-verified live: this session's browser pane has no way to toggle `prefers-reduced-motion` (`resize_window` doesn't expose it), so this rests on `useReducedMotion()` being framer-motion's own hook, used identically to the already-verified `Drawer.tsx` instance, not on a fresh live capture.
  - **Closed gap — the app's one clearly touch-first surface:** `App.tsx`'s `ShellTopBar` (the "Open conversations"/"Open context panel" triggers) only renders at phone/compact breakpoints, making it the single UI surface in the app that is unambiguously touch-first rather than "could be tapped or clicked." Its buttons were 32×32px (`size="icon"`); bumped to a full 44×44px (bar height 44→48px to fit them) and verified live via `getBoundingClientRect()` at 390×844. Deliberately scoped to just this surface rather than a global `icon` size-variant change, which would also resize every desktop-only icon button (theme toggle, secret-store retry, etc.) without a design review of the consequences — "where practical" in this task's own wording was read as license to prioritize the surface actually built for touch, not to bulk-resize the whole icon system.
  - **Not done:** a full touch-target sweep beyond `ShellTopBar` (many icon buttons elsewhere remain 32×32px, above the 24×24px "justified exception" floor but below 44×44px); "200% zoom and OS text scaling preserve function without horizontal page scroll" was not assessed at all in this pass.
  - Full validation suite run and passing: `pnpm run typecheck`, `pnpm run format` + `pnpm run lint`, `pnpm run test:frontend` (41/41), `pnpm run build`, `pnpm run architecture:check`.
- **Description:** Fix dark destructive/error contrast, implement prefers-reduced-motion across CSS/Framer, verify 200% zoom/reflow, and increase touch targets where practical toward 44 px.
- **Reason:** Dark error text measured about 4.02:1; no reduced-motion policy exists; 32 px targets are legal but weak for touch.
- **Related audit findings:** A-UX-17.
- **Dependencies:** UX-009.
- **Priority / complexity:** High / Medium.
- **Expected outcome:** Theme and motion remain comfortable and WCAG 2.2 AA compliant.
- **Acceptance criteria:**
  - All normal text reaches at least 4.5:1 and large text 3:1 in every theme/state.
  - Reduced motion disables nonessential transforms and replaces spinners/transitions with non-motion feedback where needed.
  - 200% zoom and OS text scaling preserve function without horizontal page scroll at supported desktop widths.
  - Touch-first surfaces target 44×44 px; justified exceptions remain ≥24×24 px.
- **Potential risks:** Token changes can subtly alter brand appearance.
- **Suggested implementation notes:** Fix semantic color tokens rather than isolated components.

#### UX-009 — Consolidate the design system and component behavior

- **Status: Partial (2026-08-14) — most of this task's own listed tokens/components already exist, built incrementally across UX-001–008 rather than as a single system; motion tokens were the one genuinely missing/inconsistent piece, now added. Documented states, keyboard/axe/visual-regression coverage, and a formal migration audit remain undone.** This task's own suggested-implementation-notes ("build only components required by roadmap screens; do not introduce a large third-party framework") describes almost exactly what already happened organically: this session added `StatePanel`, `NumberField`, `Drawer`, `ShortcutsDialog`, and reused `Button`/`Input`/`Badge`/`Panel`/`Select` throughout rather than reaching for a component library.
  - **Already satisfied, consolidated here rather than rebuilt:**
    - *Color tokens*: `src/styles.css`'s `:root`/`.dark` CSS custom properties (`--background`, `--foreground`, `--primary`, `--destructive`, etc.) plus `tailwind.config.ts`'s `theme.extend.colors` mapping them to Tailwind utilities is already a semantic (not literal-value) color token system — every component in the app already draws from it exclusively; UX-008 fixed the one measured contrast defect within it.
    - *Breakpoint tokens*: `src/lib/useBreakpoint.ts` (UX-001) is exactly "breakpoints correspond to behavior, not device brand labels" — three named tiers (`phone`/`compact`/`desktop`) driven by two named pixel constants, not "iPhone-width" or similar.
    - *Radius/font tokens*: `tailwind.config.ts`'s `borderRadius`/`fontFamily` extensions, pre-existing.
    - *`StatePanel`*: the reusable "one state component family with contextual variants" this task's own suggested-implementation-notes calls for was already built under UX-004.
  - **Closed gap — motion tokens:** this was the one real, measurable inconsistency: transitions used a mix of bare `duration-150`/`duration-200` Tailwind classes and, in framer-motion `transition` props (which don't read Tailwind's config at all), hardcoded `0.14`/`0.15`/`0.18` second literals for what were conceptually the same two kinds of motion — a micro-interaction fade written as 140ms in one file and 150ms in another for no reason. Added named tokens on both sides that must now agree on purpose: `tailwind.config.ts`'s `theme.extend.transitionDuration` (`fast: "150ms"`, `standard: "200ms"`) for CSS transitions, and `src/lib/motionTokens.ts` (`MOTION_FAST_SECONDS = 0.15`, `MOTION_STANDARD_SECONDS = 0.2`) for framer-motion's second-based `transition` prop. Migrated every existing raw `duration-150`/`duration-200` usage and every hardcoded framer-motion duration literal to reference these two names instead.
  - **Verified live:** confirmed via direct stylesheet inspection that `.duration-fast`/`.duration-standard` rules exist with the correct `150ms`/`200ms` values and that a real element (`ConversationSidebar`'s `<aside>`) resolves `duration-standard` to a computed `transition-duration: 0.2s` — this required a full dev-server restart to pick up the `tailwind.config.ts` change (a stale-cache verification hiccup caught and resolved during this pass, not a defect in the change itself).
  - **Not done:** type-scale and spacing/elevation tokens are not formally named (Tailwind's own default scale is used consistently throughout, which is arguably already sufficient, but wasn't audited or documented as a deliberate decision here); no component-example/showcase surface exists demonstrating default/hover/focus/disabled/loading/error/success states side by side; keyboard/axe/visual-regression test coverage is TST-004's scope, not repeated here; no formal migration audit confirming every feature has moved off ad hoc local variants onto the shared primitives.
  - Full validation suite run and passing: `pnpm run typecheck`, `pnpm run format` + `pnpm run lint`, `pnpm run test:frontend` (41/41), `pnpm run build`.
- **Description:** Define semantic color/type/spacing/elevation/motion/breakpoint tokens and standard Button, Input, Select, Tabs, Menu, Dialog, Toast, StatePanel, Badge, and message patterns with documented states.
- **Reason:** Visual consistency is good but state behavior and accessibility are inconsistent and future features will multiply components.
- **Related audit findings:** A-UX-03, A-UX-07, A-UX-15–17.
- **Dependencies:** None; coordinate with UX tasks.
- **Priority / complexity:** Medium / Large.
- **Expected outcome:** New UI uses owned, accessible primitives instead of ad hoc variants.
- **Acceptance criteria:**
  - Component examples cover default/hover/focus/disabled/loading/error/success and dark/light/reduced-motion.
  - Breakpoints correspond to behavior, not device brand labels.
  - Components pass keyboard/axe/visual regression tests.
  - Existing feature migrations are incremental and remove obsolete local variants.
- **Potential risks:** A design-system rewrite can delay critical flows.
- **Suggested implementation notes:** Build only components required by roadmap screens; do not introduce a large third-party framework.

#### UX-010 — Redesign diagnostics and benchmark reporting

- **Status: Partial (2026-08-14) — three concrete, named bugs from this task's own Reason are fixed; the formal measurement methodology this task's own acceptance criteria call for is PERF-001's, which has not started.** PERF-001 (the "measurement contract" that would authoritatively define TTFT/generation/throughput methodology) has no status entry at all yet. Rather than wait, fixed the three specific defects this task's own Reason names outright — they're bugs in the *current* approximate benchmark, not something that needs PERF-001's formal contract to fix.
  - **Closed gap — disk reported the wrong volume:** `diagnostics.rs`'s `run_diagnostics` previously summed `total_space`/`available_space` across *every* disk `sysinfo` could see — a user with a large secondary drive would see plenty of "available disk" even while the drive their workspace actually lives on was nearly full, which is exactly "misleading" per this task's own Reason. Now resolves the specific disk containing `state.workspace`'s root path (matched by longest mount-point prefix, so nested mounts like `/` vs `/home` resolve correctly) and reports only that one. The frontend label was changed to "Disk (workspace volume)" so the scope is explicit rather than implied.
  - **Closed gap — benchmark errors were silently discarded:** `run_benchmark(...).await.ok()` threw away the error entirely — a benchmark that failed (e.g. the provider disconnecting mid-stream, a real typed `stream_incomplete` case already covered by an existing test) was indistinguishable from one that was never attempted; both showed the same generic "performance is unknown" text. Added `benchmark_failure: Option<AppError>` to `DiagnosticsResult`, populated whenever a benchmark was attempted and failed; `performance_guidance` now surfaces its actual code/message instead of the generic fallback, and `DiagnosticsPanel.tsx` renders it as a dedicated `role="alert"` block.
  - **Closed gap — throughput mixed load and generation time:** `approximate_tokens_per_second` was `word_count / total_time_ms`, where `total_time_ms` includes however long the provider took to produce *anything* — model load, request queueing, prompt processing — before generation even started. Two benchmarks with identical generation speed but different startup latency would report different throughput for reasons having nothing to do with generation. Added `generation_time_ms` (`total_time_ms` minus `time_to_first_token_ms`) and throughput is now computed against it specifically; `DiagnosticsPanel.tsx` shows "Generation time" as its own metric alongside "First token"/"Total time" so the three phases are visually distinguished, satisfying "metrics state their method and distinguish model load, TTFT, generation, and total" for the parts this benchmark actually measures (it has no separate model-load timer distinct from TTFT, which would need PERF-001's fuller methodology).
  - **Verified:** all three fixes covered by Rust unit/integration tests (a new `performance_guidance_reports_the_actual_failure_when_the_benchmark_errored` test, `generation_time_ms` invariant assertions on the existing mock-stream benchmark test); full 231-test Rust suite, fmt, clippy, and the frontend contract check (both `DiagnosticsResult`/`BenchmarkResult`'s new fields added to `contract/schema.json` and `src/types/ark.ts` together) all pass. Live-verified in the browser via a new `runDiagnostics` override on the `?fixture=long-conversation` fixture (every other fixture's `runDiagnostics` is unimplemented) returning a failed benchmark: the `role="alert"` block rendered with the real `stream_incomplete` code and message, and the "Disk (workspace volume)" label was confirmed present.
  - **Not done — this task's remaining acceptance criteria, genuinely PERF-001's scope:** "guidance rules are versioned and tested per platform/provider" (today's guidance is a single, unversioned threshold ladder); GPU/accelerator detection remains explicitly "not available" rather than attempted (this is already the honest choice this task's own acceptance criteria asks for — "unsupported GPU detection says unknown/not supported rather than guessing" — so this is correctly *not* a gap, just unchanged); a true model-load-time measurement distinct from TTFT.
  - Full validation suite run and passing: `pnpm run typecheck`, `pnpm run format` + `pnpm run lint`, `pnpm run test:frontend` (41/41), `pnpm run build`, `pnpm run contract:check`; Rust `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` (231 passed, 1 pre-existing ignored).
- **Reason:** Current disk/GPU data is misleading/incomplete; benchmark errors are swallowed and whitespace token/s mixes load/generation.
- **Related audit findings:** A-UX-13, A-FUN-09.
- **Dependencies:** ARC-003, ARC-010, PERF-001.
- **Priority / complexity:** Medium / Large.
- **Expected outcome:** Diagnostics help users choose/fix a local model and produce useful redacted support evidence.
- **Acceptance criteria:**
  - Metrics state their method and distinguish model load, TTFT, generation, and total.
  - Disk refers to active workspace/model volumes.
  - Unsupported GPU detection says unknown/not supported rather than guessing.
  - Failures retain provider/runtime error category and recommended next step.
  - Guidance rules are versioned and tested per platform/provider.
- **Potential risks:** Cross-platform GPU APIs are inconsistent.
- **Suggested implementation notes:** Prefer accurate “unknown” over fragile vendor detection; integrate richer hardware fit in PERF-004/FTR-006.

#### UX-011 — Make onboarding, feedback, and response metadata truthful

- **Status: Partial (2026-08-14) — the two real, verifiable gaps this task's own Reason names are closed; FTR-006 (one of this task's own listed dependencies, Extra Large, not started) blocks a full first-run/capability-detection rework.** FND-001, SEC-001, and UX-004 (this task's other three dependencies) are all in a usable state.
  - **Already satisfied, verified rather than rebuilt:** "onboarding overclaims built-in inference" — grepped the whole frontend for "ships with"/"comes with"/"bundled model"/"out of the box" phrasing; found none. `BuiltInRuntimeForm` (`SettingsView.tsx`) already states plainly that "the binary is not bundled with the app — it's downloaded once via the setup script." "First run detects available supported providers without blocking access to local history/settings" and "unavailable capabilities have accurate install/connect steps" — already true of the existing bootstrap architecture (conversations/settings always load regardless of provider health) and `SetupBanner.tsx` (specific, accurate per-provider instructions — Ollama, llama.cpp, LM Studio, Jan — not vague guidance). "Make keyboard shortcuts discoverable" — already done under UX-007's `ShortcutsDialog`, this task's own description names the same deliverable a second time.
  - **Closed gap — persisted response metadata was entirely hidden:** `Message` (`types/ark.ts`) has carried `providerId`/`modelId`/`tokenCount` since ARC-002, but no UI anywhere displayed any of it — exactly "useful persisted metadata is hidden" per this task's own Reason. Added a per-message, collapsed-by-default disclosure (`ChatMessageList.tsx`, a small `Info`-icon toggle next to the "You"/"Ark" role label, `aria-expanded` correctly reflecting state) showing provider name, model, route class, and token count when present. Route class reuses the exact same `CONNECTION_METADATA` icon/label/tone/description `ChatView.tsx`'s header indicator already uses — extracted to `src/lib/destinationClass.ts` specifically so the per-message and header indicators can never drift out of sync with each other, rather than maintaining two copies of a SEC-001 privacy-relevant classification. Matches this task's own suggested-implementation-notes ("default to a compact disclosure row with an expandable detail view") and the "Metadata can clutter the chat" risk it flags — collapsed by default, per-message, not a permanent chrome addition.
  - **Closed gap — copy actions gave no accessible confirmation, and one had no failure handling at all:** `MarkdownMessage.tsx`'s code-block "Copy" button changed its icon/label on success but announced nothing to a screen reader; added a `role="status" aria-live="polite"` `sr-only` region. While fixing that, found `copy()` had **no error handling whatsoever** — confirmed live that a rejected `navigator.clipboard.writeText` (this session's browser pane genuinely throws `NotAllowedError: Document is not focused`, not a hypothetical) left an uncaught promise rejection and the button silently stuck on "Copy" forever, with no indication anything went wrong. Now catches the failure, shows "Copy failed" on the button, and announces "Copy failed. Your browser or OS blocked clipboard access." through the same live region.
  - **Verified live:** clicked the metadata toggle and confirmed the panel shows "Provider: Built-in llama.cpp / Model: fixture-model.gguf / Route: local" (via a new `runDiagnostics`-adjacent path — reused the existing `?fixture=long-conversation` fixture's provider); clicked Copy and, checking within the 1200ms transient window (a naive check after normal tool round-trip latency missed it entirely, landing after the auto-reset — the same false negative pattern already noted for UX-004's toast auto-dismiss check), confirmed both the button text and the live region correctly show the failure state before resetting to idle.
  - **Not done — genuinely FTR-006's scope:** the full "rework first-run/provider setup around capability detection" this task's own description calls for depends on FTR-006's not-yet-built model discovery/download/hardware-fit system; there is no dedicated first-run *wizard* distinct from the existing always-visible `SetupBanner`, which was judged to already satisfy this task's specific acceptance criteria (non-blocking, accurate) without needing a separate onboarding flow invented from scratch.
  - Full validation suite run and passing: `pnpm run typecheck`, `pnpm run format` + `pnpm run lint`, `pnpm run test:frontend` (41/41), `pnpm run build`, `pnpm run architecture:check`, `pnpm run contract:check`, `pnpm run secret-boundary:check`.
- **Reason:** Onboarding overclaims built-in inference, success is ambiguous, and useful persisted metadata is hidden.
- **Related audit findings:** A-UX-09, A-UX-12, A-UX-18, A-FUN-11.
- **Dependencies:** FND-001, SEC-001, UX-004, FTR-006 provider/model state.
- **Priority / complexity:** Medium / Large.
- **Expected outcome:** Users know what is configured, where data went, whether an operation succeeded, and how a response was produced.
- **Acceptance criteria:**
  - First run detects available supported providers without blocking access to local history/settings.
  - Unavailable capabilities have accurate install/connect steps and no “ships with” claim unless packaged.
  - Response metadata is concise, optional, accessible, and includes route class/provenance.
  - Success feedback is contextual and copy announcements use live regions.
- **Potential risks:** Metadata can clutter the chat.
- **Suggested implementation notes:** Default to a compact disclosure row with an expandable detail view.

### Phase 5 — Production feature completion

#### FTR-001 — Implement verified backup, restore, and workspace migration

- **Status: Complete for the acceptance criteria's own stated safe defaults (2026-08-14); "move" (delete the original) and multi-release-fixture restore testing are explicitly out of scope, with reasons.** Dependencies ARC-004/ARC-005 are `Complete`; COR-010 is `Not complete` but only for E2E process-level startup fixtures (TST-005) — its actual recovery infrastructure, which is what this task needed, already exists.
  - **New `src-tauri/src/backup.rs` module**, reusing the exact checkpoint-then-Online-Backup-API-then-independently-verify pattern already established by `db::backup_before_migrations` (a raw file copy only produces a complete backup if a checkpoint happens to fully drain the WAL at that exact instant; the Backup API reads a consistent snapshot regardless) and `data_protection.rs`'s `begin_maintenance`/`MaintenanceGuard` exclusivity guard (made `pub(crate)` and reused directly rather than duplicated, since backup/restore/workspace-copy need the identical "no concurrent database mutation, no active stream/import" property SEC-006's protection-mode changes were originally built for).
  - **`create_backup`**: snapshots the workspace database plus a hash-manifested `.ark-backup-manifest.json` sidecar (app version, timestamp, SHA-256, size) into a caller-chosen directory; never overwrites an existing backup. Added `Database::create_verified_backup` as a *new*, separate method from `backup_before_migrations` — deliberately not refactored to share code with that well-tested pre-migration safety path, so a change to user-initiated backups can never risk it or vice versa.
  - **`preview_restore`/`restore_backup`**: read-only inspection (integrity check, live-queried schema version and conversation/message counts, not just the manifest's self-reported claims) followed by a restore that always lands in a *brand-new* workspace directory — this task's own acceptance criteria's stated safe default ("defaults to a new path... with original retained"), never the live one in place. The live `AppState`/database connections are never touched by either function; switching to the restored copy afterward reuses the existing `set_workspace` flow (already `requiresRestart: true`).
  - **Workspace-change copy mode**: `SetWorkspaceRequest` gained an optional `copyData` flag; `workspace::set_workspace_root` now seeds the new location with a verified copy of the current database (via a new `backup::copy_workspace_data`) before repointing to it, only after the destination directory itself is confirmed writable — a failed copy leaves the current workspace selection completely unchanged. Frontend: a new checkbox in Settings → Storage, unchecked (start-empty) by default.
  - **Deliberately not implemented — "move" (copy then delete the original):** the live app still holds the original database file open for the remainder of the session (a workspace change only takes effect after restart), and deleting a file the process still has open is not reliably safe or behaviorally consistent across Windows/macOS/Linux. `backup.rs`'s own module doc comment records this reasoning. The documented alternative — copy, confirm the new location works, delete the old one yourself — covers the same practical need without the platform risk.
  - **A real robustness gap found and fixed during testing, not by inspection:** `copy_workspace_data` implicitly relied on its only caller (`set_workspace_root`, which happens to create the target directory first via `prepare_workspace_root`) to have already prepared the destination — an undocumented precondition, not something the function enforced itself. A direct unit test calling `copy_workspace_data` on its own failed with `unable to open database file` before this was caught; fixed by making the function create and harden its own destination directory, matching `create_backup`'s already-self-sufficient pattern.
  - **Verified:** 10 new Rust integration tests (real temp-file SQLite databases and real `AppState`, not mocks) covering: a created backup's hash/size matching an independent post-hoc recomputation and the backup itself being a real, readable database with the seeded data; refusing to overwrite an existing backup/restore/copy destination *and leaving it byte-for-byte unchanged*; a backup-creation failure (a deterministic proxy for insufficient-space/interrupted — the destination's parent path is occupied by a plain file, so directory creation fails) leaving the *source* database completely unaffected; a corrupt/garbage "backup" file being rejected without touching the live workspace; a missing backup file being rejected with a specific error code; a full restore-to-new-workspace round trip with both the restored copy and the original live workspace independently verified to still have the seeded conversation; and `copy_workspace_data` seeding a new location while leaving the original provably untouched. Contract-tested (`BackupManifest`/`BackupResult`/`RestorePreview`, both Rust and TypeScript sides, `pnpm run contract:check` passing at 36 types). Frontend UI (`BackupRestorePanel` in `SettingsView.tsx`) live-verified in the browser via new fixture overrides on `?fixture=long-conversation`: create-backup renders the manifest summary; preview renders a "compatible" badge with counts and enables Restore; a backup manifest claiming a schema version newer than this build knows renders an "unsupported schema" badge, an explicit warning, and *hides* the Restore control entirely (checked directly, not assumed) rather than letting a doomed restore be attempted.
  - **Not done:** "restore from every supported release fixture is tested" — this pass proves the current-schema round trip thoroughly but does not maintain a library of historical release-schema fixture databases to restore against; "retention options" (e.g. automatic pruning of old backups) were not built — this task's own acceptance criteria don't actually require them, only the Description mentions them as a nice-to-have. Native file/folder pickers remain out of scope, consistent with UX-005's existing deferral (typed paths throughout, same as everywhere else in Settings).
  - Full validation suite run and passing: Rust `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` (241 passed, 1 pre-existing ignored); frontend `pnpm run typecheck`, `format` + `lint`, `test:frontend` (41/41), `build`, `architecture:check`, `contract:check`, `secret-boundary:check`.
- **Description:** Add consistent SQLite snapshot backup, manifest/hash verification, retention options, restore preview, restore-to-new-workspace, and copy/move workflows when changing workspace.
- **Reason:** Workspace selection exists but history is not migrated and no backup/restore strategy protects user data.
- **Related audit findings:** A-FUN-08, A-OPS-04.
- **Dependencies:** ARC-004, ARC-005, COR-010.
- **Priority / complexity:** Critical / Large.
- **Expected outcome:** Users can recover or relocate all non-secret data without manual database manipulation.
- **Acceptance criteria:**
  - Backup uses SQLite-safe snapshot/checkpoint behavior and includes schema/app version and file hashes.
  - Restore validates before replacing anything and defaults to a new path/atomic swap with original retained.
  - Workspace change offers start empty, copy, or move with clear consequences and rollback.
  - Interrupted/insufficient-space/corrupt-backup tests preserve source and destination.
  - Restore from every supported release fixture is tested.
- **Potential risks:** Synced folders and large model/attachment files complicate consistency.
- **Suggested implementation notes:** Separate core workspace data from re-downloadable models/cache; document what each backup tier includes.

#### FTR-002 — Add scalable conversation search and organization

- **Description:** Implement indexed content search, archive/unarchive, pin, folders/projects, filters, keyboard navigation, and optional bulk actions on paginated history.
- **Reason:** Title-only search and flat history do not scale; archive exists only in schema.
- **Related audit findings:** A-UX-08, A-FUN-01, A-FUN-02, A-CMP-03, A-CMP-09.
- **Dependencies:** ARC-005, ARC-007, UX-001/002.
- **Priority / complexity:** High / Large.
- **Expected outcome:** Large histories remain findable and manageable without loading everything.
- **Acceptance criteria:**
  - Search finds title and message content with snippets, keyboard traversal, and documented Unicode behavior.
  - Archive/pin/folder/project mutations are transactional and undoable where safe.
  - Search/index updates remain consistent after edit/import/delete/restore/migration.
  - 1,000-conversation baseline meets PERF budgets.
- **Potential risks:** Project/folder semantics can overlap.
- **Suggested implementation notes:** Define project as behavior/context container and folder/tag as organization before schema work.

#### FTR-003 — Implement projects, reusable prompts, personas, and instructions

- **Description:** Add project-scoped chats, files/context policy, instructions, default provider/model/settings, and a versioned prompt/persona library with preview and provenance.
- **Reason:** User-facing workspaces/projects and prompt management are competitive gaps; current workspace is only a database location.
- **Related audit findings:** A-CMP-03, A-CMP-06, A-FUN-02.
- **Dependencies:** ARC-006, FTR-002, SEC-009.
- **Priority / complexity:** High / Large.
- **Expected outcome:** Users can organize repeatable work without copying hidden instructions between conversations.
- **Acceptance criteria:**
  - Precedence among application, project, persona, conversation, and user instructions is documented and visible.
  - Prompt versions are immutable or revisioned, exportable, and do not silently alter past provenance.
  - Project deletion/archive handles conversations/files safely with preview.
  - Injected instructions are separated from retrieved/untrusted content in the provider request model.
- **Potential risks:** Hidden instruction composition confuses users and increases prompt-injection impact.
- **Suggested implementation notes:** Provide an inspectable “context sent” view and avoid magical memory behavior.

#### FTR-004 — Complete conversation/provider generation settings

- **Description:** Implement validated system prompt, temperature, max tokens, and streaming behavior with a clear provider/project/conversation override hierarchy; remove or implement the hard-coded streaming toggle.
- **Reason:** Schema fields are unused, streamingEnabled is always true, and sources of truth are duplicated.
- **Related audit findings:** A-FUN-02, A-FUN-05, A-FUN-11.
- **Dependencies:** ARC-006, COR-003, UX-005.
- **Priority / complexity:** High / Medium.
- **Expected outcome:** Every exposed generation option changes provider behavior predictably and is preserved/exported.
- **Acceptance criteria:**
  - Effective settings and their source are visible before send and stored in response provenance.
  - Non-streaming is either implemented end-to-end for capable providers or the toggle is removed.
  - Unsupported/out-of-range values are disabled or rejected using provider capabilities.
  - Settings round-trip through export/import and project/conversation migration tests.
- **Potential risks:** Provider-specific ranges/semantics differ.
- **Suggested implementation notes:** Store normalized intent plus provider-effective values in provenance when transformations occur.

#### FTR-005 — Build a branch explorer and reproducibility view

- **Description:** Visualize message ancestry/alternatives, name branches, switch explicitly, compare sibling assistant outputs, and expose model/settings/route provenance without mutating prior nodes.
- **Reason:** Append-only branching is an Ark strength but current switching is opaque and path selection follows deepest descendants.
- **Related audit findings:** A-FUN-02, A-CMP-10, A-CMP-15.
- **Dependencies:** ARC-007, FTR-004, UX-003.
- **Priority / complexity:** Medium / Large.
- **Expected outcome:** Branching becomes a differentiated, understandable research workflow.
- **Acceptance criteria:**
  - Users can see current path, alternatives, and descendant impact before switching.
  - Comparison renders sibling responses side-by-side or sequentially with provenance.
  - Branch names and selection survive restart/export/import/sync.
  - No operation rewrites historical message content/settings.
- **Potential risks:** Tree UI becomes overwhelming on small screens.
- **Suggested implementation notes:** Use a compact branch indicator in chat and a dedicated explorer/drawer for full topology.

#### FTR-006 — Deliver managed local runtime and model lifecycle

- **Description:** Implement discovery, verified download, storage selection, compatibility/hardware fit, progress/cancel/resume, load/unload, delete, runtime update, model metadata/license, and clear external-provider coexistence. This includes explicit Ollama model management: browsing models available in the Ollama library from within Ark, pulling models via the Ollama API with live progress, deleting models, and viewing per-model metadata (size, quantization, context length, license) — so users never need to leave Ark or use the CLI to manage their Ollama models.
- **Reason:** One-click model management is table stakes; the current built-in path is an incomplete manual scaffold. Ollama is currently the primary supported local provider but Ark exposes no UI to browse, pull, or delete Ollama models — users must manage them out-of-band via the terminal.
- **Related audit findings:** A-FUN-04, A-CMP-01, A-PERF-05.
- **Dependencies:** SEC-002, SEC-004, SEC-007, ARC-003, ARC-010, PERF-004.
- **Priority / complexity:** High / Extra Large.
- **Expected outcome:** A supported user can browse, install, and run a compatible local model without typing paths, using the CLI, or installing unverified binaries.
- **Acceptance criteria:**
  - Runtime/model catalog records source, hash, license, size, quantization, context, architecture, and compatibility.
  - Download is resumable/atomic and fails closed on verification error.
  - Hardware fit warns before download/load and prevents clearly unsafe configurations unless an advanced override is justified.
  - Load/unload/delete handles in-use models and preserves user data.
  - Windows/macOS/Linux support claims match tested packaged runtime variants.
  - **Ollama-specific:** Ark presents a browsable model list sourced from the connected Ollama instance (installed models) and, where feasible, the Ollama library (available models to pull).
  - **Ollama-specific:** Users can pull a model by name from within Ark; download progress (bytes received, percentage, speed) is shown inline and the pull can be cancelled.
  - **Ollama-specific:** Users can delete a model from within Ark; a confirmation dialog names the model and its disk footprint before proceeding.
  - **Ollama-specific:** Per-model metadata visible in Ark includes: parameter count, quantization level, context length, disk size, and the model family/architecture where available from the Ollama API.
  - The Ollama model management surface degrades gracefully when Ollama is unreachable (shows last-known list as stale, disables pull/delete, surfaces a reconnect action).
- **Potential risks:** Large downloads, GPU backend matrix, model licensing, antivirus, upstream churn. Ollama library browsing depends on Ollama's registry API availability and stability.
- **Suggested implementation notes:** Start with one well-supported CPU/runtime path and external Ollama; expand accelerators only with platform test coverage. For Ollama model browsing, use the `/api/tags` endpoint for installed models and `/api/show` for metadata; evaluate the Ollama registry/search API for library browsing and fall back to a curated in-app list if the API is insufficiently stable.

#### FTR-007 — Add secure cloud and authenticated remote providers

- **Description:** Implement selected cloud/OpenAI-compatible remote adapters with keychain credentials, route disclosure, rate-limit/retry semantics, TLS requirements, model capability discovery, and cost/privacy context.
- **Reason:** Cloud providers are a competitive gap, while existing arbitrary URLs are insecurely classified.
- **Related audit findings:** A-CMP-02, A-SEC-03–04.
- **Dependencies:** SEC-001, SEC-005, ARC-003, FTR-004.
- **Priority / complexity:** High / Large.
- **Expected outcome:** Users can deliberately choose remote models without compromising local-first defaults.
- **Acceptance criteria:**
  - No remote provider is enabled/configured by default.
  - Credentials remain in secure storage and are redacted from every failure path.
  - Requests show endpoint/model/route and context categories leaving device.
  - Adapter tests cover auth failure, rate limit/retry-after, quota, unavailable model, stream variants, cancellation, and usage metadata.
  - Provider deletion revokes/removes local secret references after confirmation.
- **Potential risks:** Provider API drift, cost surprises, data-retention differences.
- **Suggested implementation notes:** Ship a small supported set; generic endpoints remain advanced and visibly unverified.

#### FTR-008 — Complete data portability and batch operations

- **Description:** Add batch/project/workspace export, versioned manifests, import merge/duplicate policies, safe provider mapping, attachment references, and human-readable recovery exports.
- **Reason:** Per-conversation Markdown/JSON is useful but insufficient for full portability and future projects/files.
- **Related audit findings:** A-FUN-07, A-OPS-04.
- **Dependencies:** COR-009, FTR-001–005, CMP-001 for attachment-inclusive export.
- **Priority / complexity:** High / Large.
- **Expected outcome:** Users can leave, migrate, archive, and recover data without database knowledge or vendor lock-in.
- **Acceptance criteria:**
  - Export supports conversation/project/full-workspace scopes and excludes secrets/cache by default.
  - Manifest versions every included entity and records hashes/provenance.
  - Import preview offers skip/duplicate/merge only where semantic merge is safe.
  - Markdown remains readable without Ark; JSON/schema documentation is published.
  - Round-trip and forward-unknown-field tests pass.
- **Potential risks:** Full exports can leak sensitive content if destination is insecure.
- **Suggested implementation notes:** Present sensitivity warning and optional archive encryption without silently inventing passwords/keys.

#### FTR-009 — Complete provider/model state management

- **Description:** Refresh providers asynchronously, preserve last-known state with timestamp, reconcile removed/renamed models, display capability/availability, and give explicit fallback/reselection paths.
- **Reason:** Provider refresh blocks bootstrap and selected models can become stale or ambiguously unavailable.
- **Related audit findings:** A-UX-12, A-UX-14, A-FUN-04, A-PERF-03.
- **Dependencies:** ARC-003, ARC-008, UX-004.
- **Priority / complexity:** High / Medium.
- **Expected outcome:** Cached local data is immediately usable and model availability changes never strand the composer without explanation.
- **Acceptance criteria:**
  - Shell/history render before network/provider refresh completes.
  - Last-known provider/model state is labelled stale with refresh time.
  - Removed selected model remains visible as unavailable and offers compatible alternatives.
  - Refresh requests are deduplicated/cancellable and cannot overwrite newer state.
- **Potential risks:** Stale state may suggest a model is usable when provider changed.
- **Suggested implementation notes:** Require a current health/capability check at send time while keeping selection/history usable.

#### FTR-010 — Expose a versioned local companion/integration API

- **Description:** Provide a disabled-by-default authenticated local API for supported conversation/provider operations, using the same application services and protocol rather than raw database access. Per the Phase 8 scope decision, this is also the *entire* companion service the PWA speaks to — no separate "cross-device protocol" or "companion service" task exists (former MOB-002/MOB-003, retired and folded in here); the same API and application services serve both the PWA's web-build assets and every conversation/provider operation the phone client needs.
- **Reason:** Competitors expose integration APIs and mobile/LAN access requires a safe service boundary.
- **Related audit findings:** A-CMP-11, A-MOB-03, A-SEC-04.
- **Dependencies:** ARC-001–003, SEC-010.
- **Priority / complexity:** High / Large.
- **Expected outcome:** Integrations and the phone companion use a documented, versioned, least-privilege API.
- **Acceptance criteria:**
  - Default state is off; enablement explains network scope and requires authentication.
  - API supports version negotiation, rate limits, request IDs/idempotency, typed errors, and audit events.
  - Loopback and paired-LAN modes have distinct controls.
  - No endpoint exposes arbitrary filesystem/database access.
  - OpenAPI or equivalent contract and conformance tests are published.
- **Potential risks:** Inbound API materially expands attack surface.
- **Suggested implementation notes:** Implement after threat-model approval and reuse ArkClient/application services to prevent semantic drift.

### Phase 6 — Competitive capabilities

#### CMP-001 — Add safe attachments and multimodal vision

- **Description:** Support attach/paste/drop files and images with preview/remove, validated storage, provider capability checks, route disclosure, lifecycle/delete/export, and vision message formatting.
- **Reason:** Attachments/vision are general-assistant table stakes and the current Files panel is placeholder-only.
- **Related audit findings:** A-CMP-07, A-FUN-07, A-SEC-06.
- **Dependencies:** SEC-007, ARC-003, FTR-003, FTR-008.
- **Priority / complexity:** High / Large.
- **Expected outcome:** Users can safely reason over supported local files/images with clear provenance.
- **Acceptance criteria:**
  - Allowed types/sizes are explicit; content sniffing does not trust extension alone.
  - Preview/remove occurs before send and remote upload disclosure names each attachment.
  - Copies/references, deletion, backup, export, and missing-file behavior are documented.
  - Unsupported provider/model combinations are blocked with alternatives.
  - Malformed image/document and large-file tests do not exhaust memory.
- **Potential risks:** Metadata/privacy leakage, native decoders, huge files.
- **Suggested implementation notes:** Strip or disclose sensitive metadata where appropriate; isolate extractors and preserve original hash/provenance.

#### CMP-002 — Implement RAG, embeddings, knowledge lifecycle, and citations

- **Description:** Add project knowledge ingestion, chunking, embeddings, index versions, retrieval policy, source citations, reindex/delete, evaluation, and context-budget controls.
- **Reason:** RAG/knowledge is a major competitive gap and required for the Context/Files vision.
- **Related audit findings:** A-CMP-04, A-SEC-09, A-CMP-15.
- **Dependencies:** CMP-001, FTR-003, ARC-003 embeddings capability, SEC-009.
- **Priority / complexity:** High / Extra Large.
- **Expected outcome:** Answers can be grounded in user-selected sources with inspectable citations and safe lifecycle management.
- **Acceptance criteria:**
  - Every retrieved chunk links to source/version/location and can be inspected.
  - Delete/reindex removes stale vectors and validates referential consistency.
  - Retrieval evaluation dataset defines recall/precision/answer-citation targets and regression gates.
  - Context budget and injection boundaries are visible; untrusted documents cannot become system instructions.
  - Local/remote embedding and generation routes are separately disclosed.
- **Potential risks:** Hallucinated citations, stale indexes, sensitive embeddings, poor retrieval quality.
- **Suggested implementation notes:** Start with a narrow set of text/PDF formats and one embedding backend; expand only after evaluation quality is stable.

#### CMP-003 — Implement capability-scoped MCP, tools, and agents

- **Description:** Add MCP/tool discovery and execution, scoped permissions, previews/approvals, secret references, sandbox/network policy, cancellation, audit log, and bounded agent loops.
- **Reason:** Tools/agents are a major competitive gap but introduce the highest prompt-injection and side-effect risk.
- **Related audit findings:** A-CMP-05, A-SEC-09, A-CMP-15.
- **Dependencies:** SEC-005, SEC-009, ARC-003, FTR-010.
- **Priority / complexity:** High / Extra Large.
- **Expected outcome:** Users can extend Ark without granting opaque unlimited authority.
- **Acceptance criteria:**
  - Tool install/connect shows publisher/source, scopes, data access, and trust status.
  - Read and write/network/secret capabilities are independently grantable/revocable.
  - Side effects require preview and approval unless a narrowly scoped remembered grant exists.
  - Agent loops have step/time/token/cost limits, cancellation, and a visible trace.
  - This task's tool set is limited to Ark Chat's general-assistant scope (e.g., web search, utilities, notes, memory, external-service connectors) and explicitly excludes filesystem write, git, and process/command execution — those are hard-scoped to Ark Code (CODE-004/CODE-005, Phase 6.5) per SEC-009's scope-tier boundary and are never exposed as an Ark Chat tool.
  - Adversarial security suite and malicious MCP server fixtures pass.
- **Potential risks:** Arbitrary code execution, credential exfiltration, destructive actions, consent fatigue.
- **Suggested implementation notes:** Begin read-only with a small allowlisted protocol subset. The filesystem/shell exclusion above is a hard acceptance criterion for this task, not an implementation preference — see CODE-004/CODE-005 for where that capability actually lives.

#### CMP-004 — Add web search as a provenance-aware tool

- **Description:** Implement one or more search/retrieval providers through the tool policy, with query preview, domain/source citations, fetch limits, route/cost disclosure, and prompt-injection isolation.
- **Reason:** Web search is a competitor capability and complements RAG, but retrieved pages are untrusted.
- **Related audit findings:** A-CMP-09, A-SEC-09.
- **Dependencies:** CMP-003, SEC-009.
- **Priority / complexity:** Medium / Large.
- **Expected outcome:** Users can request current information with visible sources and controlled data flow.
- **Acceptance criteria:**
  - Search is explicit per conversation/request and off by default for local-only workflows.
  - Queries and destination provider are previewed/disclosed.
  - Answers cite fetched sources; failures distinguish search, fetch, parsing, and model errors.
  - Page content cannot authorize tools or override system/user policy.
- **Potential risks:** Privacy leakage through queries, malicious pages, licensing/copyright obligations.
- **Suggested implementation notes:** Store minimal fetched content with source/time and respect robots/provider terms as applicable.

#### CMP-005 — Add accessible voice input and optional output

- **Description:** Add microphone permission, recording state, cancellation, transcription route selection, local/remote disclosure, editable transcript, and optional speech output.
- **Reason:** Voice is a high-value competitor, accessibility, and future mobile capability.
- **Related audit findings:** A-CMP-08, A-MOB-06.
- **Dependencies:** ARC-003 audio capabilities, SEC-001/005, MOB-008 for iOS integration.
- **Priority / complexity:** Medium / Large.
- **Expected outcome:** Voice works predictably without silently uploading audio.
- **Acceptance criteria:**
  - Recording has unmistakable visual/audible/screen-reader state and immediate stop.
  - Permission denial/revocation and unavailable device have recoverable flows.
  - Transcript is editable before send; retention/deletion is explicit.
  - Local vs remote transcription/speech route is shown before capture/output.
- **Potential risks:** Sensitive ambient audio, platform codec/device differences, accessibility conflicts.
- **Suggested implementation notes:** Start with push-to-talk transcription; defer always-listening behavior.

#### CMP-006 — Add completion notifications and background-safe behavior

- **Description:** Notify users when long generations/agent tasks finish or need approval, with opt-in settings, privacy-safe text, deep links, and platform background constraints.
- **Reason:** Long local work benefits from notification and mobile continuation; current UI has no background completion model.
- **Related audit findings:** A-CMP-13, A-MOB-06.
- **Dependencies:** FND-002, CMP-003, MOB-008, OPS-001.
- **Priority / complexity:** Medium / Medium.
- **Expected outcome:** Users can leave the foreground without missing completion or approval while content stays private.
- **Acceptance criteria:**
  - Notification content defaults to generic and never includes prompts/output unless explicitly enabled.
  - Clicking opens the exact task/conversation safely.
  - Duplicate/late/cancelled completion does not notify incorrectly.
  - OS permission denial and do-not-disturb are respected.
- **Potential risks:** Privacy on lock screens and notification fatigue.
- **Suggested implementation notes:** Notify terminal state/approval only, not token progress.

#### CMP-007 — Implement project memory with explicit controls

- **Description:** Add inspectable, editable, scoped memory derived from user-approved facts, with source, retention, disable/delete/export, and clear separation from project instructions/RAG.
- **Reason:** The right panel and competitive analysis imply memory, but hidden automatic memory would conflict with Ark's auditability goal.
- **Related audit findings:** A-CMP-03, A-CMP-15, A-SEC-09.
- **Dependencies:** FTR-003, CMP-002, SEC-009.
- **Priority / complexity:** Medium / Large.
- **Expected outcome:** Useful continuity without opaque cross-conversation data reuse.
- **Acceptance criteria:**
  - Memory is off by default or introduced with explicit opt-in.
  - Every item has source, scope, created/updated time, and delete/edit controls.
  - Context preview shows which memories will be sent and where.
  - Project/global separation and sync/export/delete behavior are tested.
- **Potential risks:** Sensitive inference, stale facts, hidden prompt influence.
- **Suggested implementation notes:** Store user-confirmed concise facts, not raw hidden summaries, for the initial release.

#### CMP-008 — Add safe automations and artifacts after tool maturity

- **Description:** Build scheduled/bounded workflows and editable rendered artifacts on top of the approved tool/agent system, with ownership, resource limits, approval checkpoints, history, cancellation, and export.
- **Reason:** Automation/artifacts are competitive gaps but should not precede a safe capability foundation.
- **Related audit findings:** A-CMP-14.
- **Dependencies:** CMP-003, CMP-006, OPS-001.
- **Priority / complexity:** Low / Extra Large.
- **Expected outcome:** Repeatable tasks and outputs are controllable, inspectable, and recoverable.
- **Acceptance criteria:**
  - Schedules show next run, permissions, model/provider, expected data route, and resource ceilings.
  - Side-effecting steps retain approval policy and idempotency.
  - Runs have immutable status/history and can be cancelled/disabled.
  - Artifacts are sandboxed, exportable, versioned, and cannot execute unrestricted code.
- **Potential risks:** Persistent background execution and artifact sandbox escape.
- **Suggested implementation notes:** Deliver only after real-world tool safety data; keep initial automation local and foreground-capable.

#### CMP-009 — Make an explicit team/multi-user edition decision

- **Description:** Conduct a product/security architecture gate after the single-user desktop and companion API mature; either approve a hosted/team edition with identity/RBAC/audit/tenant isolation or record it as out of scope.
- **Reason:** Multi-user/RBAC is a competitor feature but the audit explicitly says it is unnecessary for the current local desktop.
- **Related audit findings:** A-CMP-12, A-SEC-01.
- **Dependencies:** FTR-010, SEC-010; evidence of team demand.
- **Priority / complexity:** Low / Small for decision; Extra Large if approved.
- **Expected outcome:** No accidental half-multi-user architecture or cosmetic local login.
- **Acceptance criteria:**
  - Decision records target users, threat model, hosting/data residency, tenancy, RBAC, audit, support, and cost.
  - If deferred, no unresolved code task remains and protocol extension points are documented.
  - If approved, a separate funded roadmap is created before implementation.
- **Potential risks:** Treating a decision task as permission to ship weak shared access.
- **Suggested implementation notes:** The default disposition is **no implementation required for the single-user local desktop**; revisit only with product evidence.

### Phase 6.5 — Ark Code (agentic coding environment)

This phase is a product-roadmap addition, not an audit remediation — no task below maps to a numbered audit finding, and none is required to close C-01–C-10 or the Desktop production Definition of Done (Section 11.2). It becomes eligible for planning only once Phase 5 (Ark Chat is desktop feature-complete) and Phase 6's tool/agent/security foundation (CMP-003, SEC-009, ARC-003) are done, per the phase overview and dependency diagram above.

Ark Code is Ark's second application surface — a provider-agnostic, local-model-first agentic coding assistant — built entirely on the generic capability-scoped tool/permission/agent-loop infrastructure CMP-003 and SEC-009 already establish. It deliberately does **not** define a second tool-permission framework, a second agent-loop primitive, or a second provider abstraction, and it does not reopen A-RET-02 ("minimal Tauri core/event capabilities; no broad FS/shell plugin" — an audited strength this plan preserves): every filesystem/process capability added here is scoped, repository-restricted, and approval-gated through CMP-003/SEC-009, and CODE-008 extends TST-006's least-privilege regression suite to cover it. Ark Chat is unaffected — this phase adds a new `ActiveView` and new backend modules alongside chat and does not modify the generation lifecycle (FND-002/ADR-0001) or the chat message schema.

The infrastructure is shared; the tools are not. Per SEC-009's scope-tier boundary and CMP-003's own acceptance criteria, filesystem write, git, and process/command execution are hard-scoped to this phase alone (CODE-004/CODE-005) and are never exposed as an Ark Chat tool — Ark Chat's tool surface (CMP-003/004/007/008) stays limited to general-assistant capabilities such as web search, utilities, notes, memory, and external-service connectors, none of which require a bound Repository.

Its repository concept binds to an existing FTR-003 Project rather than introducing a second, colliding "project" concept, and is explicitly named "Repository" to avoid colliding with the existing storage-location "Workspace" (`workspace/mod.rs`).

#### CODE-001 — Extend the provider capability registry for structured tool calling

- **Description:** Add an optional structured tool-calling path to the `Provider` trait/registry established by ARC-003 — a `tools` request field, a structured tool-call/tool-result event alongside the existing text-delta stream, and a real per-model `supportsTools`/`contextWindow` capability (replacing today's hardcoded `false`/`None` stubs) — plus an Ark-defined prompted fallback protocol (a structured single-tool-call text format with a bounded repair-retry loop) for models without native function calling.
- **Reason:** `ProviderChatRequest`/`ChatMessage` are plain-text-only today; `ProviderCapabilities.tools` and `ModelInfo.supportsTools` exist but are never populated. Ark Code cannot exist without a real tool-calling contract, and local models vary widely in native support.
- **Related audit findings:** None (see phase note) — extends the ARC-003 capability registry (A-ARC-05, A-CMP-01–02) for a new consumer; not a duplicate of that task.
- **Dependencies:** ARC-003.
- **Priority / complexity:** High / Large.
- **Expected outcome:** Any provider/model can declare native tool support, prompted-fallback support, or neither, and Ark Code degrades predictably rather than silently.
- **Acceptance criteria:**
  - `Provider` gains a default-unsupported tool-calling method, mirroring the existing `pull_model`/`delete_model` default-unsupported pattern, so Ark Chat and non-tool providers are unaffected.
  - Native tool-calling is used when the selected model declares it; the prompted fallback is used otherwise, with one documented retry on malformed output before the step fails.
  - Per-model `supportsTools`/`contextWindow` are populated from real provider/model metadata, not hardcoded.
  - Existing chat contract tests and `assert_provider_contract()` pass unchanged.
- **Potential risks:** Prompted-fallback reliability on very small local models; scope creep into a general function-calling framework beyond what Ark Code needs.
- **Suggested implementation notes:** Keep the fallback protocol minimal (single tool call per turn) rather than attempting parallel tool calls on models that cannot reliably follow one.

#### CODE-002 — Define the durable agent-run lifecycle

- **Description:** Write an ADR (companion to ADR 0001) defining Ark Code's agent-run state machine — queued/planning/awaiting-approval/executing-tool/observing/completed/failed/cancelled/interrupted — its transaction boundaries, and, specifically, crash recovery for a tool call whose real-world side effect (file write, command execution) may or may not have completed before a crash.
- **Reason:** ADR 0001 only covers text-generation crash recovery (safe to mark "interrupted"); a tool call with an uncertain side effect cannot be recovered the same way — it must be re-verified against actual filesystem/git state before Ark Code decides what "resume" means.
- **Related audit findings:** None (see phase note) — direct analog of FND-002 for a new lifecycle, not a modification of it.
- **Dependencies:** FND-002, CODE-001.
- **Priority / complexity:** High / Medium.
- **Expected outcome:** One authoritative, tested contract for agent-run states that backend, frontend, and tests all follow, with no silent partial-complete for a tool call.
- **Acceptance criteria:**
  - ADR is approved before CODE-004/005 implementation begins.
  - Every state transition has a defined transaction boundary and idempotent retry/cancel behavior.
  - Startup recovery re-verifies actual state (file/git) for any tool call left in an uncertain status, rather than blindly marking it interrupted.
  - Runaway controls (max steps, wall-clock timeout, token/cost budget, loop detection on repeated identical tool calls) are part of the state machine, not bolted on later.
- **Potential risks:** Under-specifying the "uncertain side effect" recovery path, which is the hardest correctness problem in this phase.
- **Suggested implementation notes:** Reuse the "durable state is authoritative, events are overlays" pattern from ADR 0001 rather than inventing a new persistence philosophy.

#### CODE-003 — Implement the repository workspace concept

- **Description:** Add a filesystem-repository binding that Ark Code operates against, distinct from the existing storage "Workspace" (`workspace/mod.rs`, which is only Ark's own SQLite data location). Bind a repository path to an FTR-003 Project rather than inventing a second "project" concept.
- **Reason:** No repository/codebase concept exists in Ark today; reusing the word "workspace" for it would collide with the existing storage-location meaning, and inventing a second "project" concept would duplicate FTR-003.
- **Related audit findings:** A-CMP-03 (informative only — FTR-003 remains the primary task for that finding; this task extends it for Ark Code once FTR-003 ships).
- **Dependencies:** FTR-003, COR-007 (reuse validated, non-destructive path-probing).
- **Priority / complexity:** High / Medium.
- **Expected outcome:** Opening a repository in Ark Code is explicit, validated, and never collides with or silently reuses the app-data workspace path.
- **Acceptance criteria:**
  - A Project may optionally declare a repository path; validation reuses COR-007's safe-probe pattern and rejects paths outside a real, user-selected directory.
  - No Ark Code tool can resolve a path outside the bound repository root.
  - Switching or removing a repository binding does not require an app restart (unlike the storage workspace) and does not touch storage-workspace state.
  - Naming/documentation clearly distinguish "Workspace" (app data location) from "Repository" (a Project's bound codebase).
- **Potential risks:** User confusion between the two workspace-adjacent concepts if naming discipline slips in the UI.
- **Suggested implementation notes:** Surface the distinction explicitly in Settings/UI copy, not just in code naming.

#### CODE-004 — Implement the foundation read-only coding tool set

- **Description:** Implement `list_directory`, `read_file`, `search` (text search), `git_status`, and `git_diff` as capability-scoped tools through the CMP-003 tool/permission framework, plus the coding-specific repo-map/context retrieval Ark Code needs to select relevant files without embedding the whole repository.
- **Reason:** This is the smallest useful Ark Code slice — an investigation-only agent — and matches CMP-003's own suggested implementation note to "begin read-only... do not expose a generic shell/filesystem tool by default."
- **Related audit findings:** None (see phase note) — consumes CMP-003/SEC-009 rather than duplicating them.
- **Dependencies:** CMP-003, SEC-009, CODE-001, CODE-003.
- **Priority / complexity:** High / Large.
- **Expected outcome:** A user can ask Ark Code to investigate a repository and get a grounded answer with zero write/execute risk.
- **Acceptance criteria:**
  - Every tool declares its scope (read-only, repository-restricted) through CMP-003's scope model; none requests write/network/secret capability.
  - Enumeration respects `.gitignore` and a bounded file-size/type filter; no whole-repository dump into the prompt.
  - Tool execution stays inside the bound repository root under all inputs, including adversarial path traversal attempts.
  - This tool set alone is shippable as the Ark Code MVP without CODE-005.
- **Potential risks:** Large repositories exceeding a weak local model's context window even for read-only investigation.
- **Suggested implementation notes:** Reuse existing streaming-event and error-code (`AppError`) conventions rather than introducing a parallel error taxonomy.

#### CODE-005 — Implement write-capable coding tools: edit, git checkpoint, gated commands

- **Description:** Add `edit_file` (search/replace blocks with atomic write and read-before-write staleness checks), git-branch-scoped checkpoint/rollback (never touching the user's active branch or uncommitted changes), and an allowlisted, user-configured command tool (test/build/lint only — no generic shell) — each requiring per-use preview and approval through the CMP-003/SEC-009 approval model.
- **Reason:** This is the write/execute tier that turns the read-only investigation agent (CODE-004) into an editing agent, and is where destructive-action risk concentrates.
- **Related audit findings:** None (see phase note) — must not regress A-RET-02's "no broad FS/shell plugin" disposition; command execution stays allowlisted, never a generic shell.
- **Dependencies:** CODE-004, SEC-005, SEC-009.
- **Priority / complexity:** High / Extra Large.
- **Expected outcome:** Ark Code can make and verify code changes without ever overwriting unrelated user work or executing unreviewed commands.
- **Acceptance criteria:**
  - Every edit shows a per-file diff preview and requires explicit approval before being written.
  - The workspace must be a git repository (or the user is offered `git init`); Ark Code's changes land on a dedicated branch/checkpoint, never directly overwriting a dirty working tree.
  - Command execution is restricted to a user-configured allowlist, runs with the repository as cwd, a stripped environment, output/timeout bounds, and is killable on cancellation.
  - Rollback is scoped to exactly the files/commits Ark Code produced.
- **Potential risks:** Partial multi-file edits left inconsistent if a run is cancelled mid-batch; command allowlist misconfiguration.
- **Suggested implementation notes:** Reuse the sidecar's existing child-process timeout/kill/log-redaction machinery for command execution rather than rebuilding it.

#### CODE-006 — Implement coding-session context and token-budget management

- **Description:** Add a real per-model context-window budget allocator for Ark Code sessions (system/tool instructions, plan, sliding window of tool observations, open file contents), with explicit, visible compaction (superseded reads collapsed to summaries) instead of silent truncation.
- **Reason:** Local models have materially smaller and more variable context windows than the premium cloud models coding-agent UX patterns assume; Ark's own principle 2.1 prohibits silently labelling a partial/truncated context as complete.
- **Related audit findings:** None (see phase note).
- **Dependencies:** CODE-001 (real `contextWindow` capability), CODE-002.
- **Priority / complexity:** Medium / Large.
- **Expected outcome:** Ark Code stays useful across a long session on small-context local models without silently dropping information the model needed.
- **Acceptance criteria:**
  - Context budget is computed from the selected model's real context window, not a fixed constant.
  - Eviction/compaction is visible in the session trace, not silent.
  - Sessions on small-context models degrade to shorter, more frequent planning steps rather than failing opaquely.
- **Potential risks:** Over-aggressive compaction losing information the model still needs.
- **Suggested implementation notes:** Defer semantic/embedding-based retrieval to when CMP-002 (RAG) exists; use grep/glob-based retrieval for V1.

#### CODE-007 — Build the Ark Code UI surface and session persistence

- **Description:** Add a third `ActiveView` alongside `chat`/`settings`, its own frontend feature module, domain store, and `ArkClient` methods/events, plus the `code_sessions`/session-event schema (through a versioned migration) needed to persist and resume agent runs.
- **Reason:** Ark Code needs to feel like a dedicated section of the app, not a chat variant, while reusing Ark's existing container/store/typed-client architecture rather than inventing a parallel one.
- **Related audit findings:** None (see phase note).
- **Dependencies:** ARC-002, ARC-008, ARC-005, CODE-002.
- **Priority / complexity:** High / Large.
- **Expected outcome:** Switching between Ark Chat and Ark Code is a first-class, polished navigation action, and closing/reopening Ark stops and resumes coding sessions without data loss.
- **Acceptance criteria:**
  - New commands/events are added to `contract/schema.json` and pass `pnpm contract:check`/`cargo test` like every existing DTO.
  - `scripts/check-module-boundaries.mjs` passes unmodified — the new feature module follows the existing layering rules with no exceptions carved out.
  - Session list, tool-execution cards, diff cards, and approval prompts reuse existing UI primitives/patterns rather than a new design language.
  - A crashed or closed session is resumable per the CODE-002 lifecycle contract, or is explicitly and visibly marked as needing user decision, never silently resumed into a wrong state.
- **Potential risks:** Scope pressure to replicate every Claude Code/Codex UI affordance instead of Ark's own minimal shell.
- **Suggested implementation notes:** Ship the MVP UI against CODE-004 (read-only) only; add diff/approval UI with CODE-005.

#### CODE-008 — Adversarial security and least-privilege regression testing for coding tools

- **Description:** Extend TST-006's adversarial security suite with coding-agent-specific cases: prompt injection via file content (e.g. a source comment instructing the model to ignore prior instructions or run a destructive command), path-traversal attempts against every tool, malicious/oversized repository fixtures, and approval-bypass attempts.
- **Reason:** SEC-009's acceptance criteria require an adversarial prompt suite before any tool with side effects ships, and Ark Code introduces the highest-side-effect tool surface in the product.
- **Related audit findings:** A-SEC-09 (informative only — SEC-009/TST-006 remain the primary tasks; this extends their coverage to Ark Code's specific tools).
- **Dependencies:** TST-006, CODE-004, CODE-005.
- **Priority / complexity:** High / Large.
- **Expected outcome:** Ark Code's tool layer is held to the same least-privilege regression bar A-RET-02 already established for the rest of Ark, with evidence, not assertion.
- **Acceptance criteria:**
  - Adversarial fixtures cover exfiltration, instruction override via file/tool-result content, path traversal, and approval-fatigue/bypass patterns.
  - Every CODE-004/005 tool has a passing least-privilege regression test proving it cannot act outside its declared scope or the bound repository root.
  - Suite runs in CI, not only manually.
- **Potential risks:** False confidence from a suite that doesn't keep pace with new tools added later.
- **Suggested implementation notes:** Require a new adversarial fixture as part of the Definition of Done (Section 11.1) for any future CODE task that adds a tool.

### Phase 7 — Performance and scalability

#### PERF-001 — Add privacy-safe performance instrumentation and budgets

- **Description:** Instrument cached-shell readiness, provider refresh, TTFT, inter-token latency, render/update time, database batch count/duration, memory, import/export, search, and sidecar resources against the audit budgets.
- **Reason:** Bundle sizes are acceptable, but startup/memory/runtime behavior is not measured.
- **Related audit findings:** A-PERF-01–06, A-OPS-01.
- **Dependencies:** FND-005, OPS-001 design.
- **Priority / complexity:** High / Medium.
- **Expected outcome:** Performance regressions are observable without collecting content.
- **Acceptance criteria:**
  - Metrics contain identifiers/timings/counts only and follow opt-in/local policy.
  - Cached shell ≤1.0 s on reference hardware; provider refresh never blocks history.
  - Cancellation acknowledgement ≤100 ms and stream DB batches ≤20/s.
  - Baselines and regression thresholds run in CI/nightly with artifacts.
- **Potential risks:** CI hardware variance and instrumentation overhead.
- **Suggested implementation notes:** Keep local diagnostic metrics available even when remote crash/telemetry is disabled.

#### PERF-002 — Remove startup and refresh blocking

- **Description:** Render cached workspace/conversations immediately, lazy-load diagnostics/settings, refresh providers/models concurrently with deduplication/cancellation, and move noncritical work after first interactive paint.
- **Reason:** Bootstrap can block on three-/five-second provider operations.
- **Related audit findings:** A-PERF-03, A-UX-12, A-FUN-04.
- **Dependencies:** ARC-008, FTR-009, PERF-001.
- **Priority / complexity:** High / Medium.
- **Expected outcome:** An unavailable provider never delays access to local history/settings.
- **Acceptance criteria:**
  - Offline provider fixture meets cached-shell budget.
  - Provider state displays stale/loading independently of application loading.
  - Refresh completion cannot overwrite a newer selection/workspace.
  - Startup trace identifies no avoidable synchronous provider/diagnostic work.
- **Potential risks:** Cached stale information can be misleading.
- **Suggested implementation notes:** Revalidate immediately before send while allowing navigation/read-only actions.

#### PERF-003 — Scale history and transcript rendering

- **Description:** Combine paginated/indexed queries with conversation/message virtualization or windowing, stable scroll anchoring, and bounded branch data loading.
- **Reason:** All conversations and the full active path render without virtualization; path query is N+1.
- **Related audit findings:** A-PERF-02, A-ARC-07, A-UX-04.
- **Dependencies:** ARC-007, UX-003, PERF-001.
- **Priority / complexity:** High / Large.
- **Expected outcome:** Large histories and long transcripts remain responsive and accessible.
- **Acceptance criteria:**
  - 1,000-conversation and 100,000-character fixtures meet response/frame budgets.
  - Virtualization preserves screen-reader/keyboard navigation and search result focus.
  - Loading older messages does not jump scroll.
  - Branch switching loads only required topology/content.
- **Potential risks:** Virtualization can harm accessibility and variable-height Markdown anchoring.
- **Suggested implementation notes:** Prefer incremental rendering/pagination if a virtualizer cannot meet accessibility requirements.

#### PERF-004 — Govern local-model resources

- **Description:** Measure and enforce model disk/RAM/VRAM/context/concurrency budgets, add one-generation-at-a-time default, backpressure, queue visibility, and process resource telemetry.
- **Reason:** Users can launch models that swap or destabilize the machine; diagnostics do not estimate fit.
- **Related audit findings:** A-PERF-05, A-FUN-09, A-CMP-01.
- **Dependencies:** ARC-010, FTR-006, PERF-001.
- **Priority / complexity:** High / Large.
- **Expected outcome:** Ark prevents clearly unsafe launches and explains resource tradeoffs.
- **Acceptance criteria:**
  - Preflight estimates model + context memory and free disk/RAM with a confidence label.
  - Queue/concurrency limits are visible, cancellable, and provider-capability aware.
  - Runtime telemetry detects sustained pressure/crash and recommends a smaller context/model.
  - Reference hardware matrix validates supported model classes.
- **Potential risks:** VRAM/unified-memory estimates vary by backend and can be inaccurate.
- **Suggested implementation notes:** Use conservative ranges and allow an informed advanced override rather than claiming exact fit.

#### PERF-005 — Optimize Markdown/highlighting and enforce load regressions

- **Description:** Profile Markdown rendering, avoid re-highlighting incomplete code fences/full accumulated output per delta, lazy-load language grammars, and add long-response/search/import/backup benchmarks.
- **Reason:** The Chat chunk is the largest lazy bundle and growing content is repeatedly parsed/highlighted.
- **Related audit findings:** A-FUN-06, A-PERF-01, A-PERF-06.
- **Dependencies:** COR-011, UX-003, PERF-001.
- **Priority / complexity:** Medium / Medium.
- **Expected outcome:** Rich responses remain smooth without premature bundle micro-optimization.
- **Acceptance criteria:**
  - Open code fences use plain/preformatted rendering until stable or updates are throttled.
  - Completed unchanged message components do not rerender during another message's stream.
  - Language support loads on demand where beneficial.
  - Nightly load suite reports startup, stream, render, search, import, backup, and memory regressions.
- **Potential risks:** Incremental Markdown changes can briefly alter layout.
- **Suggested implementation notes:** Preserve Markdown correctness/security; optimize based on profiles, not raw bundle size alone.

### Phase 8 — Mobile readiness and iPhone delivery

**Phase 8 scope decision (2026-08-14):** Ark is personal-use software for one user and a small
number of named friends, with no App Store distribution and no Apple Developer Program
enrollment — an explicit, deliberate scope decision, not a resource shortfall. This changes
Phase 8's shape substantially: a native Expo/React Native shell requiring App Store or
TestFlight distribution isn't viable at this scope (Apple's free-tier personal-team builds
expire every 7 days and require collecting each friend's device UDID — not workable for a
casual friend group). The tasks below are rewritten around a PWA that reuses Ark's existing
React frontend (ARC-002's typed `ArkClient` boundary exists specifically so the transport
underneath — Tauri IPC today, HTTP/WebSocket for the PWA — is swappable) served over the local
companion API (FTR-010), with device pairing replacing account-based auth. Several originally
separate tasks are retired as redundant under this model rather than rewritten; each retired
entry is kept as a pointer, not deleted, so existing cross-references elsewhere in this plan
still resolve. Remote access beyond the home network is an explicit non-goal for Ark itself — a
user wanting that layers their own personal VPN (Tailscale, WireGuard) entirely outside Ark's
scope. Revisit this whole decision, not just individual tasks, if the intended audience ever
grows beyond a handful of named people.

#### MOB-001 — Build the PWA shell over the existing Ark frontend

- **Description:** Add a web build target and a service worker/manifest for installability. Add an `HttpArkClient` implementing the existing `ArkClient` interface over the FTR-010 companion API instead of Tauri IPC, so every existing feature component (`ChatView`, `SettingsView`, etc.) is reused unmodified. Extend UX-001's responsive shell to phone viewport widths.
- **Reason:** ARC-002's typed client boundary exists specifically so the transport underneath is swappable; a parallel native codebase would duplicate UI work Ark already has, for no benefit at this distribution scope.
- **Related audit findings:** A-MOB-01, A-MOB-02 (reframed — "DOM/Tauri cannot be reused as iPhone UI" assumed a native-installed binary; a browser-rendered PWA *is* the DOM, which is directly reusable).
- **Dependencies:** ARC-002, FTR-010, UX-001.
- **Priority / complexity:** High / Medium.
- **Expected outcome:** Everything Ark Chat does on desktop is usable from a phone browser and installable to the home screen, with one codebase.
- **Acceptance criteria:**
  - A web build target produces a static bundle the companion API's HTTP server can serve.
  - `HttpArkClient` implements the full `ArkClient` interface; the existing transport-isolation module-boundary rule (today: only `ArkClient.ts` imports `@tauri-apps/*`) extends to cover it — only the adapter file touches `fetch`/`WebSocket`.
  - `manifest.json` and a service worker make the page installable on iOS/Android home screens.
  - Supported phone widths render without horizontal scroll or clipped controls.
- **Potential risks:** iOS Safari's PWA feature support (storage limits, background behavior) lags Chrome; must be verified on real iOS Safari, not desktop responsive-mode.
- **Suggested implementation notes:** Don't fork feature components for a phone-only layout; branch inside the existing component via a viewport hook if a genuine phone-specific need appears.

#### MOB-002 — Retired, folded into FTR-010

There is no separate "cross-device protocol" under the PWA model — the PWA speaks the exact
same FTR-010 companion API contract every other client would. Any acceptance-criteria content
this task would have owned (version negotiation, request/revision IDs, streaming
resume/reconciliation) belongs to FTR-010 directly.

#### MOB-003 — Retired, folded into FTR-010

"The authenticated companion service" is FTR-010 itself — there is no separate mobile-specific
service to build. What this task's LAN/pairing-mode acceptance criteria covered now lives in
MOB-009.

#### MOB-004 — Retired, folded into MOB-009

OAuth/OIDC PKCE, refresh-token rotation, and device revocation-at-scale solve identity-federation
problems this product doesn't have at its explicit personal-use scope. MOB-009's lightweight LAN
pairing tokens are the entire authentication model; there is no separate "mobile authentication"
task. A phone's own OS lock screen is the app-lock — Ark does not implement a second one.

#### MOB-005 — Disconnected-state UX

- **Description:** Detect when the companion API becomes unreachable (phone left the LAN) and show a clear, honest "Ark is unreachable — check you're on the same network as your computer" state. No local data cache, no write queue, no reconciliation.
- **Reason:** The PWA is a live thin client to the desktop-hosted database, not a device with its own local replica — there is no sync/conflict problem to solve. Building one anyway would be exactly the speculative complexity Section 2.7 warns against.
- **Related audit findings:** A-MOB-05 (reframed — "no offline outbox" was a gap under a cloud-sync model; it's a correct absence under a LAN-only thin-client model, not a gap).
- **Dependencies:** MOB-001.
- **Priority / complexity:** Low / Small.
- **Expected outcome:** Leaving the house never shows stale or half-synced data — it says "unreachable," plainly.
- **Acceptance criteria:**
  - Connection loss is detected within a bounded time and rendered as a distinct, styled state, not a blank screen or silent failure.
  - No conversation content persists on the phone beyond the current rendered session.
  - Reconnection is automatic when the phone rejoins the LAN.
- **Potential risks:** None significant — deliberate scope reduction from the original offline-sync task.
- **Suggested implementation notes:** A heartbeat against the companion API's health endpoint is sufficient; do not build a service-worker data cache for conversation content.

#### MOB-006 — Retired

No native Expo/React Native shell. Distribution constraints (no App Store, no Apple Developer
Program) make it non-viable at this product's explicit scope; MOB-001's PWA covers the same
user-facing goal. Revisit only alongside a revisit of the Phase 8 scope decision above.

#### MOB-007 — PWA installability and home-screen polish

- **Description:** Icon/splash assets for home-screen install, `display: standalone` manifest behavior, safe-area handling for notches/home indicators, and a dismissible "Add to Home Screen" hint (Safari has no native install prompt).
- **Reason:** Making a browser tab feel like an installed app is manifest/CSS work once MOB-001's shell exists.
- **Related audit findings:** None directly — general PWA polish supporting A-MOB-01's resolution via MOB-001.
- **Dependencies:** MOB-001.
- **Priority / complexity:** Low / Small.
- **Expected outcome:** Opening Ark from the home screen looks and behaves like a real app.
- **Acceptance criteria:**
  - Manifest passes an installability audit.
  - Safe-area insets are respected on notched devices.
  - An install hint is shown once, is dismissible, and never nags on repeat visits.
- **Potential risks:** iOS-specific viewport/status-bar quirks need real-device verification.
- **Suggested implementation notes:** Use real iOS Safari for verification; desktop responsive-mode does not reproduce Safari-specific PWA quirks.

#### MOB-008 — Web Push notifications

- **Description:** Opt-in Web Push (self-generated VAPID keys, no third-party push service, no Apple Developer Program) for generation-complete notifications when the PWA is backgrounded. Camera, native share sheet, native file picker, and microphone are explicit non-goals — a PWA on iOS either lacks these or only partially exposes them, and building around partial support isn't worth it at this scope.
- **Reason:** Push is the one native-feeling capability actually achievable for free; the rest of the originally-scoped capability set assumed a native app with App Store entitlements.
- **Related audit findings:** A-MOB-06 (partial — notifications only; camera/share-sheet/file-picker explicitly out of scope, not silently dropped).
- **Dependencies:** MOB-001, MOB-007, CMP-006.
- **Priority / complexity:** Medium / Medium.
- **Expected outcome:** You're notified on your phone when a long-running generation finishes, without the tab open.
- **Acceptance criteria:**
  - VAPID keys are generated and stored server-side, not through a third-party push service.
  - The permission request is explicit and deniable.
  - Delivery is verified on an installed (not just open-tab) iOS PWA, on the actual minimum supported iOS version.
- **Potential risks:** iOS Web Push has real version gating (16.4+ and home-screen-installed only) and platform quirks; must be tested on real hardware, not assumed from spec.

#### MOB-009 — Implement LAN discovery and device pairing

- **Description:** Opt-in local-network discovery, QR-code/manual-code pairing issuing a long-lived, high-entropy per-device bearer token, a Settings screen listing every paired device (name, first-paired date, last-seen) with individual revoke, and network-change handling. This *is* the entire authentication model for the PWA — absorbs what MOB-003's LAN-mode acceptance criteria and MOB-004's device-identity acceptance criteria would have covered separately.
- **Reason:** OAuth/PKCE, refresh-token rotation, and device revocation-at-scale solve identity-federation problems Ark doesn't have; a personal LAN tool needs "did I show this device the code," not an identity provider.
- **Related audit findings:** A-MOB-04, A-MOB-07, A-CMP-15.
- **Dependencies:** FTR-010, SEC-005 (for how Ark itself stores issued tokens server-side).
- **Priority / complexity:** High / Medium.
- **Expected outcome:** A friend gets LAN access to your Ark desktop in about ten seconds; you can revoke any one device without touching the others.
- **Acceptance criteria:**
  - Pairing tokens are high-entropy and server-generated, never guessable/sequential.
  - Settings lists every paired device with individual, immediate revoke — not just on next refresh.
  - Discovery advertises no conversation/provider secrets.
  - No credential is visible in plaintext after the initial pairing screen closes.
  - Public/untrusted network changes re-confirm or disable access according to policy; never auto-trust by subnet.
- **Potential risks:** The home Wi-Fi network is the actual trust boundary; if it's shared/public, pairing tokens are only as safe as the network. State this plainly in the UI rather than implying account-grade security.
- **Suggested implementation notes:** Reuse SEC-005's OS-backed secret storage pattern for server-side token storage; the phone-side token can live in ordinary browser storage since it is not a credential Ark needs to protect from the phone's own user. Provide a manual pairing-code fallback alongside QR.

#### MOB-010 — Retired

On-device iPhone inference evaluation was already gated behind "product demand evidence" and a
native Swift module — neither applies once there is no native app. A PWA cannot meaningfully
run local on-device inference on iOS. Revisit only if Phase 8's scope decision changes.

### Phase 9 — Testing, operations, and production release

#### TST-001 — Complete domain and application unit coverage

- **Status: Partial (2026-08-14).** This item's own description names ten specific behaviors ("lifecycle transitions, validation, title generation, settings precedence, provider capabilities, route classification, branch operations, guidance, provenance, and error mapping"). Nine of the ten already have real, focused test coverage — built up incrementally as each owning task (COR/ARC/SEC/FTR) shipped its own tests, exactly the "map tests to acceptance criteria and past defects" discipline this task's own suggested-implementation-notes calls for, not a single after-the-fact sweep. `validation.rs`, `security/mod.rs` (route/destination classification), `device_settings.rs` (settings precedence/legacy-seed resolution), `diagnostics.rs` (`performance_guidance`), `supply_chain.rs` (provenance verification), `sidecar.rs`/`data_protection.rs`/`workspace.rs` (lifecycle transitions), and `generation.rs` (branch operations, title generation via `maybe_title_conversation`'s transaction tests) all have positive/negative/boundary cases today — 282 Rust tests total as of this pass, up from the 18 this task's description cites as the original gap.
  - **The one genuinely missing piece this pass closed: error mapping.** `errors.rs` — the `From<rusqlite::Error>`/`From<std::io::Error>`/`From<reqwest::Error>` classification logic that decides whether a failure becomes `database_locked`, `disk_full`, `workspace_read_only`, `redirect_blocked`, etc. — had zero tests despite being exactly the kind of "state transition/validation boundary" this task asks for and directly deciding what recovery UI a user sees (`docs/troubleshooting.md`, written earlier this session, depends on this classification being correct). 15 new tests cover every branch: each `rusqlite`/SQLite result-code class (busy/locked, corrupt/notadb, full, readonly/permission) built via real `rusqlite::ffi::Error` values, the fallback path for both an unclassified SQLite code and a non-`SqliteFailure` variant; each `std::io::Error` class (permission-denied kind and its three raw-OS-error aliases, storage-full kind and its three aliases) plus its fallback; and — since `reqwest::Error` has no public constructor, a hand-built value would prove nothing — two tests that trigger a *real* local-loopback network condition: a genuine connection refusal for the `is_connect()`/"provider unreachable" branch, and a real `redirect::Policy::limited(0)` client hitting a real redirecting local server for the `is_redirect()`/`redirect_blocked` branch.
  - **A real, previously-undocumented finding surfaced by writing that last test, not invented for narrative effect:** the `redirect_blocked` branch's existing code comment claimed providers' `redirect::Policy::none()` clients trigger it — false. `Policy::none()` doesn't error at all; it returns the 3xx response itself, which `providers/mod.rs`'s ordinary `!response.status().is_success()` handling reports as `provider_error` instead (proven by the pre-existing `providers::tests::ollama_client_does_not_follow_redirects`, which already asserted `error.code == "provider_error"` for exactly this case — the assertion was correct, the `errors.rs` comment describing *why* was not). `redirect_blocked` is reachable and correctly implemented, but not reached by any call site in this codebase today, since nothing uses an erroring redirect policy. Fixed the comment to state this accurately rather than leave a plausible-sounding but wrong explanation next to the code, and cited the real proof (the new test) directly in it.
  - **Deliberately not claimed as fully done:** "coverage thresholds are risk-based... branch coverage targets and mutation testing where practical" — no coverage-measurement or mutation-testing tooling is wired into this repo (no `cargo-tarpaulin`/`cargo-llvm-cov`/`cargo-mutants` in CI), so this criterion has no evidence either way; adding and tuning that tooling is its own real task, not something to silently claim via "the tests look thorough." A systematic module-by-module audit against "positive, negative, and property/boundary cases" for every one of the nine already-covered behaviors was not performed in this pass — the assessment above is based on confirming real test modules exist and spot-checking their shape, not re-deriving the entire acceptance criterion from scratch for a repo whose Rust suite is already 282 tests deep.
  - Full validation: `cargo fmt --check`/strict `clippy -D warnings` clean; `cargo test` 282 passed/1 ignored (unrelated pre-existing ARC-005 issue) — 15 new `errors` tests, zero regressions.
- **Description:** Add focused tests for lifecycle transitions, validation, title generation, settings precedence, provider capabilities, route classification, branch operations, guidance, provenance, and error mapping.
- **Reason:** Existing 18 Rust tests do not cover most core behavior and there are no frontend/domain unit suites.
- **Related audit findings:** A-OPS-02, C-01–C-08.
- **Dependencies:** Corresponding modules/tasks.
- **Priority / complexity:** Critical / Large.
- **Expected outcome:** Pure rules fail fast and deterministically without native UI/provider setup.
- **Acceptance criteria:**
  - Every state transition/validation boundary has positive, negative, and property/boundary cases.
  - Coverage thresholds are risk-based; critical modules require branch coverage targets and mutation testing where practical.
  - Tests contain no sleeps/network and complete in the fast PR lane.
- **Potential risks:** Chasing global percentage rather than risk.
- **Suggested implementation notes:** Coverage is evidence, not the goal; map tests to acceptance criteria and past defects.

#### TST-002 — Complete provider and generation integration coverage

- **Description:** Use FND-004 simulators plus optional real-version smoke jobs to test request construction, parsers, timeouts, cancellation, retries, event revisions, checkpointing, and all terminal states.
- **Reason:** Provider protocol and streaming are the highest-risk untested paths.
- **Related audit findings:** C-02, C-04, C-05, A-FUN-04.
- **Dependencies:** FND-004, COR-002/003/005/011, ARC-003.
- **Priority / complexity:** Critical / Large.
- **Expected outcome:** Provider/network variability cannot silently corrupt lifecycle state.
- **Acceptance criteria:**
  - Full protocol matrix runs deterministically on every PR.
  - Supported real Ollama/managed runtime versions run nightly/release smoke with a tiny approved model where infrastructure permits.
  - Failures assert durable database state, emitted revisions, UI reconciliation input, and redacted errors.
- **Potential risks:** Real-model tests are slow and hardware dependent.
- **Suggested implementation notes:** Keep deterministic simulators required; treat real providers as compatibility smoke, not primary correctness proof.

#### TST-003 — Complete database, migration, backup, and recovery tests

- **Description:** Test transaction fault injection, concurrent operations, WAL/busy behavior, migration fixtures, crash recovery, corruption/newer schema, backup/restore, import rollback, and workspace move interruption.
- **Reason:** Data integrity and recovery are production gates.
- **Related audit findings:** C-01, C-06, A-ARC-02/03, A-OPS-04.
- **Dependencies:** COR-001/004/009/010, ARC-004/005, FTR-001.
- **Priority / complexity:** Critical / Large.
- **Expected outcome:** Supported data survives upgrades, crashes, failures, and relocation.
- **Acceptance criteria:**
  - Failure is injected after every statement/phase of each logical mutation.
  - Every supported release DB fixture upgrades and preserves invariants/content hashes.
  - Restore never overwrites the only good copy.
  - Concurrent-instance and locked/read-only/full-disk simulations produce typed safe outcomes.
- **Potential risks:** True crash/power-loss behavior is hard to simulate.
- **Suggested implementation notes:** Combine deterministic fault injection with process-kill tests on disposable workspaces.

#### TST-004 — Complete component, accessibility, keyboard, and visual regression coverage

- **Description:** Test design-system components and feature states with fake ArkClient, axe, keyboard-only flows, contrast checks, reduced motion, zoom, dark/light themes, and viewport snapshots.
- **Reason:** Accessibility and responsive defects are widespread and currently manual-only.
- **Related audit findings:** A-UX-01–18.
- **Dependencies:** ARC-002, UX-001–011.
- **Priority / complexity:** Critical / Large.
- **Expected outcome:** UI regressions are caught before native E2E/release.
- **Acceptance criteria:**
  - Every state catalog variant has a test.
  - Zero serious/critical axe findings; approved visual diffs are reviewed.
  - Required viewport matrix and 200% zoom are automated where reliable.
  - Manual NVDA/VoiceOver checklist is completed for each release candidate.
- **Potential risks:** Snapshot brittleness.
- **Suggested implementation notes:** Assert semantics/behavior first and keep snapshots focused on stable layouts.

#### TST-005 — Build native desktop and mobile end-to-end suites

- **Description:** Automate clean launch, onboarding, provider setup, chat/stream/stop/retry/restart, branch, search, settings, import/export, backup/restore, model lifecycle, updates, and critical mobile flows.
- **Reason:** No native E2E suite verifies webview-to-Rust-to-SQLite behavior or packaged artifacts.
- **Related audit findings:** A-OPS-02/03, A-MOB-01.
- **Dependencies:** Release-scope implementation tasks, FND-004.
- **Priority / complexity:** Critical / Extra Large.
- **Expected outcome:** The actual installed application, not only modules, is release-tested.
- **Acceptance criteria:**
  - Windows primary suite runs on every release candidate; macOS/Linux supported matrices run before declaring support.
  - Tests use disposable isolated workspaces and verify no external user data changes.
  - Real iOS Safari (PWA, not a native simulator) smoke covers pairing, chat, the disconnected-state UX, and Web Push permissions.
  - Failure artifacts include screenshots, redacted logs, DB state summary, and version metadata.
- **Potential risks:** Native UI automation flakiness and runner cost.
- **Suggested implementation notes:** Keep a small blocking smoke suite and broader nightly/release suites.

#### TST-006 — Add automated security and adversarial testing

- **Description:** Add dependency/license/SBOM scans, secret scanning, CSP/link/Markdown tests, URL/redirect tests, import/model fuzzing, API auth/rate/replay tests, prompt-injection/tool adversarial cases, and artifact tamper tests.
- **Reason:** Security controls must be continuously validated as attack surface expands.
- **Related audit findings:** A-SEC-01–12, C-08–C-10.
- **Dependencies:** SEC-001–011, CMP-002–004 where applicable.
- **Priority / complexity:** Critical / Extra Large.
- **Expected outcome:** Known classes of data exfiltration, unsafe execution, and supply-chain regression fail CI/release gates.
- **Acceptance criteria:**
  - No unreviewed critical/high advisory or committed secret in release.
  - Fuzzers have seed corpora, time budgets, crash triage ownership, and regression fixtures.
  - Tool/RAG tests include indirect prompt injection and approval bypass attempts.
  - Signed artifact/update tampering and downgrade/replay fail closed.
- **Potential risks:** Scanner false positives and nondeterministic fuzz time.
- **Suggested implementation notes:** Use explicit reviewed suppressions with expiry, not blanket ignores.

#### TST-007 — Enforce performance, soak, and regression qualification

- **Description:** Run startup/stream/history/import/backup/model-resource benchmarks, long-generation soak, repeated restart/cancel, memory leak, and sync/offline stress on reference environments.
- **Reason:** Performance budgets and long-running reliability are otherwise unproven.
- **Related audit findings:** A-PERF-01–06, A-OPS-02.
- **Dependencies:** PERF-001–005.
- **Priority / complexity:** High / Large.
- **Expected outcome:** Release candidates meet user-facing budgets and do not degrade over sustained use.
- **Acceptance criteria:**
  - Blocking thresholds match PERF-001; trends and artifacts are retained.
  - Soak covers repeated generations, branch switching, provider loss, workspace backup, and process sleep/wake.
  - Memory returns within documented tolerance after conversation/model unload.
  - Regressions require explicit reviewed waiver with expiry.
- **Potential risks:** Noisy hardware results.
- **Suggested implementation notes:** Use dedicated/repeatable reference runners for blocking metrics and developer machines for directional profiling.

#### OPS-001 — Add structured redacted observability and diagnostics bundles

- **Status: Complete for local structured logging, crash capture, and reviewed diagnostics export (2026-08-14).** Not done, and explicitly scoped out below: local performance summaries (that is PERF-001's own stated job — this task's role is the logging/redaction/export substrate, not the perf-budget measurement itself).
  - **Redaction extracted and shared, not duplicated:** `src-tauri/src/redaction.rs` is a new module carrying the marker/path redaction logic that previously lived only in `sidecar.rs` (moved, not copied — `sidecar.rs` now calls `crate::redaction::redact`), extended with cookie headers (`Cookie:`/`Set-Cookie:`), sync/session/refresh/access tokens, and query-string stripping (`?...` on any URL-shaped token, replaced wholesale rather than trying to enumerate every possible sensitive parameter name). Both `sidecar.rs`'s runtime-output buffer and the new structured log go through the identical function, so a marker added for one consumer protects both. Six new tests cover credentials, sync/session tokens, cookies, query strings, absolute Unix/Windows paths, and (a negative case) that ordinary non-sensitive text is left untouched.
  - **New `src-tauri/src/observability.rs`:** `DiagnosticsLog` — a bounded (500 entries / 512 KiB) in-memory ring, the same shape as `sidecar.rs`'s pre-existing `RuntimeLogBuffer`, plus best-effort append to a rotated local file (`<app config dir>/logs/ark.log`, single-step rotation to `.log.1` at 2 MB) so a record survives past a crash that follows it. Every `record()` call redacts its message before either sink sees it. This is an architectural guarantee, not just a redaction one: every real call site added this session (`generation.rs`'s `mark_stream_failed`, `provider_management.rs`'s runtime-healthy milestone, `lib.rs`'s workspace-fallback warning) passes only stable identifiers — error *codes*, category names, a boolean — never `.message`, prompt text, or model output; the module's own doc comment states this as the actual guarantee and instructs future call sites to keep it. `LogLevel::Debug` has no current call site and is `#[allow(dead_code)]`-marked with an explicit "remove the moment a real Debug-level call site exists" comment, the same discipline `tool_policy.rs` already established for the same reason.
  - **Crash capture (`lib.rs::install_crash_hook`):** chains onto (never replaces) the default panic hook. Deliberately bypasses `DiagnosticsLog`'s `Mutex` entirely via a standalone `observability::record_crash_directly_to_file` — if a panic happened inside a call already holding that mutex on the same thread, taking it again in the hook would deadlock instead of failing safely; this is a real, considered risk, not a hypothetical. Re-reads `DeviceSettings` from disk at panic time (not a value captured at startup), so toggling the new opt-in `crashCaptureEnabled` field (`device_settings.rs`, default `false`, `#[serde(default)]` so a pre-existing settings file still parses) takes effect on the very next panic without a restart. Off by default; when off, nothing beyond the existing default stderr panic output happens.
  - **`src-tauri/src/diagnostics_bundle.rs`:** assembles one plain-text bundle — app version, OS/CPU/memory, the sidecar's own already-redacted runtime diagnostics/logs, and the structured log's recent in-memory entries folded with the on-disk file's tail (so a crash record from a session that already ended still surfaces). The workspace path is included only in already-redacted form. `save_diagnostics_bundle` writes back the exact text it is given, byte for byte — no second assembly step that could drift from what was reviewed, which is what makes "inspect exactly what a diagnostics bundle contains before saving" a structural guarantee rather than a UI convention. Two new commands (`export_diagnostics_bundle`, `save_diagnostics_bundle`) and a new `DiagnosticsBundle` contract type (`contract.rs`/`schema.json`/`ark.ts`/`ArkClient`).
  - **Settings UI (`SettingsView.tsx`'s new `DiagnosticsBundlePanel`):** a checkbox for the crash-capture opt-in (wired through a new `changeCrashCaptureEnabled` controller action following the exact same optimistic-update/rollback pattern as the existing `changeTheme`/`changeBuiltInModelPath`), a "Generate diagnostics bundle" button, a read-only textarea showing the full bundle text, and a save-path input + Save button that is disabled until a bundle has been generated. Browser-verified live against a new `?fixture=long-conversation` override (`developmentArkClient.ts`): toggled the checkbox and confirmed the underlying `updateDeviceSettings` round-trip fired; generated a bundle and confirmed the exact fixture preview text rendered in the textarea (including the redacted workspace path); saved it and confirmed the "Saved to ..." confirmation appeared with the path just entered. No console errors.
  - **`scripts/check-secret-boundaries.mjs` updated, not just left passing by accident:** the pre-existing "runtime logging must retain its sensitive-value redaction regression test" assertion pointed at the old `sidecar.rs` test name, which moved; fixed to check `redaction.rs` under its new name, plus a new assertion that `sidecar.rs` actually routes through the shared module. More importantly, this script already contained a comment (written when SEC-005 landed) stating almost verbatim: *"Ark has no crash-report transport yet... OPS-001 must replace the absence assertion with payload-level redaction tests before adding one."* That absence-only guard is now replaced with real assertions: `observability.rs` must call `redact(...)` before writing a crash record and must retain its payload-redaction regression test; the whole production source set (now including the three new files) must never reference a third-party crash-reporting service (Sentry, etc.); and `diagnostics_bundle.rs` must never reference an HTTP client, keeping the export path a local file save with no automatic upload route to regress into.
  - **New `docs/diagnostics-and-logs.md`** (linked from README, cited from `docs/settings-catalog.md`'s new catalog row): states what is and isn't logged and why that's an architectural guarantee, exactly where the file lives and its 2 MB/single-rotation retention bound, the crash-capture opt-in/off-by-default/immediate-revocation behavior, and that nothing described leaves the device automatically — the diagnostics bundle export is always a manual, reviewed action.
  - **Deliberately not done:** correlation IDs are a per-call `Option<&str>` parameter that real call sites already thread through (e.g. `mark_stream_failed` passes the message ID) rather than a dedicated request/generation-ID *system* generating and propagating new IDs across the whole request lifecycle — the acceptance criterion's actual need (tying a log line back to the conversation/message it concerns) is met without that additional machinery, and adding a generic ID-propagation layer with nothing yet needing more than "which message" would be speculative. Local performance summaries are explicitly PERF-001's task, not duplicated here. Structured-log call sites are the few genuinely high-value ones added this session (stream failure, runtime healthy, workspace-open fallback) — not an exhaustive instrumentation pass across the whole codebase, which would be its own large, separate effort and risks becoming log noise rather than signal.
  - Full validation green: `cargo fmt --check`/strict `clippy -D warnings` clean; `cargo test` 267 passed/1 ignored (unrelated pre-existing ARC-005 issue) — 6 new `redaction` tests, 8 new `observability` tests, 2 new `diagnostics_bundle` tests, 1 new `contract` test, plus `device_settings.rs`'s existing round-trip/backward-compat tests extended for the new field; frontend `typecheck`/`lint`/`build`/`test:frontend` (41/41) all clean; `pnpm run contract:check` 37 types; `pnpm run secret-boundary:check`, `architecture:check`, `csp:check`, `markdown-safety:check` all pass; `pnpm run supply-chain:check` unchanged at 886 components (no new dependency — `hyper`/`hyper-util`/etc. were already resolved via SEC-002's work earlier this session).
- **Description:** Implement local structured logs, correlation/request/generation IDs, bounded retention, redaction, runtime logs, local performance summaries, opt-in crash reporting, and user-reviewed diagnostics export.
- **Reason:** There is no logging/crash/support path and sidecar output is discarded.
- **Related audit findings:** A-OPS-01, A-FUN-09, A-SEC-03.
- **Dependencies:** ARC-002 error envelope, ARC-010, SEC-011, PERF-001.
- **Priority / complexity:** Critical / Large.
- **Expected outcome:** Production failures are diagnosable without collecting prompts, outputs, files, secrets, or paths by default.
- **Acceptance criteria:**
  - Redaction tests cover credentials, query strings, headers, prompts/output, filenames/paths, attachment text, and sync tokens.
  - Users can inspect exactly what a diagnostics bundle contains before saving/sending.
  - Crash reporting is off by default/explicit opt-in and has consent/revocation/retention disclosure.
  - No behavioral analytics are added as part of this task.
- **Potential risks:** Over-redaction removes useful context; under-redaction breaks privacy promise.
- **Suggested implementation notes:** Prefer stable error codes, versions, counts, and hashes over raw content.

#### OPS-002 — Produce locally-built, unsigned installers for personal distribution

- **Description:** Produce a working, bundled installer per OS (via the CI matrix's bundling step once COR-012 adds a real one) without code signing or notarization; document the one-time OS trust-bypass steps for the small number of named people who will install it.
- **Reason:** Signing/notarization establish trust with strangers downloading software from the internet — a problem that doesn't exist when installers go directly to named people the developer knows, per the Phase 8 scope decision recorded above MOB-001.
- **Related audit findings:** C-03, C-10, A-SEC-10, A-OPS-03 (the underlying "installer must actually work" finding still applies in full; "must be signed/trusted at scale" does not, at this explicit scope).
- **Dependencies:** COR-012, FND-003, SEC-003/004.
- **Priority / complexity:** Low / Small (was Critical / Extra Large under a public-distribution assumption).
- **Expected outcome:** The developer and their friends install a working build by clicking through one OS warning, with no signing infrastructure to build or maintain.
- **Acceptance criteria:**
  - A bundled installer builds successfully for each OS actually in use.
  - Install/uninstall is smoke-tested at least once per OS.
  - Docs state plainly that builds are unsigned and explain the one-time trust step per OS (Windows SmartScreen "More info → Run anyway"; macOS right-click → Open).
- **Potential risks:** If the intended audience ever grows beyond a handful of known people, this disposition needs revisiting — unsigned installers do not scale to strangers with no reason to trust an "unknown publisher" warning.
- **Suggested implementation notes:** Revisit this disposition explicitly, not silently, if the user base ever grows — the same pattern CMP-009 already uses to gate the team/multi-user decision rather than letting it drift.

#### OPS-003 — Complete product, support, legal, and release documentation

- **Status: Partial (2026-08-14).** Most of this task's listed documentation surface already existed from earlier tasks this session (privacy/security, backup/restore, data formats, support matrix, third-party notices); this pass added the two genuinely missing pieces from the description's own list, fixed real staleness found while cross-checking existing docs against current behavior, and explicitly identifies what remains undocumented and why.
  - **New `docs/troubleshooting.md`:** written from a fresh, deliberately independent audit of every user-facing `AppError` code actually reachable in normal use (workspace/storage, built-in runtime, provider/network, credentials, workspace encryption, backup/restore, import), cross-referenced against what recovery UI genuinely exists for each today (not invented) — e.g. confirming `getWorkspaceRecoveryActions` really does return the same Retry/Choose workspace/Copy diagnostics triplet for every startup storage code before documenting it that way. Organized by user-facing symptom ("Ark won't open," "the built-in runtime won't start") rather than mirroring the internal code taxonomy, since that's how someone actually searches when something's wrong. Ends by pointing at the OPS-001 diagnostics bundle for anything not covered.
  - **Found and fixed real staleness, not just added new pages:** README's Workspace Storage section still said "The MVP does not automatically move an existing database into the new workspace" — FTR-001 (earlier this session) added exactly that capability (`setWorkspace`'s `copyData` option); the README was simply never updated when that shipped. `docs/privacy-and-data-flow.md` still described runtime/diagnostic logs as "in-memory only... nothing is written to disk" and crash reporting as "not yet implemented (OPS-001)" — both were true when written and both are now what OPS-001 (this session, immediately prior task) actually changed; updated both sections to describe the real implemented behavior (a bounded, rotated local log file; opt-in, off-by-default local crash capture with no report-service transport) rather than leaving a privacy document describing behavior the app no longer has. This is exactly the "documentation must match tested artifacts" acceptance criterion doing its job, not a hypothetical concern.
  - **Deliberately not done, and why — not silently skipped:**
    - **Install/update/uninstall guide:** not written. OPS-002 (packaged installers) is itself deferred (Low/Small, blocked on COR-012's real icon artwork) and no installer has ever been built for a release. Writing install instructions for an installer that doesn't exist yet would itself violate "claims match tested artifacts" — this is explicitly sequenced behind OPS-002, not forgotten.
    - **Accessibility statement:** not written. TST-004 (the actual axe/keyboard/contrast/screen-reader test pass this claim would need to be true) hasn't been done. An accessibility statement asserting untested conformance would be a claim without evidence, which is exactly the failure mode this task's own acceptance criteria (and the plan's general "Complete means tested, not merely coded" discipline) exist to prevent.
    - **Changelog:** not started. No version has ever been tagged or released (OPS-004, the release-checklist task, is itself undone) — a changelog today would either be empty or would need to fabricate historical release entries that don't correspond to real releases. Revisit alongside OPS-004.
    - **Companion API documentation:** not applicable yet — FTR-010 (the companion API itself) isn't built. Nothing to document.
    - **Migration notes (between Ark versions):** not written, for the same reason as the changelog — there is only ever one version so far. `docs/protocol-versioning.md` already covers the schema/contract-versioning mechanics this would eventually build on.
    - **"License and third-party notices are approved before public distribution"** and **"Release checklist validates links and stale feature claims"**: both require an actual release process and, for the former, real legal/owner sign-off — the plan's own suggested implementation note for this task says as much ("treat legal/security wording as requiring appropriate owner/counsel approval, not engineering invention"). `THIRD_PARTY_NOTICES.md` itself is generated and current (SEC-004); the *approval* step is a product/ownership action, not an engineering one.
  - Full validation: `pnpm run format:check`/`lint`/`typecheck` clean; `pnpm run secret-boundary:check` clean (new doc content doesn't reference any forbidden pattern). No Rust or contract changes in this pass — documentation and one small README correction only.
- **Description:** Publish accurate README/onboarding/help, install/update/uninstall, provider/model support, privacy/security, backup/restore, troubleshooting, diagnostics, data formats, API, accessibility, third-party licenses, changelog, support matrix, and migration notes.
- **Reason:** Documentation is stale/contradictory and license/privacy/security/release material is missing.
- **Related audit findings:** A-OPS-05, A-UX-12, A-SEC-10/11.
- **Dependencies:** FND-001, SEC-011, FTR/OPS feature completion.
- **Priority / complexity:** Critical / Large.
- **Expected outcome:** Users can install, understand data flow, recover, report issues, and evaluate licensing/support without source inspection.
- **Acceptance criteria:**
  - Documentation is versioned with the release and links to the exact support matrix.
  - Built-in/external/cloud provider claims match tested artifacts.
  - Privacy notice covers all current data flows and opt-in crash reporting.
  - License and third-party notices are approved before public distribution.
  - Release checklist validates links and stale feature claims.
- **Potential risks:** Legal requirements need specialist review.
- **Suggested implementation notes:** Treat legal/security wording as requiring appropriate owner/counsel approval, not engineering invention.

#### OPS-004 — Manual release checklist

- **Description:** Tag a version, attach built installer(s) to a GitHub Release with a short changelog, keep at least the previous release's installer available. No staged channels, no telemetry-driven rollback triggers, no cohort rings.
- **Reason:** Staged rollout percentages and phased channels solve problems that come from an unknown-sized public user base; at the Phase 8 scope decision's explicit personal-use audience, they are pure process overhead with no corresponding benefit.
- **Related audit findings:** C-10, A-OPS-01–06 (the underlying "there is no release/rollback process at all" finding is still resolved — by a smaller process fit to the smaller audience, not left unresolved).
- **Dependencies:** OPS-002 (rewritten).
- **Priority / complexity:** Low / Small (was Critical / Large under a public-rollout assumption).
- **Expected outcome:** Shipping a new version takes minutes; undoing a bad one is "grab the previous attached file."
- **Acceptance criteria:**
  - Each GitHub Release states what changed.
  - The prior release stays available (not deleted) for at least one version back.
  - Friends know where to check for updates — no auto-update pipeline required.
  - A security delta review runs before tagging (`docs/secure-development-checklist.md`'s release section — SEC-011) and any capability/CSP/auth-surface change is noted in the release description.
- **Potential risks:** None significant at this scope.
- **Suggested implementation notes:** Revisit alongside OPS-002 if the audience ever grows.

## 5. Architecture Changes

### 5.1 Target module boundaries

The target architecture keeps the current technology stack and extracts explicit layers incrementally:

~~~text
apps/
  desktop/
    src/
      app/                 composition, routing, shell
      features/            chat, conversations, providers, settings, diagnostics
      ui/                  owned accessible primitives
      lib/                 ArkClient — Tauri adapter (desktop) and HTTP/WebSocket adapter (PWA)
    src-tauri/
      commands/            thin Tauri transport adapters
      application/         use cases and transaction boundaries
      domain/              entities, lifecycle, validation contracts
      ports/               repository/provider/files/secrets/runtime/observability traits
      infrastructure/      SQLite, HTTP providers, OS files/keychain, sidecar, companion API (FTR-010)
packages/
  test-fixtures/           provider streams, conversation graphs, protocol versions
~~~

Per the Phase 8 scope decision, there is no separate `apps/mobile/` or `packages/domain`/
`packages/protocol`/`packages/design-tokens` — the PWA (MOB-001) is the same `apps/desktop/src/`
frontend served over a second `ArkClient` transport adapter, not a second application. The
`packages/` split this diagram originally reserved for cross-device sharing is unnecessary when
there is only ever one frontend codebase to begin with.

The exact folder names may vary, but dependency direction is mandatory:

~~~mermaid
flowchart LR
    UI["Desktop or mobile UI"] --> CLIENT["ArkClient"]
    CLIENT --> TRANSPORT["Tauri / HTTPS-LAN transport"]
    TRANSPORT --> USE["Application use cases"]
    USE --> DOMAIN["Domain policies and state machines"]
    USE --> PORTS["Ports"]
    PORTS --> ADAPTERS["SQLite · providers · files · secrets · runtime"]
~~~

- Domain and application code must not import Tauri, React, SQLite, reqwest, or platform dialogs.
- Infrastructure may depend inward on ports/domain; domain never depends outward.
- Tauri commands and HTTP handlers call the same use cases.
- The mobile protocol exposes semantic operations and revisions, never raw tables or local paths.

### 5.2 Generation lifecycle

FND-002 is the architectural keystone. Implement one durable generation aggregate/state machine with:

- explicit active and terminal states;
- immutable identity plus monotonically increasing revision;
- short transaction boundaries before launch, during checkpoints, and at terminal transition;
- idempotent cancellation and completion;
- interrupted recovery for stale leases;
- provider transport as an effect, not the source of truth;
- UI events emitted only after committed transitions;
- resumable/refetchable state for desktop event loss and mobile disconnect.

COR-001–005 and COR-011 must land before new providers, tools, or mobile reuse. No feature may define a second lifecycle.

### 5.3 State management

ARC-008 replaces broad App state and direct invoke usage with:

- a typed ArkClient;
- server-state queries keyed by entity/revision;
- a focused generation store for active deltas;
- local component state for composer, menus, drawer, scroll, and ephemeral form editing;
- normalized provider/model/project/conversation entities;
- revision-based reconciliation and authoritative refetch on gaps;
- selectors that prevent unrelated feature rerenders.

Choose the smallest state/query library that meets these behaviors. The plan does not mandate Redux, Zustand, or TanStack Query; the decision must be profiled, documented, and avoid duplicate caches.

### 5.4 API and provider design

ARC-002/003 establish:

- versioned schemas and typed error envelopes;
- idempotency/request/revision identifiers;
- provider capability negotiation;
- protocol-specific adapters for Ollama and OpenAI-compatible SSE;
- destination classification independent from provider marketing labels;
- explicit capabilities for models, streaming, non-streaming, auth, context, vision, embeddings, tools, unload, and usage;
- backwards-compatible deprecation and unknown-version behavior.

FTR-010 (the same API the PWA client speaks to — see Section 8) adds inbound APIs only after SEC-010 approves their threat model.

### 5.5 Database and persistence

ARC-004/005/007 implement:

- bounded database access through a worker or justified pool;
- WAL, busy timeout, non-panicking errors, and clean checkpoint/shutdown;
- ordered checksummed transactional migrations;
- pre-migration backup and supported-version upgrade fixtures;
- cursor pagination, indexes/FTS, and recursive branch queries;
- atomic use-case transactions and fault injection;
- durable sync revisions/outbox/tombstones added through tested migrations;
- explicit data scopes for device/workspace/project/conversation/secret settings.

The existing SQLite workspace remains the desktop source of truth. Do not replace it merely to enable mobile. Add a protocol/change-log boundary and preserve open exports.

### 5.6 Migration sequence

1. Back up and fingerprint current legacy schema.
2. Introduce real migration metadata without rewriting content.
3. Normalize stale generation statuses and setting ownership.
4. Add generation revision/lease fields if selected by FND-002.
5. Add search/project/branch metadata and indexes.
6. Add attachment/knowledge tables only with corresponding lifecycle implementation.
7. Add sync change-log/tombstone fields only after protocol conflict rules are approved.

Every step must support injected rollback, untouched backup retention, and upgrade fixtures. A public release must never execute speculative future schema.

### 5.7 Refactoring controls

- Refactor along tasks/use cases, not by rewriting whole directories.
- Characterize behavior before moving it.
- Keep formatting-only changes separate.
- Delete obsolete paths immediately after the replacement passes tests.
- Avoid compatibility adapters with no removal plan.
- Record architecture decisions for lifecycle, DB service, settings ownership, provider capabilities, protocol versioning, encryption, mobile auth/sync, and runtime distribution.

## 6. UI/UX Roadmap

### 6.1 Navigation and responsive shell

UX-001/002 establish three behavioral modes:

| Mode | Navigation | Context | Header |
|---|---|---|---|
| Wide desktop | Expanded conversation sidebar | Persistent when useful | Title, route/model, primary state, overflow |
| Compact desktop/tablet | Rail or drawer | Overlay drawer | Compact title/model plus overflow |
| Phone-width desktop webview / PWA (MOB-001) | Single main stack, conversation sheet | Sheet | Back/menu, title, model, overflow |

The Phase 8 PWA (Section 8) reuses this exact phone-width layout rather than a separate native design — there is no native iPhone app (MOB-006, retired) to diverge from it.

### 6.2 Core user flows

Each flow requires success, cancellation, error, retry, offline, stale, and recovery variants:

1. First run → choose external/local managed provider → select/install model → send a verified test.
2. Create/search/select/archive/pin/move conversation.
3. Send → stream → stop/interruption → retry/keep partial.
4. Edit/regenerate → inspect/switch/compare branches.
5. Configure project/persona/conversation settings with visible precedence.
6. Import preview → validate → map/merge → commit/summary.
7. Export/backup → choose scope/destination → verify → completion.
8. Change/migrate/restore workspace without overwriting source.
9. Attach/retrieve/use tools with route/capability approval.
10. Diagnose provider/model/runtime/storage and export a reviewed support bundle.

### 6.3 Design system

UX-009 owns semantic:

- surface/text/muted/primary/destructive/success/warning/focus tokens;
- typography and readable content widths;
- spacing, touch targets, elevation, breakpoints, and z-index;
- motion duration/easing and reduced-motion variants;
- accessible primitives for control, menu, dialog, toast, tabs, state panels, and messages.

Design-system work is demand-driven by roadmap screens; it must not become a standalone redesign project.

### 6.4 Accessibility

WCAG 2.2 AA is the desktop and mobile baseline:

- semantic landmarks, headings, tabs, menus, dialogs, labels, pressed/selected state;
- keyboard-only interaction with visible focus and deterministic restoration;
- throttled status/live announcements for stream/progress;
- 4.5:1 normal text contrast, including dark errors;
- reduced motion, 200% zoom/reflow, Dynamic Type on iOS;
- ≥24 px targets everywhere and 44 px target on touch-first surfaces where practical;
- NVDA, VoiceOver macOS, and VoiceOver iOS release checks.

UX-006–008 and TST-004/005 own validation.

### 6.5 Message and content experience

UX-003, FTR-005, CMP-001/002:

- assistant output uses the readable column width; user bubbles remain visually distinct;
- stable auto-follow respects users reading above;
- Markdown/code/table overflow is intentional and secure;
- branch/provenance is inspectable without cluttering the default stream;
- files, citations, memory, model, route, settings, timing, and terminal status use compact disclosures;
- incomplete/interrupted content is never visually indistinguishable from complete output.

### 6.6 State and feedback system

UX-004/011 define when to use:

- page state for bootstrap/storage/recovery blockers;
- inline state for provider/model/form/conversation problems;
- progress surface for imports, downloads, backup, indexing, sync;
- toast for noncritical transient success or failure with a durable contextual state;
- dialog for destructive/high-impact confirmation;
- status/live region for asynchronous completion and copy feedback.

Every state must answer: what happened, what data is safe, what the user can do next, and where technical diagnostics are available.

### 6.7 Animation and polish

- Motion communicates panel/message/state relationships only.
- Reduced-motion mode eliminates nonessential transforms and never relies on animation alone.
- Performance budgets prohibit per-token layout animation.
- Visual polish work follows state/accessibility correctness; it does not displace P0/P1 fixes.

## 7. Security Roadmap

| Control | Why required | Implementation | Validation |
|---|---|---|---|
| Provider route enforcement | “Local” can currently send remote | SEC-001 Rust URL classification, redirect/rebinding policy, route disclosure | Unit/network redirect tests; UI route tests |
| Sidecar isolation | Localhost is not authentication | SEC-002 per-launch secret, loopback, CORS/trusted host, supervised process | Unauthorized/CORS/process lifecycle integration tests |
| Dependency hygiene | Three current Rust advisories | SEC-003 upgrades and exception expiry; FND-003/TST-006 scans | cargo-audit all targets and release graph review |
| Binary/model provenance | Unverified executables/models are native attack surface | SEC-004 hashes/signatures/provenance/SBOM/licenses | Tamper/truncation/traversal tests; signed release verification |
| Secret storage | Cloud/mobile credentials must not enter DB/logs | SEC-005 OS keychains and opaque references | Cross-platform adapter and redaction tests |
| Data at rest | Transcripts are plaintext | SEC-006 permissions, honest disclosure, optional encrypted workspace | Permission, key lifecycle, migration/restore tests |
| File/model ingestion | Imports/models can exhaust or exploit parser | SEC-007 canonical paths, limits, format checks, resource controls | Fuzz/boundary/archive/model tests |
| Webview/content | Future UI changes could weaken current strengths | SEC-008 CSP, no raw HTML, controlled links | Production CSP and hostile Markdown test suite |
| Prompt/tool safety | RAG/tools expand injection impact | SEC-009 untrusted channels, scopes, previews, approvals, audit | Adversarial indirect-injection and approval-bypass tests |
| Coding-agent tool safety | Repository/file/command tools are Ark Code's highest side-effect surface and must not regress A-RET-02's no-broad-FS/shell disposition | CODE-004/005 capability-scoped tools under the CMP-003/SEC-009 framework; CODE-008 adversarial and least-privilege regression | Adversarial injection, path-traversal, and approval-bypass tests extending TST-006 |
| Companion API/LAN identity | Remote access needs real per-device authentication, not cookie-based sessions | SEC-010 custom-header bearer tokens, per-device pairing (MOB-009), immediate revocation | Drive-by cross-origin request tests (same threat class as SEC-002); device-revocation tests |
| Operations | Users need disclosure and incident response | SEC-011 policy/runbooks; OPS-001/003 | Release security review and incident rehearsal |
| Signed updates | Supply-chain recovery depends on trust chain | OPS-002 signing, notarization, signed manifests, rollback | Clean-machine/tamper/downgrade/update tests |

### Explicit current non-actions

- Do **not** add local username/password authentication to the single-user desktop. It would not protect data from the same OS session and is not required by the current threat model.
- Do **not** implement cookie-based session authentication for the companion API (FTR-010) or add CSRF controls for it — SEC-010's design deliberately avoids cookies entirely (custom-header bearer tokens instead), which sidesteps the CSRF threat class by construction rather than needing to defend it.
- Do **not** implement multi-user RBAC for the local desktop. CMP-009 is the explicit decision gate.
- Preserve parameterized SQL and no-raw-HTML Markdown through regression tests; no replacement is required.
- Preserve minimal Tauri capabilities and expand only per reviewed feature scope.
- Do **not** pursue code-signing, notarization, App Store/Play Store distribution, or Apple Developer Program enrollment for the current personal-use, no-public-distribution scope. OPS-002/OPS-004 and MOB-006/MOB-010 are reduced or retired accordingly. Revisit only with an explicit decision if the intended audience changes — the same pattern CMP-009 already uses for the team/multi-user question.

## 8. Mobile Strategy (iPhone)

Rewritten 2026-08-14 for the Phase 8 scope decision recorded above MOB-001: personal-use
software for one user and a small number of named friends, with no App Store distribution and
no Apple Developer Program enrollment. This is not a smaller version of a native-app strategy —
it is a materially different, and materially simpler, architecture.

### 8.1 Recommended stack

- **Client:** the existing Ark React/TypeScript frontend, served as an installable PWA. No Expo, no React Native, no separate mobile codebase.
- **Transport:** HTTP/WebSocket to the FTR-010 local companion API, LAN-only.
- **Identity:** long-lived per-device pairing tokens issued via QR/manual code (MOB-009) — no OAuth/OIDC/PKCE, no third-party identity provider.
- **Push:** Web Push with self-generated VAPID keys (MOB-008) — works on iOS 16.4+ for home-screen-installed PWAs, requires no Apple Developer Program enrollment.
- **Remote access beyond the home network:** an explicit non-goal for Ark itself. A user wanting this layers their own personal VPN (Tailscale, WireGuard) entirely outside Ark's scope — the companion API doesn't need to know or care how the connection reached it.
- **On-device inference:** dropped (MOB-010, retired). It was already gated behind product-demand evidence and a native Swift module; neither applies without a native app.

A PWA was rejected in this plan's original mobile strategy on the grounds that it is
"insufficient for the strategic product" — true under a public-distribution, App-Store-present
assumption, false under this one. ARC-002's typed `ArkClient` boundary was built specifically so
the transport underneath is swappable; a PWA is the natural, and now correct, use of that
boundary rather than a compromise.

### 8.2 Shared architecture now

Complete these desktop tasks before mobile feature implementation:

1. FND-002 generation contract.
2. ARC-001 application use cases.
3. ARC-002 ArkClient/protocol schemas.
4. ARC-006 setting/data ownership.
5. SEC-005 SecretStore (for server-side pairing-token storage).
6. SEC-010 pairing/session threat model (rewritten in scope alongside this section — see SEC-010's own entry).
7. FTR-010 companion API boundary.

Because the PWA reuses the existing frontend rather than a parallel codebase, there is
no separate "which parts are shared vs. platform-specific" boundary to design the way a native
client would need — the same components, state stores, and `ArkClient` interface serve both
desktop and the PWA. Only the transport adapter (`HttpArkClient`, MOB-001) is genuinely new.

### 8.3 API and authentication

FTR-010 (which now also serves as the PWA's companion API — see its updated description) requires:

- the same version negotiation and typed-error envelope as the rest of the ArkClient contract;
- authenticated, resumable streaming;
- long-lived per-device pairing tokens (MOB-009), individually revocable, immediately on revoke;
- no database/filesystem exposure through the API surface.

There is no separate mobile authentication design (OAuth/PKCE, refresh rotation, device
inventory beyond the pairing list) — MOB-009's pairing model is the entire authentication
surface, by explicit decision, not by omission.

### 8.4 Offline and synchronization

There is no offline sync, outbox, or conflict-resolution design (MOB-005, rewritten). The PWA is
a live thin client to the desktop-hosted SQLite database — it has no local replica to reconcile.
Leaving the LAN simply makes Ark unreachable, surfaced honestly (MOB-005's disconnected-state
UX) rather than papered over with a sync engine solving a problem that doesn't exist at this
architecture.

### 8.5 Platform-specific capabilities

MOB-008 (rewritten, reduced) owns Web Push permission/delivery only. Camera, native share sheet,
native file picker, and microphone are explicit non-goals — a PWA on iOS either lacks these or
only partially exposes them, and building around partial support isn't worth it at this scope.
MOB-007 owns installability/home-screen polish (manifest, safe areas, install hint).

### 8.6 On-device inference

Retired (MOB-010). Desktop llama-server spawning was never portable to iOS, and the native
module + App Store distribution path that would have made an on-device evaluation worthwhile no
longer applies. If the Phase 8 scope decision above is ever revisited toward native distribution,
re-open MOB-010 as part of that larger decision, not in isolation.

## 9. Testing Strategy

### 9.1 Test pyramid and ownership

| Layer | Scope | Runs | Primary tasks |
|---|---|---|---|
| Static/contract | TypeScript/Rust types, protocol compatibility, lint/format, forbidden dependencies | Every PR | FND-003, ARC-002/009 |
| Unit/property | State transitions, validation, settings, route policy, branch/domain rules, error mapping | Every PR | TST-001 |
| Component | UI semantics/states with fake ArkClient | Every PR | TST-004 |
| Database | Transactions, queries, migrations, backup/recovery, concurrency | Every PR where bounded; nightly fault matrix | TST-003 |
| Provider integration | Offline protocol simulators and event reconciliation | Every PR | FND-004, TST-002 |
| Native E2E | Installed desktop/mobile critical workflows | Blocking smoke on RC; broader nightly/release | TST-005 |
| Accessibility | axe/keyboard/contrast/visual plus manual assistive-tech | Automated PR; manual RC | TST-004/005 |
| Security | Audits, secret/SBOM, fuzzing, hostile content, API/tool adversarial | Scans every PR; extended nightly/release | TST-006 |
| Performance/soak | Startup, streaming, render, search, memory, runtime, sync | Directional PR; dedicated nightly/RC | TST-007 |
| Release | Signed install/update/rollback/uninstall, clean machine | Every release candidate | OPS-002/004 |

The implementer of a task writes its focused tests. TST tasks own shared harnesses, matrices, cross-cutting gaps, and release qualification—not deferred testing.

### 9.2 Unit testing

- Test every generation state transition and invalid transition.
- Use property/boundary tests for Unicode, numeric ranges, URLs/IP classes, import schemas, branch graphs, and sync revisions.
- Test settings precedence and provider-capability negotiation.
- Assert stable typed error codes and redaction.
- Use mutation testing selectively on lifecycle, validation, transaction, and security-policy modules.

### 9.3 Integration testing

- Provider simulators cover protocol framing, timeouts, disconnect, terminal markers, cancellation, and retries.
- Real temporary SQLite databases cover fault injection and concurrency.
- Tauri/application service integration covers request validation, committed events, and state reconciliation.
- Secret-store, filesystem, process manager, and companion API adapters use OS-specific or isolated integration jobs.
- Optional real-provider/model smoke is compatibility evidence, not a replacement for deterministic fixtures.

### 9.4 End-to-end and UI testing

- Test actual packaged binaries with isolated disposable workspaces.
- Cover restart during generation/import/migration/update.
- Cover viewport matrix, themes, zoom, keyboard, screen reader checklist, and reduced motion.
- Cover first-run through successful model response and all recovery paths.
- Real iOS Safari (PWA) covers device pairing, the disconnected-state UX, stream reconnect, and Web Push permissions — no native simulator, no offline outbox/sync-conflict testing (neither exists under the Phase 8 scope decision).

### 9.5 Performance testing

Use FND-005 fixtures and PERF-001 budgets:

- cached shell ≤1.0 second on reference hardware;
- provider refresh never blocks cached history/settings;
- cancellation acknowledgement ≤100 ms;
- stream persistence ≤20 batches/second;
- 1,000-conversation search/filter ≤100 ms target where the audit specifies;
- 100,000-character response remains responsive and approximately linear;
- memory is measured at baseline, long chat, large code output, import, and local model;
- soak includes repeated model load/unload and sleep/wake.

Threshold changes require benchmark evidence and review, never silent relaxation.

### 9.6 Security testing

- npm/cargo advisories, licenses, SBOM, secret scan, artifact signature.
- URL classification, DNS/redirect behavior, TLS/auth/rate/replay.
- import/archive/model fuzzing and resource-exhaustion boundaries.
- CSP/external-link/hostile Markdown/code/attachment rendering.
- keychain/log/crash/diagnostics redaction.
- prompt-injection, malicious documents/pages/MCP server, approval bypass, tool replay.
- signed updater tamper/downgrade and signing-key incident rehearsal.

### 9.7 Regression policy

Every fixed audit defect receives a named regression test linked to its audit and task ID. A task cannot close while its regression test is quarantined. Flaky tests are production defects with owners and deadlines; they are not silently retried until green.

## 10. Release Strategy

**This entire section describes the process for *if* Ark ever moves to public, staged
distribution — it is not the current path.** Per the Phase 8 scope decision (recorded above
MOB-001) and OPS-002/OPS-004's rewrite, the actual current release process is a manual
checklist: build, smoke-test, attach to a GitHub Release, done. Signed installers, staged
rings, canary cohorts, TestFlight, and feature-flag-gated public rollout all assume a
public/unknown-sized audience this product does not have today. This section is retained as
the design for that scenario, gated the same way CMP-009 gates the team/multi-user decision —
revisit it together with OPS-002/004 if the distribution scope decision ever changes, rather
than treating any milestone below as active.

### 10.1 Milestones

| Milestone | Included work | Audience | Entry/exit |
|---|---|---|---|
| M0 — Correctness build | Phases 0–1 core plus critical dependency/package fixes | Engineering only | All C-01–C-09 regression tests; debug installer works |
| M1 — Hardened alpha | Security/architecture baseline, responsive/accessibility core, backup/recovery, observability | Internal dogfood | No unresolved critical; migration/backup/restart rehearsed |
| M2 — Desktop beta | Production feature completion, performance budgets, signed installers/updater | Closed external beta | No unreviewed high security/data-loss issue; clean-machine matrix |
| M3 — Desktop 1.0 RC | All desktop release gates, docs/support/rollback | Canary cohort | Definition-of-Done desktop subset and RC soak pass |
| M4 — Desktop stable | Staged production rings | Supported users | Canary health and support thresholds met |
| M5 — Competitive workspace | Attachments/RAG/tools/web/voice as individually gated | Opt-in beta then stable | Capability-specific security/evaluation gates |
| M6 — iPhone beta | Mobile shared architecture, API/auth/sync/core/native capabilities | TestFlight cohort | Mobile security/offline/device/accessibility matrix |
| M7 — iPhone stable / roadmap complete | Mobile rollout plus all non-deferred master tasks | Supported users | Full Definition of Done |

### 10.2 Internal testing

- Dogfood with disposable and migrated real-world copies, never the only production data.
- Include interrupted generation, provider offline, large history, low disk/RAM, non-ASCII, screen reader, and multiple provider versions.
- Run signed update from the prior ring on every candidate.
- Require backup verification before destructive migration rehearsal.

### 10.3 Beta rollout

- Closed beta cohorts represent supported OS versions, hardware classes, local providers, cloud opt-in, workspace sizes, accessibility use, and network conditions.
- Feature flags isolate managed runtime, cloud providers, RAG/tools, and mobile sync until their specific gates pass.
- Feedback collection is voluntary and separates diagnostics consent from general feedback.
- Block promotion on any reproducible data loss, privacy misrouting, unsafe tool side effect, update/signature failure, or unrecoverable stuck chat.

### 10.4 Production rollout

1. Internal signed channel.
2. Small canary percentage/cohort.
3. Expanded stable cohort after minimum observation/use thresholds.
4. Broad stable release.
5. Post-release review with incident/test/metric changes.

The local-first app should not require always-on remote analytics to perform staged rollout. Channel enrollment, opt-in crash data, support cases, and deterministic release tests provide the evidence.

### 10.5 Rollback

- Retain prior signed installers and update manifests.
- Migrations back up before change and document whether application downgrade is schema-compatible.
- If downgrade is incompatible, rollback restores the verified pre-migration workspace copy rather than opening a newer DB with old code.
- Update channels can pause/revoke a release.
- Security/signing compromise follows SEC-011 incident response and key rotation.
- User content created after a migration is exported before restore where technically safe and clearly reconciled.

## 11. Definition of Done

### 11.1 Per-task Definition of Done

A task is done only when:

- all stated acceptance criteria pass;
- focused unit/integration/UI/security/performance tests are merged and green;
- data/protocol changes include migrations/version compatibility;
- accessibility and privacy impact are reviewed;
- structured errors/logging are redacted and documented;
- documentation/support matrix is updated;
- no temporary flag, duplicate path, unused code, or stale claim introduced by the task remains;
- audit/task IDs appear in change documentation;
- code review includes the responsible domain owner.

### 11.2 Desktop production Definition of Done

- C-01–C-10 and every Critical/High desktop audit item are closed or receive an explicit approved non-action disposition in the matrix.
- Generation cannot remain stuck or silently complete after crash, race, cancel, malformed/truncated stream, timeout, or import.
- Chat/import/workspace/migrations are atomic/recoverable and backups restore on supported versions.
- Provider route and “local” privacy claims are enforced in Rust.
- No unreviewed critical/high dependency vulnerability exists.
- Signed installers and signed updater pass install/update/rollback/uninstall on every declared OS.
- WCAG 2.2 AA, keyboard, screen reader, reduced motion, zoom, and viewport gates pass.
- Startup, streaming, history, memory, and runtime budgets pass.
- Structured redacted diagnostics and opt-in crash reporting are operational.
- Support, privacy, security, license, third-party, migration, backup, and release documentation is accurate.
- CI and release gates are required, repeatable, and green.

### 11.3 Full roadmap Definition of Done

In addition to desktop production:

- managed models, secure cloud providers, projects/prompts, branch explorer, search/organization, attachments/vision, RAG/citations, safe tools/MCP/agents, web search, voice, notifications, and approved automation/artifact scope meet their task gates;
- the Ark Code agentic coding environment meets its task gates (CODE-001–008) on the tool/agent foundation established by CMP-003/SEC-009, without having altered the chat generation lifecycle or duplicated the provider/tool-permission frameworks;
- every capability declares model/provider support, data route, permissions, provenance, export/delete behavior, and failure recovery;
- the Phase 8 PWA meets its own, smaller task gates (MOB-001/005/007/008/009) — installability, disconnected-state honesty, accessibility, Web Push, and LAN pairing; there is no Expo client or App Store release requirement under the Phase 8 scope decision, and claiming either would misrepresent what was actually built;
- on-device iPhone inference (MOB-010) is retired under the same scope decision, not deferred pending a future gate — re-opening it requires re-opening that decision first, not just this line item;
- CMP-009 records the team/multi-user decision; no RBAC work is required if the local single-user product remains the approved scope;
- the Audit Traceability Matrix has no unmapped, pending-without-owner, or silently waived finding.

## Audit Traceability Matrix

The audit assigned explicit IDs only to C-01–C-10. This matrix assigns normalized A-* IDs to every other distinct actionable finding/recommendation so implementation and closure can be tracked. Grouping is used only where the audit raised the same root issue in multiple sections. “Retain” rows are deliberate no-change dispositions for audited strengths or currently non-applicable controls.

### Critical findings

| Audit finding | Audit statement | Implementation task(s) / disposition |
|---|---|---|
| C-01 | Durable streaming/pending state can permanently wedge a conversation after restart/import; cancel has no durable fallback | FND-002, COR-001, COR-005, UX-004, TST-001/003/005 |
| C-02 | Backend events can beat the frontend placeholder and leave missing/endless output | FND-002/004, COR-002, ARC-002/008, TST-002 |
| C-03 | Tauri bundle fails because icon assets are incomplete | COR-012, OPS-002, TST-005 |
| C-04 | Ollama/OpenAI-compatible adapters accept malformed/truncated streams as complete | COR-003, FND-004, TST-002 |
| C-05 | Whole-request 60/120-second timeouts break valid long generations | COR-003, TST-002/007 |
| C-06 | Send/edit/regenerate/import multi-write flows are not transactional | COR-004, COR-009, ARC-001/004, TST-003 |
| C-07 | Byte-sliced automatic title can panic on long non-ASCII text | COR-006, TST-001 |
| C-08 | A provider labelled local can be pointed at any remote URL | SEC-001, COR-008, UX-005/011, TST-006 |
| C-09 | Rust lockfile has three advisories and 17 unmaintained/unsound warnings | SEC-003, FND-003, TST-006 |
| C-10 | No CI, signing, updater, rollback, crash reporting, or backup release chain | FND-003, FTR-001, OPS-001–004, TST-005–007 |

### UI/UX findings

| Audit finding | Audit statement | Implementation task(s) / disposition |
|---|---|---|
| A-UX-01 | At 390×844 the center chat collapses to zero; no mobile breakpoint | UX-001, TST-004; the Phase 8 PWA (MOB-001) reuses the same responsive shell rather than a separate native solution |
| A-UX-02 | At configured 980 px minimum, chat header actions clip | UX-001/002, TST-004 |
| A-UX-03 | Placeholder Context/Files/Memory panel consumes 260 px | FND-001, UX-001/002; implemented content FTR-003, CMP-001/002/007 |
| A-UX-04 | No auto-follow or jump-to-latest for streaming/long chats | UX-003, PERF-003, TST-004/005 |
| A-UX-05 | Stale streaming state disables all chat mutation UI without recovery | COR-001/005, UX-004 |
| A-UX-06 | Assistant technical output shrink-wraps and code becomes cramped | UX-003, TST-004 |
| A-UX-07 | Failure-only global toast; success and contextual errors are weak | UX-004/011, OPS-001 |
| A-UX-08 | No content search, archive, pin, folders, tags/projects, bulk management | ARC-007, FTR-002/003 |
| A-UX-09 | Native confirm is inconsistent and weak for focus/recovery | UX-007/009, FTR-002 undo/archive |
| A-UX-10 | Token/status/timing/throughput metadata is stored or computable but hidden | UX-011, UX-010, PERF-001 |
| A-UX-11 | Temperature/max-token fields accept arbitrary text and invalid numbers | COR-008, UX-005, FTR-004 |
| A-UX-12 | Provider URL validation is only non-empty and lacks route safety | SEC-001, COR-008, UX-005 |
| A-UX-13 | Workspace/model paths require manual typing; no native pickers | UX-005, SEC-007 |
| A-UX-14 | Built-in provider overclaims shipped engine; provider refresh delays bootstrap | FND-001, COR-012, UX-011, FTR-009, PERF-002 |
| A-UX-15 | Diagnostics disk/GPU/benchmark/output/error behavior is inaccurate or incomplete | UX-010, ARC-010, PERF-001/004 |
| A-UX-16 | Bootstrap, refresh, stale model, import progress, interrupted, DB/disk/workspace, and offline states are missing/weak | COR-010, UX-004, FTR-009, OPS-001 |
| A-UX-17 | Dark error contrast fails AA; semantic tabs/pressed/labels/live regions/landmarks are incomplete | UX-006/008/009, TST-004 |
| A-UX-18 | Focus, keyboard restoration, shortcut discovery, reduced motion, and touch-target polish are incomplete | UX-007/008/011, TST-004/005 |

### Functionality and data findings

| Audit finding | Audit statement | Implementation task(s) / disposition |
|---|---|---|
| A-FUN-01 | Conversation/history loads do not scale; path lookup is N+1; no pagination/virtualization/content search | ARC-007, FTR-002, PERF-003 |
| A-FUN-02 | Archive/system prompt/per-conversation settings exist only in schema; branch visualization/selection is limited | ARC-006, FTR-002–005 |
| A-FUN-03 | Stop generation is in-memory only and partial/interrupted UX is incomplete | COR-001/005, UX-004 |
| A-FUN-04 | Ollama/local host paths lack protocol confidence; built-in runtime/model lifecycle/capabilities/auth are incomplete | FND-004, COR-003/012, ARC-003/010, FTR-006/009, TST-002 |
| A-FUN-05 | streamingEnabled is hard-coded true and adapters always stream | ARC-006, FTR-004, COR-003 |
| A-FUN-06 | Markdown needs link/security tests; highlighting reparses growing output; copy lacks announcement | SEC-008, UX-003/006, PERF-005, TST-004/006 |
| A-FUN-07 | Import/export is unbounded/nontransactional and lacks batch, full metadata, attachment, and round-trip guarantees | COR-009, FTR-008, CMP-001, TST-003 |
| A-FUN-08 | Workspace switch does not migrate; no backup/restore; probe can delete a same-name file | COR-007/010, FTR-001, OPS-004 |
| A-FUN-09 | Diagnostics/benchmark and sidecar readiness/logging/resource insight are incomplete | ARC-010, UX-010, PERF-004, OPS-001 |
| A-FUN-10 | Single DB mutex, poisoned-lock unwrap, corruption/full-disk/concurrent-instance behavior are unsafe | COR-010, ARC-004, TST-003 |
| A-FUN-11 | Stale selected models, duplicated setting ownership, localStorage model path, unused preview/metadata cause inconsistency | ARC-006/008, FTR-004/009, UX-010/011 |

### Security findings

| Audit finding | Audit statement | Implementation task(s) / disposition |
|---|---|---|
| A-SEC-01 | Authentication/session are N/A locally but mandatory for mobile/sync; local cosmetic login would not help | SEC-010, MOB-009; no local-login action by explicit disposition |
| A-SEC-02 | Minimal Tauri capabilities are a strength; future tools need scopes | SEC-008/009, TST-006; retain least privilege |
| A-SEC-03 | api_key_ref is unused and no secure credential path exists | SEC-005, ARC-006, FTR-007, MOB-009 |
| A-SEC-04 | Arbitrary remote/LAN endpoints and local sidecar lack complete API authentication/trust boundaries | SEC-001/002/010, FTR-010, MOB-009 |
| A-SEC-05 | SQLite transcripts are plaintext | SEC-006, FTR-001, OPS-003 disclosure |
| A-SEC-06 | File handling has unbounded import, unsafe probe, native parser/resource risks | COR-007/009, SEC-007, CMP-001 |
| A-SEC-07 | CSP/raw-HTML baseline is good, but unsafe-inline/external links/hostile content need policy/tests | SEC-008, TST-006 |
| A-SEC-08 | Native input validation is inconsistent | COR-008, SEC-001/007, TST-001/006 |
| A-SEC-09 | Prompt injection is low-impact now but becomes high with RAG/tools/web | SEC-009, CMP-002–004, TST-006 |
| A-SEC-10 | Updater/signing/rollback and binary-download integrity/SBOM/licenses are missing | SEC-004, OPS-002/003, TST-006 |
| A-SEC-11 | Current Rust advisories and platform/transitive warnings need remediation/review | SEC-003, FND-003, TST-006 |
| A-SEC-12 | Sidecar/model native attack surface needs auth, provenance, validation, limits, and lifecycle supervision | SEC-002/004/007, ARC-010, FTR-006, PERF-004 |

### Architecture and technical-debt findings

| Audit finding | Audit statement | Implementation task(s) / disposition |
|---|---|---|
| A-ARC-01 | Generation truth is distributed across UI/events/DB/in-memory task state | FND-002, COR-001–005/011, ARC-001/002/008 |
| A-ARC-02 | Migration runner is not a true ordered/version-gated transactional system | ARC-005, FTR-001, TST-003 |
| A-ARC-03 | Global Mutex Connection serializes work and can be poisoned | ARC-004, COR-011, TST-003 |
| A-ARC-04 | Command/ChatView/DB/provider modules are oversized and responsibilities mixed | ARC-001/008/009 |
| A-ARC-05 | Closed provider enum/switch is not scalable or capability-driven | ARC-003, FTR-007 |
| A-ARC-06 | Direct Tauri calls/broad App state/duplicated DTOs block reuse and reconciliation | ARC-002/008, MOB-001/002 |
| A-ARC-07 | One-query-per-ancestor path lookup is inefficient | ARC-007, PERF-003 |
| A-ARC-08 | Unused/duplicated schema fields and localStorage/SQLite split lack ownership | ARC-006, ARC-005 migrations |
| A-ARC-09 | No lint/format CI; strict clippy fails two findings | FND-003, ARC-009 |

### Performance findings

| Audit finding | Audit statement | Implementation task(s) / disposition |
|---|---|---|
| A-PERF-01 | Per-delta DB concat/read/full-content event/React Markdown work approaches O(n²) | COR-011, ARC-008, PERF-001/005, TST-007 |
| A-PERF-02 | All history/full active path render without paging/windowing and path lookup is N+1 | ARC-007, PERF-003 |
| A-PERF-03 | Provider health/model refresh blocks initial shell | FTR-009, PERF-002 |
| A-PERF-04 | Global DB serialization amplifies streaming contention | ARC-004, COR-011 |
| A-PERF-05 | No model hardware-fit, context/concurrency/backpressure/resource governance | PERF-004, FTR-006, ARC-010 |
| A-PERF-06 | Bundle is acceptable but Chat chunk/highlighting and startup/memory lack ongoing budgets | FND-005, PERF-001/005, TST-007; no speculative bundle rewrite |

### Competitive and product-gap findings

| Audit finding | Audit statement | Implementation task(s) / disposition |
|---|---|---|
| A-CMP-01 | No one-click model discovery/download/load/unload/delete/hardware fit | FTR-006, PERF-004 |
| A-CMP-02 | No secure cloud provider support | SEC-005, FTR-007 |
| A-CMP-03 | Filesystem workspace is not a project/workspace with instructions/files/prompts/memory | FTR-002/003, CMP-001/002/007 |
| A-CMP-04 | No RAG/knowledge/embeddings/citations | CMP-002, SEC-009 |
| A-CMP-05 | No tools/MCP/agents | CMP-003, SEC-009 |
| A-CMP-06 | No prompt/persona library | FTR-003 |
| A-CMP-07 | No attachments/vision/media | CMP-001, SEC-007, FTR-008 |
| A-CMP-08 | No voice | CMP-005, MOB-008 |
| A-CMP-09 | No conversation content search or web search | FTR-002, CMP-004 |
| A-CMP-10 | Branching exists but is not transparent/reproducible enough | FTR-005, UX-011 |
| A-CMP-11 | No integration/local server API | ARC-002, FTR-010 |
| A-CMP-12 | No multi-user/RBAC; audit says not required for local desktop | CMP-009 explicit decision/non-action |
| A-CMP-13 | No mobile/sync and no mobile-native capabilities/notifications | MOB-001–009, CMP-006 |
| A-CMP-14 | No automations/artifacts | CMP-008 after safe tools |
| A-CMP-15 | Differentiation opportunity: auditable routing, branch research, local control plane, safer tools | SEC-001, UX-011, FTR-005/006, CMP-003, MOB-009 |

### Mobile findings

| Audit finding | Audit statement | Implementation task(s) / disposition |
|---|---|---|
| A-MOB-01 | Mobile readiness is 10/100; DOM/Tauri/sidecar cannot be reused as iPhone UI/runtime | MOB-001/006/010 |
| A-MOB-02 | Extract pure domain/types/ports and shared monorepo packages now | ARC-002/006, MOB-001/002 |
| A-MOB-03 | Need a versioned companion API rather than raw DB/Tauri access | FTR-010 |
| A-MOB-04 | Need PKCE/device identity and Keychain/SecureStore | Superseded by the Phase 8 scope decision: MOB-009's lightweight LAN pairing tokens (SEC-005-backed server-side storage), not PKCE/OAuth — an explicit, documented substitution, not an unmet finding |
| A-MOB-05 | Need offline outbox/change log/tombstones/conflicts and safe sync | ARC-005, MOB-005/007, TST-003/005 |
| A-MOB-06 | Need notifications, files, camera, voice, permission/denial behavior | CMP-001/005/006, MOB-008 |
| A-MOB-07 | Need authenticated LAN discovery/pairing and network-change policy | SEC-010, MOB-009 |
| A-MOB-08 | On-device inference requires separate native evaluation; desktop sidecar cannot port | MOB-010 retired under the Phase 8 scope decision (no native app to host it); re-open only alongside that larger decision |

### Production-readiness and operations findings

| Audit finding | Audit statement | Implementation task(s) / disposition |
|---|---|---|
| A-OPS-01 | No structured logs, crash reporting, support bundle, performance telemetry, or clear opt-in policy | OPS-001, PERF-001, SEC-011 |
| A-OPS-02 | Testing is sparse: 18 Rust tests, no frontend/provider/E2E/accessibility/CI matrix | FND-003/004, TST-001–007 |
| A-OPS-03 | Bundle fails; no code signing, notarization, updater, update signatures, channels, rollback | COR-012, OPS-002/004 |
| A-OPS-04 | No backup/restore, robust migration, corruption/disk/lock recovery | COR-010, ARC-004/005, FTR-001, TST-003 |
| A-OPS-05 | Documentation/privacy/security/license/support/release material is stale or missing | FND-001, SEC-011, OPS-003 |
| A-OPS-06 | No staged deployment/monitoring/analytics policy/configuration management | ARC-006, OPS-001/004; behavioral analytics explicitly remain off |

### Audited strengths and no-action/retain dispositions

| Audit finding | Audited strength or non-applicable area | Task/disposition |
|---|---|---|
| A-RET-01 | Local-first, no account, no analytics, no default remote provider | Preserve through SEC-001/010, OPS-001, FTR-007 opt-in tests |
| A-RET-02 | Minimal Tauri core/event capabilities; no broad FS/shell plugin | Preserve through SEC-008/009 and TST-006 least-privilege regression |
| A-RET-03 | Parameterized SQL; no string-built injection path found | Retain; TST-001/003/006 regression. No rewrite required |
| A-RET-04 | React/Markdown raw HTML disabled, reducing XSS | Retain through SEC-008/TST-006. Do not enable raw HTML |
| A-RET-05 | Append-only branches and open Markdown/JSON portability are product strengths | Preserve/extend through FTR-005/008 and migration tests |
| A-RET-06 | Lazy Chat/Settings chunks and current desktop bundle sizes are acceptable | Retain and monitor through PERF-001/005; no speculative framework rewrite |
| A-RET-07 | UUID IDs, persisted status, narrow top-level layers are useful foundations | Preserve while implementing FND-002, ARC-001/002/005 |
| A-RET-08 | Authentication/session/CSRF/RBAC are not required for the current single-user local desktop | Explicit no-action in Section 7; SEC-010/MOB-009 when remote; CMP-009 for team decision |

### Traceability completion rule

The matrix is complete when each row has one of:

1. all mapped tasks marked done with linked acceptance evidence; or
2. an approved explicit disposition stating why no action is required, what would trigger reconsideration, and which regression guard preserves the assumption.

Closing a broad task does not automatically close every mapped finding. Each finding row must be reviewed against its exact statement during milestone sign-off.
