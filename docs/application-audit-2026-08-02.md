# Ark Comprehensive Application Audit

**Audit date:** 2 August 2026  
**Repository:** Ark 0.1.0  
**Scope:** Entire checked-out application: React/Tauri UI, Rust core, SQLite persistence, provider integrations, built-in inference scaffold, packaging, tests, documentation, security posture, competitor position, and iPhone readiness.

## Audit method and confidence

This audit used four evidence levels:

1. **Executed:** production frontend build, TypeScript checks, Rust tests, strict Rust linting, npm and Rust dependency advisory scans, and a Tauri debug bundle attempt.
2. **Rendered:** the production React UI was exercised in the in-app browser with a temporary Tauri-command harness at desktop, minimum-window, and iPhone-sized viewports. The harness used mock data; it validates layout and interaction presentation, not native IPC or real model behavior.
3. **Inspected:** all first-party source modules, database schema/migrations, configuration, packaging assets, setup scripts, product plans, and documentation were reviewed.
4. **Researched:** competitive and mobile recommendations use current official product and platform documentation linked in this report.

The following were **not** available for end-to-end validation: an installed Ollama model, a bundled llama-server binary and GGUF model, signed installers, macOS/Linux runners, an iPhone build, or production telemetry. Runtime assertions about those paths are therefore based on code inspection and are explicitly identified as risks rather than measured outcomes.

## 1. Executive summary

Ark has a sound product nucleus: a privacy-oriented Tauri shell, local SQLite history, Ollama and OpenAI-compatible local endpoints, streamed chat, append-only branching, portable workspaces, diagnostics, and conversation import/export. Its UI is visually coherent at a normal desktop width, its webview permissions and CSP are comparatively narrow, SQL is parameterized, Markdown raw HTML is not enabled, and the frontend is already split into lazy-loaded chat and settings chunks.

It is nevertheless **not ready for production distribution**. The most serious blockers are not missing “nice-to-have” features; they are correctness and delivery failures:

- A restart or crafted import can leave a conversation permanently stuck in a streaming state.
- Backend stream events can race the frontend's optimistic placeholder, potentially leaving an apparently endless response.
- Long generations are subject to whole-request timeouts, and truncated provider streams can be recorded as complete.
- Multi-write chat and import operations are not transactional.
- A long non-ASCII first prompt can panic the title-generation path.
- A provider still labelled “local” can be pointed at any URL, silently invalidating the privacy promise.
- The Rust lockfile currently has three security advisories; two are high-severity XML denial-of-service advisories.
- The installer build fails because the Windows icon is not a valid complete bundle icon set.
- There is no CI, signing, updater, release validation, crash reporting, or backup/restore workflow.
- At the configured 980 px minimum width, header actions clip. At 390 px, the chat column collapses to zero width.

The recommended strategy is to freeze major feature expansion for one hardening milestone, make the existing chat path recoverable and testable, establish a real release pipeline, then add model management and knowledge/agent capabilities in layers. For iPhone, build a companion client in Expo/React Native around shared TypeScript contracts and a versioned service boundary; do not attempt to reuse the desktop DOM or Tauri sidecar.

**Overall application health: 42/100.**  
**Production readiness: 24/100.**  
**iPhone readiness: 10/100.**

## 2. Overall health score

Scores use a weighted engineering rubric. They indicate relative risk and are not a claim of mathematical precision.

| Area | Weight | Score | Weighted result | Rationale |
|---|---:|---:|---:|---|
| Core functionality | 20% | 55 | 11.0 | Broad MVP exists; stream recovery, event ordering, and transactional integrity are unsafe. |
| UI/UX and accessibility | 15% | 46 | 6.9 | Coherent desktop styling; minimum-width clipping, no mobile layout, limited state/focus feedback. |
| Security and privacy | 15% | 52 | 7.8 | Narrow Tauri surface and safe rendering; dependency advisories, local/remote ambiguity, unencrypted data, unsafe binary supply chain. |
| Architecture and maintainability | 15% | 50 | 7.5 | Clear top-level layers; oversized command/UI modules, global DB mutex, weak migration and service boundaries. |
| Reliability and testing | 15% | 25 | 3.8 | Eighteen Rust tests pass, but critical streaming/UI/integration paths are untested and there is no CI. |
| Performance | 10% | 40 | 4.0 | Reasonable bundles and lazy loading; O(n²) streaming work, full-history rendering, synchronous bootstrap dependencies. |
| Delivery and operations | 10% | 10 | 1.0 | Native bundle currently fails; no signing, updater, release pipeline, monitoring, or recovery process. |
| **Overall** | **100%** |  | **42/100** | Promising MVP; substantial release risk. |

## 3. Strengths

### Product and UX

- The local-first positioning is understandable and the basic three-pane desktop information architecture is familiar.
- Conversation branching is more sophisticated than a typical early MVP: edits and regenerations are append-only, assistant alternatives can be switched, and the active path is reconstructed.
- Markdown, GitHub-flavored tables/lists, fenced code highlighting, and code copy are implemented.
- Conversation export supports both readable Markdown and structured JSON; JSON import performs schema-level validation.
- Portable workspace selection, diagnostics, provider setup guidance, and model availability checks address real local-AI friction.
- Light and dark palettes are visually consistent. Measured normal-text contrast passed WCAG AA in the light theme.

### Engineering and security

- Tauri substantially narrows the installed-app footprint compared with an Electron bundle.
- The capability configuration grants only core and event defaults; no broad filesystem or shell plugin capability is exposed to the webview.
- The CSP limits scripts to the application and provider connectivity to loopback HTTP endpoints. See [Tauri configuration](../src-tauri/tauri.conf.json).
- Database calls use parameterized SQL. No string-built SQL injection path was found.
- React Markdown does not enable raw HTML, materially reducing XSS exposure from model output and imported transcripts.
- Sidecar execution passes arguments as an argument list rather than constructing a shell command.
- UUID identifiers and an explicit persisted message status form a reasonable base for sync and recovery work.
- The production frontend already lazy-loads Chat and Settings.
- The repository includes focused diagnostics guidance tests, database tests, export validation tests, and provider construction tests.

### Privacy

- There is no account requirement, cloud synchronization, analytics SDK, advertising SDK, or default remote provider.
- Conversations live in a user-selectable SQLite workspace and can be exported without a proprietary server.
- The architecture is capable of fully local inference when an external local provider is actually available.

## 4. Weaknesses

- Reliability semantics are incomplete: “streaming” is treated as both durable state and live-task truth, without recovery after process loss.
- UI optimistic state and backend events have no ordering contract.
- The built-in-runtime message overstates reality: setup code exists, but the repository contains no runnable llama-server binary and packaging cannot currently complete.
- Documentation disagrees with implementation about the built-in runtime and cloud-provider status.
- Input validation is mainly “non-empty,” despite numeric and URL inputs controlling provider requests.
- Core modules have grown into orchestration monoliths: ChatView is approximately 867 lines, the command module approximately 1,100 lines, the database module approximately 917 lines, and the provider module approximately 644 lines.
- There is no frontend test suite, provider protocol test server, native end-to-end suite, accessibility automation, or release smoke test.
- The app has no responsive navigation model. Collapsing panels is manual rather than breakpoint-driven.
- Operational fundamentals—CI, signing, updater, logs, crash reports, backups, rollback, release channels—are absent.
- Strategic features such as managed models, secure cloud keys, attachments, RAG, prompt/workspace organization, tools/agents, voice, and mobile sync do not exist.

## 5. Critical issues

Severity definitions:

- **Critical:** release blocker or credible path to data loss, indefinite loss of core function, or privacy/security breach.
- **High:** major user-visible failure or security/reliability defect with no good workaround.
- **Medium:** meaningful usability, maintainability, or performance defect.
- **Low:** polish or contained technical debt.

| ID | Severity | Finding and evidence | Impact | Required remediation |
|---|---|---|---|---|
| C-01 | Critical | **Durable “streaming” rows can wedge a conversation.** ChatView treats any active assistant status as a live stream and disables the composer ([ChatView.tsx:80](../src/features/chat/ChatView.tsx#L80), [ChatView.tsx:575](../src/features/chat/ChatView.tsx#L575)). Cancellation only signals the in-memory active-stream map; after a restart no task exists. Imports also accept transient statuses. | A crash, force-quit, backend panic, or crafted import can make the conversation permanently unusable. | On startup, atomically transition stale pending/streaming rows to interrupted. Normalize imported transient statuses. Make cancel fall back to a database transition and reconciliation event. Add restart/import recovery tests. |
| C-02 | Critical | **Stream-event race.** Commands emit stream-start and spawn generation before returning. The UI installs its placeholder only after the invoke resolves, and App does not subscribe to stream-start ([App.tsx:109](../src/App.tsx#L109)). | Fast delta/complete events can arrive before the target message exists in React state, leaving missing text or an endless placeholder. | Split row creation from stream start, or return IDs before task launch. Buffer/reconcile events by message ID, listen to stream-start, and refetch authoritative state after terminal events. Test immediate-completion and out-of-order delivery. |
| C-03 | Critical | **Installer cannot be built.** The Tauri debug bundle reached packaging and failed with “Couldn't find a .ico icon.” The repository has only a 766-byte, single-layer 32×32 icon. | No distributable Windows artifact; release process cannot be exercised. | Generate the complete Tauri icon set (multi-resolution ICO plus PNG/ICNS), add a clean-machine bundle smoke test, and validate install/uninstall before release. Follow [Tauri icon guidance](https://v2.tauri.app/develop/icons/). |
| C-04 | High | **Stream integrity is not guaranteed.** Ollama treats EOF as success even without done; the OpenAI-compatible adapter ignores malformed JSON events and accepts EOF without [DONE] ([providers/mod.rs:281](../src-tauri/src/providers/mod.rs#L281), [providers/mod.rs:463](../src-tauri/src/providers/mod.rs#L463)). | Truncated or corrupt responses can be silently persisted as complete. | Use an explicit stream state machine. Require a terminal marker or finish reason, fail invalid payloads, preserve an interrupted partial response, and test arbitrary network chunk boundaries. |
| C-05 | High | **Whole-stream timeouts are 60 and 120 seconds** ([providers/mod.rs:139](../src-tauri/src/providers/mod.rs#L139), [providers/mod.rs:309](../src-tauri/src/providers/mod.rs#L309)). | Legitimate long local generations fail despite continuing to make progress. | Use short connect/header timeouts and a resettable idle timeout, not a total generation timeout. Expose an explicit user cancellation/deadline policy. |
| C-06 | High | **Chat and import writes are not transactional.** Send/edit/regenerate insert multiple related rows and change branch state across separate operations; import writes a conversation then messages incrementally. | Disk/validation/provider failures can leave half-applied user actions or partial imports. | Wrap each logical mutation in one transaction. Commit durable request state before asynchronous work; use compensating terminal status on launch failure. Add fault-injection tests. |
| C-07 | High | **Unicode title generation can panic.** The database checks byte length, then slices the first 61 bytes of the prompt ([db/mod.rs:433](../src-tauri/src/db/mod.rs#L433)). | A sufficiently long non-ASCII first token can panic after the user row has already been written. | Truncate on Unicode scalar or grapheme boundaries and test emoji, combining marks, CJK, and no-whitespace strings. |
| C-08 | High | **The “local” privacy label is not enforced.** Seeded providers remain is_local even when their editable base URL is changed to an arbitrary host; database validation only requires a non-empty URL ([db/mod.rs:486](../src-tauri/src/db/mod.rs#L486)). ChatView still shows “local” ([ChatView.tsx:448](../src/features/chat/ChatView.tsx#L448)). | Users can unknowingly send complete chat history to a remote endpoint under an inaccurate privacy indicator. Rust HTTP is not constrained by the webview CSP. | Parse and validate URLs in Rust. Enforce loopback/private hosts for local providers, or reclassify remote hosts with a blocking disclosure and TLS/auth requirements. Derive the badge from the validated destination, not a seed flag. |
| C-09 | High | **Current Rust dependency advisories.** cargo-audit found crossbeam-epoch 0.9.18 (RUSTSEC-2026-0204) and quick-xml 0.39.4 (RUSTSEC-2026-0194 and -0195; both CVSS 7.5), plus 17 allowed unmaintained/unsound warnings. quick-xml arrives through plist/Tauri; crossbeam-epoch through sysinfo/rayon. | Known memory-safety and denial-of-service risks remain in the release graph. Some affected code is build/platform-specific, but that must be demonstrated, not assumed. | Upgrade to a Tauri/plist graph using quick-xml ≥0.41 and crossbeam-epoch ≥0.9.20; re-run tests and audit for all targets. Track or explicitly deny/allow warnings with rationale. Add cargo-audit to CI. See [RustSec](https://rustsec.org/). |
| C-10 | High | **No release trust or recovery chain.** There is no CI, code signing, notarization, updater, signed update manifest, rollback channel, crash reporting, or backup restore test. | Users cannot verify provenance, receive secure fixes, or recover reliably; maintainers cannot gate regressions. | Establish CI matrices, signed installers, Tauri updater signatures, staged channels, release smoke tests, local redacted logs, opt-in crash reporting, and documented rollback. See [Tauri distribution](https://v2.tauri.app/distribute/) and [Windows signing](https://v2.tauri.app/distribute/sign/windows/). |

## 6. UI/UX findings

### 6.1 Responsive layout and information architecture

**Critical — mobile layout is unusable.** At 390×844, the 288 px left sidebar and 260 px right panel remain in the horizontal layout, leaving the center chat section at zero width. The root hides overflow, so the primary task disappears rather than scrolling. There are no responsive breakpoints.

**High — the configured desktop minimum is smaller than the UI's functional minimum.** At 980×720, the expanded side panels leave about 432 px for chat. The header content measured 517 px wide, clipping export/import/delete controls. This contradicts the manual acceptance criterion that narrow windows should not overlap.

**Medium — the right panel consumes permanent space for placeholder content.** Context, files, and memory are not implemented, yet the empty panel occupies 260 px unless manually collapsed.

**Recommended shell:**

~~~text
Desktop ≥ 1200 px
┌──────────────┬───────────────────────────────────────┬──────────────┐
│ Conversations│ Chat: title · provider · model · ••• │ Context      │
│ search/list  │                                       │ when useful  │
│              │ messages                              │              │
│              │                         composer       │              │
└──────────────┴───────────────────────────────────────┴──────────────┘

Compact desktop/tablet
┌──────┬─────────────────────────────────────────────────────────────┐
│ rail │ Chat header · model · •••                                  │
│      │                                              context drawer │
└──────┴─────────────────────────────────────────────────────────────┘

iPhone
┌────────────────────────────────────┐
│ ☰  Conversation title   model  ••• │
│                                    │
│ full-width message stream          │
│                                    │
│ attachment  message…       send    │
└────────────────────────────────────┘
Conversations, model choice, and context open as sheets.
~~~

Implementation: auto-collapse the context panel below roughly 1200 px, convert the conversation sidebar to a rail/drawer below roughly 900 px, move secondary header actions into an overflow menu, and use a single-column mobile shell. Treat the current 980 px minimum as a defect, not a substitute for responsive behavior.

### 6.2 Conversation experience

| Severity | Finding | Why it matters | Better implementation |
|---|---|---|---|
| High | No automatic follow-to-bottom or “jump to latest” behavior was found. | Streaming and long conversations can continue below the viewport without feedback. | Follow output only while the user is near the bottom; preserve reading position when they scroll up; show a “New response ↓” control. |
| High | Stale streaming state disables send/edit/regenerate and cannot be recovered in the UI. | The primary workflow can become permanently blocked. | Display “Interrupted by restart” with Retry, Keep partial, and Discard actions. |
| Medium | Assistant messages shrink to content width; a rendered code response occupied roughly 220 px at 1280 px. | Code and tables become cramped and horizontally noisy. | Give assistant content the full readable column width; constrain user bubbles, not assistant technical output. |
| Medium | The UI uses one failure toast with no success feedback and no durable error center. | Import/export/settings actions are hard to confirm; errors disappear from workflow context. | Use contextual inline errors, success toasts for file actions, and a dismissible diagnostics link for provider failures. |
| Medium | No conversation content search, archive, pin, folders, tags, or bulk management. | The sidebar degrades as history grows. | Add indexed content search, archive/pin, folders/projects, and keyboard-accessible bulk selection after reliability work. |
| Medium | Native confirm dialogs are used for destructive actions. | They are visually inconsistent and provide limited contextual recovery. | Use an accessible application dialog with item name, consequences, focus trap/restoration, and optional undo for archive/delete. |
| Low | No token count, elapsed time, throughput, or interrupted-partial indicator is shown even though token/status fields exist. | Local-model users need performance and state transparency. | Show compact per-response metadata with an opt-out setting. |

### 6.3 Forms and onboarding

- **High:** temperature and max-token inputs accept arbitrary text. The frontend parses values, while the backend does not enforce finite/range constraints. Negative, NaN, or extreme values can reach providers.
- **High:** provider base URL validation is only non-empty; invalid schemes, credentials-in-URL, remote hosts, and malformed URLs are not rejected.
- **Medium:** built-in model and workspace paths must be typed manually. Native file/folder pickers should be the primary control, with the path visible as secondary information.
- **Medium:** the built-in provider says Ark “ships a built-in inference engine,” but the repository has no engine binary. Onboarding should never promise a capability the installed artifact cannot deliver.
- **Medium:** provider health and setup guidance are useful, but bootstrap can wait several seconds for provider checks before the main shell is considered ready. Paint persisted conversations first, then refresh providers in the background.
- **Medium:** benchmark failures are swallowed, output preview is computed but not presented, token/s is a whitespace approximation, and disk capacity sums all mounted disks rather than the workspace volume. Label estimates and display actionable failure reasons.

Server-side validation should be authoritative:

~~~text
base URL: http/https only; explicit loopback/private/remote classification
temperature: finite and within provider-supported range
max tokens: positive integer with configured upper bound
import: maximum bytes, conversations, messages, depth, and content length
model file: exists, regular file, readable, plausible GGUF header and size
workspace: existing or explicitly creatable directory; randomized write probe
~~~

### 6.4 Loading, empty, and error states

**Implemented well enough for MVP:** initial loading label, empty conversation prompt, provider setup banner, model-unavailable guidance, streaming spinner, stop action, code-copy feedback.

**Missing or weak:**

- Bootstrap failure has no full-page recovery state or retry button.
- Conversation-list and model-refresh loading are not represented with scoped skeleton/status feedback.
- A provider with zero models is handled as setup guidance, but stale selected models are not clearly marked unavailable in Settings.
- Import and export do not show progress or file-size limits.
- Interrupted generation, partial import, database lock/corruption, insufficient disk space, and workspace migration failures have no designed states.
- There is no offline/LAN-disconnected state model beyond provider “unreachable.”

### 6.5 Accessibility

Measured light-theme body, muted, primary, secondary, and destructive text passed WCAG AA for normal text. Dark-theme destructive/error text measured approximately 4.02:1 and fails the 4.5:1 normal-text threshold.

Additional findings:

- **High:** no responsive mobile reading order because the main region collapses.
- **Medium:** stream changes, errors, loading, and copy completion lack reliable role=status, role=alert, or aria-live announcements.
- **Medium:** provider controls visually behave as tabs but do not expose tab semantics or selected state.
- **Medium:** theme choice buttons do not expose aria-pressed.
- **Medium:** workspace and inline rename inputs do not have robust explicit accessible labels.
- **Medium:** there is no prefers-reduced-motion path despite animated panel and message transitions.
- **Medium:** focus is not intentionally moved to the composer after creating/selecting a conversation, nor restored after dialogs/actions.
- **Low:** 32 px icon buttons exceed WCAG 2.2's 24 px minimum target but remain below the preferred 44 px touch target.
- **Low:** landmark semantics and labelled navigation regions can be strengthened.

Remediation: introduce semantic main/nav/aside regions, real tabs, explicit labels and state attributes, live regions for stream/status messages, focus tests, a reduced-motion CSS/Framer policy, and automated axe plus keyboard-only E2E coverage. Use [WCAG 2.2](https://www.w3.org/TR/WCAG22/) as the release baseline.

### 6.6 Overall polish

The visual system is more mature than the reliability system: spacing, typography, color, panel surfaces, focus rings, badges, and dark/light themes form a coherent base. The main polish deficit is state design—recovery, progress, success, offline, and responsive behavior—not ornamental styling. Do not spend the next milestone on animations or theme customization before fixing these states.

## 7. Functionality findings and implemented-feature inventory

Status meanings: **Complete** means the intended source path is present, not that it is production-hardened; **Partial** means a visible or architectural slice exists but important behavior is missing; **Broken** means the current release path cannot satisfy its claim.

### 7.1 Desktop shell and appearance

| Feature | Status | Quality | Dependencies | Known limitations and improvement |
|---|---|---|---|---|
| Tauri desktop window | Partial | Fair | Tauri 2, webview | Runs as a native shell, but bundle fails and platform matrix is untested. Fix assets and release pipeline. |
| Three-pane layout | Partial | Fair at 1280 px | React, Tailwind-style CSS | Manual collapse only; clips at 980 px and fails mobile. Implement responsive drawers/rail. |
| Persisted panel collapse | Complete | Good | localStorage | Device-local only; acceptable. Add breakpoint overrides without destroying user preference. |
| Light/dark/system theme | Complete | Good | localStorage, settings DB | Dark destructive contrast fails AA. Add reduced-motion and high-contrast review. |
| Keyboard shortcuts | Partial | Fair | Browser key events | New, focus/search, command hint, send are present. No discoverable shortcut reference, conflict handling, or comprehensive navigation. |

### 7.2 Conversations and messages

| Feature | Status | Quality | Dependencies | Known limitations and improvement |
|---|---|---|---|---|
| Create/list/select conversations | Complete | Fair | SQLite, Tauri commands | Loads all conversations; no pagination/virtualization. |
| Automatic first-prompt title | Broken on Unicode edge case | Poor | SQLite helper | Byte slicing can panic. Use grapheme-safe truncation. |
| Rename/delete | Complete | Fair | SQLite, native confirm | Add accessible dialog and undo/archive path. |
| Title search | Complete | Fair | In-memory frontend filter | Searches titles only. Add indexed content search. |
| Persist messages/status/token fields | Complete | Fair | SQLite | Transient state recovery absent; tokens not surfaced. |
| Append-only edit branch | Complete | Good concept | Parent/branch message schema | Multi-write operation is not transactional; branch visualization is minimal. |
| Regenerate/assistant alternatives | Complete | Good concept | Message tree/path lookup | Path resolution is N+1; switching only follows deepest descendant. Add branch map and recursive query. |
| Stop generation | Partial | Poor after restart | In-memory cancellation flag | Works only while the matching process task survives. Add durable cancellation/recovery. |
| Auto-scroll/follow output | Missing | — | UI scroll state | Required for usable long/streaming chat. |
| Archive | Schema only | Poor | archived column | No command or UI. |
| Per-conversation system prompt/settings | Schema only | Poor | system_prompt, temperature, max_tokens columns | No product surface. Either implement deliberately or remove/defer schema promises. |

### 7.3 Provider and model support

| Feature | Status | Quality | Dependencies | Known limitations and improvement |
|---|---|---|---|---|
| Ollama health/model discovery/chat | Partial | Fair | reqwest, local Ollama API | Real model not exercised; timeout and terminal-stream defects; no pull/delete/load management. |
| OpenAI-compatible local host | Partial | Fair | reqwest, /v1/models and chat completions | Malformed SSE is ignored; no endpoint capabilities negotiation or auth. |
| Built-in llama-server scaffold | Broken as distributed | Poor | setup scripts, llama-server resource, GGUF path | No binary in repository/bundle, manual path, generic diagnostics, stdout/stderr discarded. |
| Provider/model selection | Complete | Fair | provider/model DB | Stale model selection handling is weak; no model metadata/context limits/hardware fit. |
| Provider settings | Partial | Poor validation | SQLite, Settings UI | streamingEnabled is always written true; API key reference unused; arbitrary remote URL can retain “local.” |
| Provider health refresh | Complete | Fair | HTTP timeouts | Delays bootstrap and produces coarse error categories. Run asynchronously and preserve last-known state. |
| Cloud providers | Missing | — | Secure credential storage, HTTP adapters | Documentation says disabled; architecture has unused api_key_ref. Implement only after privacy route enforcement and OS keychain. |

### 7.4 Chat rendering and generation

| Feature | Status | Quality | Dependencies | Known limitations and improvement |
|---|---|---|---|---|
| Streaming text | Partial | High-risk | Tauri events, reqwest byte streams, SQLite | Event race, total timeout, EOF integrity, per-chunk O(n²) work. Redesign lifecycle before release. |
| Markdown/GFM | Complete | Good | react-markdown, remark-gfm | No raw HTML is a security strength. Add link policy and rendering tests. |
| Syntax highlighting | Complete | Fair | highlight.js subset | Re-highlights growing content on every chunk; supported language set is limited. Defer highlighting until code fence closes or throttle. |
| Copy code | Complete | Good | Clipboard API | Add accessible success announcement. |
| Error/partial response persistence | Partial | Fair concept | message status/error fields | Recovery and terminal reconciliation are missing. |
| Streaming toggle | Broken | Poor | Settings/API field | UI hard-codes true ([SettingsView.tsx:273](../src/features/settings/SettingsView.tsx#L273)); adapters always stream. Remove the false control or implement both paths. |

### 7.5 Files, data portability, and workspaces

| Feature | Status | Quality | Dependencies | Known limitations and improvement |
|---|---|---|---|---|
| Markdown conversation export | Complete | Good | Rust export module, save dialog | No batch export or attachment support. |
| JSON export/import | Partial | Fair | Rust validation, file dialog | Import is unbounded and non-transactional; provider metadata is not faithfully restored; transient statuses accepted. |
| User-selected workspace | Partial | Fair | SQLite, filesystem | Requires restart, does not migrate existing data, and has no backup/rollback. |
| Workspace write probe | Broken edge case | Poor | filesystem | Overwrites then deletes a fixed .ark-write-test filename ([workspace/mod.rs:165](../src-tauri/src/workspace/mod.rs#L165)). Use create_new with a UUID and guaranteed cleanup. |
| Backup/restore | Missing | — | SQLite checkpoint/copy/export | Required before production. Add verified snapshot, restore preview, and retention guidance. |
| Attachments/files | Missing | — | Safe file ingestion/storage | Right panel is placeholder only. |

### 7.6 Diagnostics and local runtime

| Feature | Status | Quality | Dependencies | Known limitations and improvement |
|---|---|---|---|---|
| CPU/RAM/disk/provider diagnostics | Partial | Fair | sysinfo, provider checks | GPU unknown; disk is not workspace-specific; recommendations are heuristic. |
| Model benchmark | Partial | Poor measurement | Provider chat | Whitespace “tokens,” load time mixed with generation, swallowed errors, unused preview. Name it an estimate and measure TTFT/inter-token timing. |
| Setup guidance | Complete | Good MVP | Diagnostics rules | Nine guidance tests pass. Keep rules versioned and platform-aware. |
| Built-in process lifecycle | Partial | High-risk | Tauri sidecar/Command | Loopback bind is good; no auth secret, logs discarded, fixed port range scan, generic timeout, no packaged runtime. |

### 7.7 Functionality bugs and edge cases not otherwise covered

- Import has no maximum file size, message count, content length, or branch depth, creating a local memory/disk denial-of-service path.
- Database operations use a single process-wide mutex; a slow or repeated write blocks all commands.
- Mutex unwraps in built-in-runtime commands can panic after lock poisoning.
- Selected provider/model state can become stale when a model is removed externally.
- Streaming preference is stored in both provider/conversation schema without a coherent source of truth.
- Built-in model path lives in localStorage while most durable settings live in SQLite, undermining portable workspaces.
- Sidecar output is discarded, making real failure diagnosis difficult.
- The benchmark's output preview is calculated but not displayed.
- Workspace switching changes the database location but offers no copy/move flow for prior history.
- There is no graceful database corruption, schema incompatibility, insufficient-disk, or concurrent-instance handling experience.

## 8. Security findings

### 8.1 Threat-model summary

Ark is currently a single-user desktop client with no inbound application server, account, cloud sync, browser extensions, or tools that modify external systems. That eliminates many web-SaaS risks. The primary assets are conversation content, local model/file paths, workspace data, future credentials, and the integrity of downloaded native binaries/models. The primary boundaries are webview → Tauri commands, Tauri → SQLite/filesystem, Tauri → provider HTTP, and Tauri → native sidecar.

### 8.2 Control assessment

| Area | Rating | Evidence and risk | Action |
|---|---|---|---|
| Authentication | Not applicable today | No multi-user or remote account surface. | Do not add cosmetic local login. Before sync/mobile, implement real OIDC/OAuth PKCE and device/session revocation. |
| Authorization | Good baseline | Narrow Tauri core/event capabilities; no broad FS/shell plugin exposed. | Keep least privilege; define command-level authorization/capability scopes before plugins/tools. |
| Session management | Not applicable today | No server session. | Mobile/sync design needs short-lived tokens, refresh rotation, secure storage, and logout/revoke. |
| Secrets | Incomplete | api_key_ref exists but is unused; no cloud key workflow. | Store secrets in OS keychain/Secure Enclave-backed APIs, never SQLite/localStorage/logs. |
| API security | High risk if remote/LAN | Editable “local” URLs accept arbitrary hosts; built-in server has no token. | Validate host class, require TLS/auth remotely, use random bearer secret for local sidecar, restrict CORS/trusted hosts. |
| Encryption at rest | Missing | SQLite transcripts are plaintext. | Clearly document OS account/full-disk-encryption dependency now; add optional encrypted database/key management for sensitive users. |
| File handling | Needs hardening | Unbounded import; fixed-name write probe can clobber; model files feed a native parser. | Bound sizes/counts/depth, use create_new random probes, validate paths/files, isolate model parser resources. |
| CSP/webview | Good baseline | self-only scripts, loopback connect source, raw Markdown HTML disabled. style-src allows unsafe-inline. | Preserve strict CSP, evaluate nonce/hash-compatible styling, add explicit external-link handling. See [Tauri CSP guidance](https://v2.tauri.app/security/csp/). |
| XSS | Low current risk | React escapes UI, Markdown raw HTML disabled, syntax output is rendered through library output. | Add hostile Markdown/import regression tests; sanitize if HTML is ever enabled. |
| CSRF | Not applicable today | No cookie-authenticated HTTP application API. | Reassess when remote control/sync is introduced. |
| SQL injection | Low | Parameterized rusqlite queries found throughout. | Preserve parameterization; add import fuzz/property tests for integrity, not injection. |
| Input validation | Weak | Numeric and URL validation insufficient; import/model/workspace limits absent. | Centralize Rust validation with typed errors and duplicate basic guidance client-side. |
| Prompt injection | Low current capability, high future risk | Models cannot currently call tools, browse, or retrieve private documents. | For RAG/tools, treat retrieved text as untrusted data; isolate system/user/tool channels, scope capabilities, preview side effects, require approval. |
| IPC | Good baseline, limited policy | Tauri command surface is explicit, but validation is inconsistent and large command module raises review cost. | Use typed request objects, per-command validation, smaller services, and negative IPC tests. |
| Updates | Missing | No updater/signing/rollback. | Signed artifacts and signed update manifests are release blockers. Review [Tauri security lifecycle](https://v2.tauri.app/security/lifecycle/). |
| Supply chain | High | Setup scripts download pinned llama.cpp archives/executables without checksums or signatures. Rust advisories present. | Publish SHA-256/SLSA provenance, verify before extraction, generate SBOM/licenses, pin package-manager versions, scan dependencies in CI. |

### 8.3 Dependency scan results

**JavaScript production dependencies:** pnpm audit reported no known vulnerabilities on 2 August 2026.

**Rust lockfile:** cargo-audit 0.22.2 scanned 477 dependencies and reported:

- RUSTSEC-2026-0204: crossbeam-epoch 0.9.18 invalid pointer dereference; upgrade to 0.9.20 or later. Dependency path: Ark → sysinfo → rayon → rayon-core → crossbeam-deque → crossbeam-epoch.
- RUSTSEC-2026-0194: quick-xml 0.39.4 quadratic duplicate-attribute checking, CVSS 7.5; upgrade to 0.41.0 or later.
- RUSTSEC-2026-0195: quick-xml 0.39.4 unbounded namespace allocation, CVSS 7.5; upgrade to 0.41.0 or later. Dependency path for both: Tauri utilities → plist → quick-xml.
- Seventeen allowed warnings, mostly unmaintained GTK3 bindings on platform-specific paths, unmaintained UNIC packages/proc-macro-error, and a glib unsoundness advisory.

The quick-xml path may mainly process controlled plist/build data in current usage, and GTK warnings are platform-specific. That lowers likely exploitability but does not make an unreviewed vulnerable release acceptable. Upgrade, test all targets, then document any narrowly justified exception.

### 8.4 Native runtime and model safety

The built-in server binds loopback, which is correct, but “localhost” is not an authentication boundary: another local process—and sometimes browser-origin traffic if server CORS is permissive—can probe or consume it. Start it with a high-entropy per-launch token, pass that token in Ark's requests, disable permissive CORS, and terminate it reliably on all app exit/crash paths.

GGUF and runtime binaries are complex native inputs. Validate file type and size before launch, apply memory/CPU limits where the OS allows, keep the parser/runtime patched, and never download/execute binaries without integrity verification. Model provenance and license should be displayed and stored.

## 9. Architecture findings

### 9.1 Current architecture

~~~mermaid
flowchart LR
    UI["React UI<br/>App + feature components"] -->|"Tauri invoke"| CMD["Rust command module"]
    CMD --> DB["SQLite repository<br/>single Mutex Connection"]
    CMD --> EXP["Import/export"]
    CMD --> DIAG["Diagnostics"]
    CMD --> PROV["Provider adapters"]
    CMD --> SIDE["llama-server process"]
    PROV --> OLL["Ollama HTTP"]
    PROV --> LOC["OpenAI-compatible HTTP"]
    CMD -->|"Tauri events"| UI
    SIDE --> LOC
~~~

The top-level dependency direction is understandable, but commands are both application service and transport controller. UI state is similarly centralized in App and passed through a large ChatView. This is manageable at MVP size and becomes fragile with sync, mobile, RAG, tools, or multiple windows.

### 9.2 Findings

| Severity | Finding | Consequence | Recommendation |
|---|---|---|---|
| High | Stream lifecycle crosses command return values, events, React optimistic state, SQLite status, and in-memory cancellation without one authoritative state machine. | Race and restart defects already exist. | Define states and allowed transitions in one domain service; persist transitions atomically; make events notifications of committed state. |
| High | Database migration runner executes migration SQL at startup and records a version, without a true ordered version gate/transactional upgrade strategy. | Future schema evolution, downgrade, and partial-failure behavior are unsafe. | Use ordered, checksummed migrations in a transaction, validate current/target version, backup before destructive upgrades, and test upgrades from every supported release. |
| High | Single Mutex<Connection> serializes all database work. | Streaming writes can block reads/settings/import; lock poisoning can cascade. | Use a small connection pool or dedicated DB worker, WAL/busy timeout, explicit transactions, and non-panicking lock errors. |
| Medium | Command module combines validation, orchestration, transactions, HTTP/provider choice, sidecar lifecycle, diagnostics, and import/export. | Harder review/testing and increasing merge risk. | Extract application services by workflow: conversations, generation, provider registry, workspace, diagnostics. Keep Tauri commands thin. |
| Medium | Provider choice is a closed enum/switch rather than a trait/capability registry. | Every new provider changes central orchestration; capabilities are assumed. | Introduce a Provider trait plus declared capabilities: streaming, models, vision, tools, embeddings, auth, context window. |
| Medium | UI calls Tauri functions directly and keeps broad cross-feature state in App. | Business behavior cannot be reused by mobile or tested without the bridge. | Introduce a typed client interface and focused state/query layer; keep domain transitions in pure TypeScript or Rust with generated contracts. |
| Medium | Message path lookup performs one query per ancestor. | Cost grows linearly in round trips and lock duration. | Use a recursive CTE or materialized branch/path representation. |
| Medium | Schema contains unused product concepts and duplicated settings. | Ambiguous ownership and false completeness. | Write an explicit schema/domain contract; either implement or migrate away dead fields after compatibility review. |
| Low | No lint/format configuration or CI quality gate; strict clippy currently fails. | Style and warnings drift. | Add rustfmt, clippy -D warnings with documented exceptions, TypeScript linting, formatting, and CI gates. |

Strict clippy found two immediate quality-gate failures: a database method with too many arguments and a useless format invocation in sidecar code. Neither is a production incident, but the fact that the gate is red should be resolved before making it mandatory.

### 9.3 Target architecture

~~~mermaid
flowchart TB
    D["Shared domain contracts<br/>conversation, message, provider, sync"] --> UC["Application use cases<br/>transactional state transitions"]
    UC --> PORTS["Ports<br/>repository · inference · files · secrets · telemetry"]
    PORTS --> DESK["Desktop adapters<br/>Tauri + SQLite + local providers"]
    PORTS --> MOB["Mobile adapters<br/>HTTPS/LAN + Expo SQLite/SecureStore"]
    UC --> EVT["Versioned event/protocol contracts"]
    EVT --> DESKUI["React desktop UI"]
    EVT --> MOBUI["React Native mobile UI"]
~~~

This is not a request for a large rewrite. Extract the stream lifecycle and typed contracts first because they fix current defects and create the future boundary. Move other workflows only when touched.

## 10. Performance findings

### 10.1 Measured build characteristics

Production frontend build:

| Asset | Raw | Gzip | Assessment |
|---|---:|---:|---|
| Initial JavaScript | 367.11 kB | 117.21 kB | Acceptable for desktop MVP. |
| Chat chunk | 261.23 kB | 80.34 kB | Largest lazy chunk, primarily rich rendering/highlighting; monitor. |
| Settings chunk | 19.41 kB | 5.47 kB | Good. |
| CSS | 17.88 kB | 4.49 kB | Good. |

Vite transformed 2,271 modules and completed successfully. These are build outputs, not startup-time or memory measurements. No production instrumentation exists for those metrics.

### 10.2 Bottlenecks

**High — quadratic stream persistence and emission.** Every delta updates SQLite by concatenating content, re-reads the full accumulated content, and emits that full content. React then maps the message array and reparses/re-highlights the growing Markdown. For n chunks, total copied/parsed data approaches O(n²).

Remediation:

- Accumulate deltas in a generation task buffer.
- Emit small deltas to a dedicated streaming store, not the entire transcript.
- Flush a checkpoint to SQLite every 50–100 ms or size threshold, then one final transaction.
- Parse stable completed Markdown blocks; defer highlighting an open code fence.
- Measure TTFT, inter-token latency, render long-task time, DB writes, and dropped frames.

**High — full-history work.** Bootstrap loads all conversation summaries; selecting a conversation resolves its full active path with one query per ancestor; the UI renders the whole path with no virtualization. Add cursor pagination, recursive path retrieval, and list/message windowing after correctness changes.

**Medium — synchronous provider bootstrap.** Initial provider refresh can spend three seconds on health and five seconds on model listing before the application exits loading. Render cached application data first, mark provider state stale, and refresh concurrently.

**Medium — global database serialization.** The single SQLite mutex amplifies per-chunk write cost and makes unrelated settings/history commands wait. Use WAL, transactions, a pool/dedicated worker, and bounded busy handling.

**Medium — local-runtime resource governance.** There is no model memory estimate, GPU detection, concurrency setting, context-budget guard, backpressure, or process resource telemetry. A user can launch a model that swaps the machine. Provide model-size/hardware-fit checks and one-generation-at-a-time policy initially.

### 10.3 Performance acceptance budget

Establish budgets before optimization:

- Cached shell interactive: ≤1.0 s on reference hardware.
- Provider refresh never blocks shell or history access.
- First 1,000 conversations: sidebar search/filter ≤100 ms with pagination.
- 100,000-character transcript: scroll remains responsive and stream update work stays below one frame budget at the selected flush rate.
- Database writes during streaming: ≤20 batches/second, with one terminal commit.
- Cancellation UI acknowledgement: ≤100 ms; provider/process termination best effort with explicit state.
- Memory: measure baseline, 100-message chat, 100k-character code response, and one local model separately.

## 11. Competitive analysis

Competitor features change quickly; this comparison is based on official documentation available on the audit date.

| Capability | Ark | Current competitive baseline | Gap |
|---|---|---|---|
| Local chat | Ollama, OpenAI-compatible host, incomplete built-in server | LM Studio and Jan provide in-app model discovery/download/load; Open WebUI supports broad providers; Ollama supplies a mature runtime | Large operational gap despite a good local-first premise. |
| Model management | Discovery of already-running provider models | [LM Studio](https://lmstudio.ai/docs/app/basics) and [Jan](https://www.jan.ai/docs/desktop/quickstart) expose model hubs and lifecycle controls | Table stakes: download progress, storage, compatibility, load/unload, deletion, hardware fit. |
| Cloud models | None | ChatGPT/Claude are cloud-first; Jan, Msty, Open WebUI support local and cloud choices | Important but should follow secure key storage and route disclosure. |
| Projects/workspaces | Filesystem workspace is only a database location | [ChatGPT Projects](https://help.openai.com/en/articles/10169521-projects-in-chatgpt), [Open WebUI Workspace](https://docs.openwebui.com/features/workspace/), Jan, and Msty organize instructions, files, prompts, models, and chats | Ark lacks user-facing project organization. |
| RAG/knowledge | None | [Open WebUI features](https://docs.openwebui.com/features/), [AnythingLLM](https://docs.anythingllm.com/), and Msty emphasize documents, knowledge bases, retrieval, and citations | Major gap for a personal AI workspace. |
| Tools/agents/MCP | None | Claude supports [local MCP servers](https://support.anthropic.com/en/articles/10949351-getting-started-with-local-mcp-servers-on-claude-desktop); LM Studio has MCP/stateful API features; Open WebUI and AnythingLLM expose tools and agents | Major feature gap, but also a chance to lead on explicit capability safety. |
| Prompt/persona management | None | Open WebUI, Msty, Jan and AnythingLLM expose prompts/personas/assistants | Expected power-user feature. |
| Attachments/vision/media | None | ChatGPT, Claude, Jan, Msty, and recent Ollama models support multimodal work | Table stakes for general assistants. |
| Voice | None | ChatGPT Desktop and major mobile assistants offer voice; competitor desktop clients increasingly do | High-value mobile/accessibility gap. |
| Search/web | Title-only local search; no web search | ChatGPT Projects includes web search; Ollama and agent clients expose web/tool flows | Content search is table stakes; web search is a later capability. |
| Conversation branches | Append-only edit/regeneration paths | Cloud leaders offer edits/regeneration, but local clients vary in branch transparency | Potential Ark strength if visualized and made reproducible. |
| API/server | Client only | [LM Studio REST API](https://lmstudio.ai/docs/developer/rest), [Jan local API](https://jan.ai/docs/api-server), Ollama, and Open WebUI expose integration APIs | Needed for phone companion and ecosystem integrations. |
| Multi-user/RBAC | None | Open WebUI provides [permissions/RBAC](https://docs.openwebui.com/features/authentication-access/rbac/permissions/) | Not needed for single-user desktop; required only for hosted/team edition. |
| Mobile/sync | None | [ChatGPT's desktop experience](https://help.openai.com/en/articles/20001276/) syncs across web/mobile/desktop; cloud assistants have mature mobile apps | Strategic gap; architecture is not prepared. |
| Automation/artifacts | None | Claude offers [Artifacts](https://support.anthropic.com/en/articles/9487310-what-are-artifacts-and-how-do-i-use-them); ChatGPT and AnythingLLM expose tasks/flows; Msty exposes automation | Later differentiator, not a pre-release blocker. |

Msty's official [feature overview](https://msty.ai/studio/features) is particularly instructive for local power-user expectations: model hub, prompt/persona/skills, split chats, agents, RAG, media, MCP, automation, and projects. Ollama's official [tool support](https://ollama.com/blog/tool-support) and [embedding models](https://ollama.com/blog/embedding-models) show that the underlying local ecosystem already supports capabilities Ark does not surface.

### Where Ark is stronger

- Simpler, inspectable local-first architecture with no account or telemetry dependency.
- Append-only branch storage is a promising foundation for reproducible edits/regenerations.
- Portable SQLite workspace and open Markdown/JSON exports reduce lock-in.
- Tauri plus a narrow capability surface can provide a smaller and safer desktop shell.
- Diagnostics and local-provider setup are treated as first-class rather than hidden behind a cloud default.

These are architectural advantages, not yet polished user advantages. The privacy badge defect and missing bundled runtime currently undermine the strongest claims.

### Differentiation opportunities

1. **Auditable private routing:** show and enforce, per response, exactly which endpoint/model received which context; include a local/remote route ledger and redaction preview.
2. **Branch-aware research notebook:** turn the existing message tree into a visible comparison/reproducibility workflow with named branches, model/settings provenance, and merge/export.
3. **Local AI control plane:** manage Ollama/llama.cpp/LM Studio-compatible runtimes, hardware fit, storage, health, routing, fallback, and LAN phone access in one place.
4. **Safety-first tools:** capability-scoped MCP/tools with a preview, per-tool data boundary, explicit approval, and immutable audit log.

Do not try to match every competitor at once. Reliability plus managed local models plus one differentiated branch/privacy workflow is a credible next-release position.

## 12. Mobile readiness assessment

### Score: 10/100

**Reusable today:**

- Conceptual domain entities and UUID identifiers.
- SQLite-shaped conversation/message model, subject to migration redesign.
- Some TypeScript DTO definitions.
- Provider concepts and Markdown content format.
- Design tokens can be extracted from the existing palette.

**Not reusable as-is:**

- React DOM components, CSS utilities, Framer Motion behavior, and the fixed desktop shell.
- Direct Tauri invoke/event calls throughout application behavior.
- Rust command implementations as mobile business logic without a service/FFI boundary.
- Desktop filesystem paths/dialogs, localStorage settings, sidecar process spawning, and localhost assumptions.
- Built-in llama-server sidecar: iOS cannot use the desktop process model.

### Recommended approach: Expo/React Native companion

Use Expo/React Native in a pnpm monorepo for the first iPhone product. It preserves React/TypeScript skills and shared domain contracts while using native navigation, SecureStore, notifications, camera, microphone, share sheet, and files. Expo supports [monorepos](https://docs.expo.dev/guides/monorepos/), documents platform [data storage choices](https://docs.expo.dev/develop/user-interface/store-data/), and provides an [authentication guide](https://docs.expo.dev/guides/authentication/).

~~~text
apps/
  desktop/             existing React + Tauri shell
  mobile/              Expo Router / React Native
packages/
  domain/              pure types, validation, state transitions
  protocol/            versioned API/event schemas
  design-tokens/       color, spacing, typography semantics
  test-fixtures/       provider streams and conversation graphs
services/
  companion/           authenticated sync/LAN API if needed
~~~

Define ports such as ConversationRepository, InferenceClient, SecureStore, FilePicker, Notifications, VoiceInput, and CameraInput. Desktop adapters use Tauri/SQLite/local providers; mobile adapters use Expo SQLite/SecureStore and an authenticated HTTPS or LAN companion protocol.

### Why not the alternatives

- **PWA:** fastest visual prototype, but weaker secure storage, background work, native file/camera/voice integration, notifications consistency, App Store presence, and on-device inference options. Not suitable as the strategic iPhone application.
- **Flutter:** good cross-platform UI, but duplicates the existing React/TypeScript ecosystem and shares little code.
- **Native Swift:** best if high-performance, on-device Metal/Core ML/llama.cpp inference is the central product. It has the highest staffing and duplication cost.
- **Tauri mobile:** possible for some shared Rust logic, but does not make the React DOM UI mobile-native and cannot reuse the desktop sidecar model. It is not the lowest-risk first mobile path here.

If offline on-device iPhone inference is non-negotiable, use a custom Swift/Metal/Core ML or llama.cpp native module and accept a separate 4–8+ week engineering track. Expo Go will not be sufficient for custom native inference; use development builds/native modules. Expo SQLite offers optional [SQLCipher support](https://docs.expo.dev/versions/v54.0.0/sdk/sqlite/), but key lifecycle and export policy still require design.

### Architectural changes to make now

1. Replace direct invoke calls with a typed ArkClient interface.
2. Extract pure validation, entities, stream state transitions, and import schemas into a shared package.
3. Define a versioned conversation/provider protocol; never expose raw database tables as the mobile API.
4. Add a durable change log/outbox, tombstones, revision IDs, idempotency keys, and deterministic conflict rules.
5. Separate “provider running on this device” from “provider reachable over authenticated LAN/cloud.”
6. Use OAuth/OIDC PKCE for accounts, Keychain/SecureStore for tokens, TLS, device revocation, and optional end-to-end conversation encryption.
7. Design offline-first writes and background sync within iOS execution limits.
8. Treat camera, microphone, files, notifications, and local-network discovery as explicit permissions with graceful denial states.

### Suggested mobile product sequence

1. Responsive product design and shared protocol, without claiming code reuse of desktop UI.
2. Read/search synced history and continue cloud/LAN-provider chats.
3. Offline draft/history cache with conflict-safe sync.
4. Push completion notifications, share sheet, voice input, camera/files.
5. Optional local-network desktop runtime discovery and pairing.
6. Evaluate true on-device inference only with validated user demand and hardware targets.

## 13. Technical debt register

| Debt | Severity | Cost if deferred | Recommended disposition |
|---|---|---|---|
| Distributed stream state machine | Critical | More race/recovery defects in every provider/mobile client | Resolve in Phase 0. |
| Invalid packaging assets/no release pipeline | Critical | No releasable artifact | Resolve in Phase 0. |
| Non-transactional workflows | High | Data inconsistency and hard migrations/sync | Resolve in Phase 0. |
| Known Rust advisories | High | Security exposure and failed compliance gates | Upgrade in Phase 0. |
| Fixed-name workspace probe | High | User-file deletion edge case | Replace immediately. |
| Global SQLite mutex/per-delta writes | High | Severe long-response degradation | Correctness-safe batching in Phase 0/1. |
| Oversized ChatView/command/database/provider modules | Medium | Slow review and feature coupling | Extract only along workflow changes; no big-bang rewrite. |
| Ad hoc migration runner | High | Unsafe upgrades after first public release | Build real migration tests before shipping schema v2. |
| Dead/duplicated schema fields | Medium | Ambiguous product semantics | Document ownership; implement or migrate after compatibility review. |
| localStorage/SQLite split | Medium | Non-portable settings and sync complexity | Create one settings contract with explicit device vs workspace scope. |
| Discarded sidecar logs | Medium | Support cannot diagnose runtime failures | Structured redacted logging and diagnostics bundle. |
| Documentation drift | Medium | Incorrect user expectations and support load | Generate feature/support matrix from release state; update now. |
| Missing license/security/privacy/release docs | High | Distribution and trust risk | Add before public release. |
| No frontend/provider/E2E tests | High | Critical paths regress silently | Add layered tests in Phase 0/1. |

## 14. Missing-features roadmap

Complexity assumes one experienced engineer familiar with the stack: **S** ≤3 focused days, **M** approximately 1–2 weeks, **L** approximately 3–6 weeks, **XL** multiple milestones. Estimates include implementation and focused tests, not external review/App Store wait time.

| Order | Capability | Priority | Complexity | Dependencies | Definition of done |
|---:|---|---|---|---|---|
| 1 | Stream recovery/order/integrity | Critical | M | Transaction service, mock providers | Crash/restart, instant response, malformed/truncated stream, cancel, and timeout cases converge to a usable terminal state. |
| 2 | Transactional data/migrations/backups | Critical | M | SQLite design | Atomic mutations, tested upgrades, verified snapshot/restore, corruption-safe startup. |
| 3 | Installable signed desktop release | Critical | M | Icons, CI runners, signing credentials | Clean-machine install/update/uninstall smoke test on supported platforms. |
| 4 | Responsive/accessibility state redesign | High | M | Shell/component work | No clipping at supported desktop sizes, viable one-column mobile webview layout, WCAG AA automated/manual checks. |
| 5 | Managed local models | High | L | Runtime packaging/provenance, hardware diagnostics | Discover/download/verify/load/unload/delete with storage and compatibility feedback. |
| 6 | Conversation organization/search | High | M | Pagination/indexing | Content search, archive/pin, folders/projects, efficient large-history behavior. |
| 7 | Secure cloud provider support | High | M | OS keychain, route disclosure, provider registry | Keys never enter DB/logs; explicit remote badge; tested retries/rate limits/stream protocols. |
| 8 | Attachments and vision | High | L | Safe blob storage, provider capabilities | Size/type validation, preview/remove, provenance, local/remote disclosure. |
| 9 | Prompt/persona/project instructions | Medium | M | Settings ownership/project model | Versioned reusable prompts with per-project instructions and clear injection boundaries. |
| 10 | RAG with citations | High | XL | Attachments, embeddings, index lifecycle, evaluation | Chunk/index controls, source citations, deletion/reindex, retrieval quality/security tests. |
| 11 | Tools/MCP/agents | Medium | XL | Capability policy, secrets, audit log, prompt-injection defenses | Scoped tools, side-effect preview/approval, revocation, immutable audit trail, adversarial tests. |
| 12 | Voice input/output | Medium | M–L | Permissions, provider/on-device choice | Accessible recording state, transcription privacy route, cancellation, error recovery. |
| 13 | Companion API and iPhone client | High strategic | XL | Shared domain/protocol, auth, sync/outbox | Offline-safe history/chat, secure pairing/login, notifications, files/voice/camera, App Store-quality UX. |
| 14 | Multi-user/RBAC | Low for desktop | XL | Hosted service, identity, audit | Defer unless team/server edition becomes a product goal. |
| 15 | Automations/artifacts | Low | XL | Tools/agents/scheduler/sandbox | Defer until safe tool foundation and product demand. |

“Planned but unfinished” evidence includes the right-panel Context/Files/Memory placeholders, archive/system-prompt/per-conversation-generation fields in the schema, api_key_ref, a nonfunctional streaming toggle, and the built-in sidecar/resource scaffold. These should be tracked explicitly rather than presented as available.

## 15. Prioritized action plan

### P0 — release blockers

1. Specify generation states and event ordering; implement crash/import recovery and terminal reconciliation.
2. Move send/edit/regenerate/import to transactional application services.
3. Fix Unicode title truncation and randomized create-new workspace probing.
4. Replace whole-request stream timeouts; reject malformed/truncated terminal streams.
5. Add a deterministic mock Ollama/OpenAI-compatible server and integration tests for chunking, cancellation, instant completion, EOF, invalid JSON, and reconnect/restart.
6. Upgrade vulnerable Rust dependencies and make npm/Rust advisory scanning a CI gate.
7. Decide the truthful built-in-runtime scope. Either package a verified runtime or remove “ships with” claims and hide the provider in release builds.
8. Generate valid platform icons and get a clean debug/release installer through CI.
9. Add responsive desktop breakpoints, action overflow, recovery states, auto-scroll, and core accessibility semantics.
10. Enforce local/remote URL classification and privacy disclosure in Rust.

### P1 — production hardening

1. Add signed installers, updater signatures, staged release channels, rollback procedure, and support matrix.
2. Add structured redacted logs, a user-controlled diagnostics bundle, opt-in crash reports, and health metrics. Preserve the no-behavioral-analytics default.
3. Add real database migrations, WAL/busy behavior, backups, restore tests, and concurrent-instance policy.
4. Batch streaming persistence/render updates and instrument startup, TTFT, throughput, memory, and long-task rendering.
5. Add frontend unit tests, accessibility/keyboard tests, native E2E smoke tests, and platform CI.
6. Add onboarding truthfulness, native file/folder pickers, numeric/URL/file/import validation, and scoped loading/success/error states.
7. Publish license, privacy, security reporting, model/binary provenance, third-party notices, support, and release documentation.

### P2 — competitive core

1. Managed model discovery/download/verification/storage/load/unload.
2. Conversation content search, archive/pin, folders/projects, prompt/persona library.
3. Secure cloud providers with keychain storage and explicit route/provenance UI.
4. Attachments and vision with provider capability negotiation.
5. A versioned companion API and shared domain package for mobile.

### P3 — knowledge, tools, and mobile

1. RAG/embeddings/citations with retrieval evaluation and lifecycle controls.
2. MCP/tools/agents only after capability scoping, approval UX, secrets policy, and prompt-injection tests.
3. Expo iPhone client with offline cache/sync, notifications, voice, camera/files, and authenticated LAN/cloud routing.

## 16. Estimated development phases

These are planning ranges for one experienced full-time engineer plus fractional product design, QA, and security/release support. Parallel staffing can reduce calendar time but not eliminate sequencing dependencies.

| Phase | Duration | Outcome | Exit criteria |
|---|---:|---|---|
| Phase 0: Correctness and bundle | 2–4 weeks | Core chat cannot wedge/corrupt silently; distributable debug artifact | All P0 reliability tests green; no known unreviewed high advisory; bundle installs on primary Windows target. |
| Phase 1: Production hardening | 3–5 weeks | Signed, observable, recoverable desktop release candidate | CI/platform matrix, signed update path, backup/restore, accessibility baseline, E2E smoke, release docs. |
| Phase 2: Competitive local workspace | 5–8 weeks | Managed models and useful organization | Model lifecycle, search/projects/prompts, performance budgets met, privacy route enforcement. |
| Phase 3: Knowledge and integrations | 6–10 weeks | Attachments/RAG and carefully scoped tools | Retrieval evaluation, citations, safe tool approvals/audit, adversarial security review. |
| Phase 4: iPhone foundation/client | 6–10 weeks after protocol foundation | Secure companion app | Shared contracts, auth/pairing, offline-safe sync, core chat/history, notifications and mobile QA. |
| Optional on-device iPhone inference | +4–8+ weeks | Native local inference experiment/product slice | Supported-device performance/thermal/memory targets, model packaging/licensing, native security review. |

The fastest credible public release is Phase 0 plus Phase 1. Shipping broad new features before that would multiply state, migration, and support risk.

## 17. Production readiness assessment

### Score: 24/100 — not ready

| Readiness dimension | Score | Release assessment |
|---|---:|---|
| Stability | 30 | Core happy path exists, but restart/race/EOF/Unicode failures are release-blocking. |
| Maintainability | 45 | Understandable stack, but large central modules and weak boundaries will not scale safely. |
| Documentation | 30 | Product plans are detailed; user/release/security docs are stale or missing. |
| Monitoring/logging | 10 | No structured app logs, crash reports, support bundle, or performance telemetry; sidecar output discarded. |
| Automated quality | 25 | Typecheck/build and 18 Rust tests pass; critical provider/UI/native paths untested; clippy gate red. |
| CI/CD and deployment | 5 | No CI/CD; bundling fails; no signing/updater/rollback. |
| Data protection/recovery | 20 | Local SQLite and export exist; no encryption, backup/restore, migration safety, or crash recovery. |
| Configuration management | 35 | Settings/workspace support exists; validation and device/workspace ownership are inconsistent. |
| Security | 40 | Good webview baseline, but current advisories, remote/local ambiguity, supply-chain, and update gaps remain. |
| User experience | 45 desktop / 5 mobile | Coherent normal-width desktop surface; incomplete states, minimum-width clipping, no mobile. |

### Verification record

| Check | Result |
|---|---|
| TypeScript typecheck | Passed |
| Production Vite build | Passed |
| Rust tests | Passed: 18 |
| Strict clippy | Failed: two warnings promoted to errors |
| pnpm production dependency audit | Passed: no known vulnerabilities |
| Rust dependency audit | Failed: 3 vulnerabilities, 17 allowed warnings |
| Tauri debug bundle | Failed at Windows icon packaging |
| Rendered desktop UI | Reviewed at 1280×720 and configured 980×720 minimum |
| Rendered iPhone-sized UI | Failed usability at 390×844: center chat collapsed |
| Real Ollama/llama-server generation | Not run; runtime/model unavailable |
| Signed installer/update | Not available |
| macOS/Linux/iOS validation | Not available |

### Go/no-go gates

Do not call the application production-ready until all of the following are true:

- No conversation can remain stuck after crash, restart, cancel, malformed stream, or import.
- Event ordering and terminal stream integrity are covered by deterministic integration tests.
- Logical writes and migrations are transactional with verified backup/restore.
- All supported provider inputs are validated and privacy routing is truthful.
- No unreviewed high/critical dependency advisory exists.
- A signed installer and signed updater pass clean-machine release smoke tests.
- CI runs type, format/lint, unit, provider integration, accessibility, E2E smoke, dependency, and bundle checks.
- Supported desktop widths meet WCAG AA and do not clip primary actions.
- Structured redacted diagnostics and a rollback/support process exist.
- Documentation accurately states which providers/runtimes/platforms are supported.

### Final verdict

Ark should be treated as an **alpha-quality local AI desktop MVP**, not a production release candidate. The underlying idea and several design choices are worth preserving. The next major release should be a reliability and distribution release: make streams deterministic and recoverable, make data mutations atomic, make privacy claims enforceable, make the package installable and updateable, and make the UI work across its declared desktop range. Once those foundations are green, managed local models and workspace organization offer the best near-term competitive return; RAG, tools, and iPhone should build on the shared protocol and safety boundaries established during that work.
