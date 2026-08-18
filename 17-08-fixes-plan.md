# Ark — 17 August Manual-Test Fixes Plan

**Status:** Implementation complete; final browser qualification blocked by external permission  
**Date:** 2026-08-17  
**Authority:** `implementation-plan.md` remains the architectural source of truth. This document refines existing roadmap work and does not mark any master task complete.  
**Scope:** Production implementation, regression coverage, packaging assets, and release qualification for FIX-1708-001 through FIX-1708-015.

## 0. Implementation Record

The 17 August implementation directive approved the plan's recommended test stack, canonical Ark mark, fixed accent palettes, and provisional Brave disposition for this release. Production implementation is present for every task. FIX-1708-001 cannot be marked complete until the required Playwright light/dark viewport baselines are generated and reviewed; the current Codex Browser permission rejects localhost. Master-roadmap statuses remain governed by their own complete acceptance criteria.

| Task | State | Production evidence |
| --- | --- | --- |
| FIX-1708-001 | Blocked by genuine external dependency | Vitest/jsdom/Testing Library/axe and the Playwright viewport/theme/keyboard/accessibility matrix are implemented and enforced in CI. Committed visual baselines still require a localhost Browser session for generation and review. |
| FIX-1708-002 | Complete | System-first UI/code typography, bundled OFL Inter fallback, neutral semantic surfaces, named type/elevation tokens, and shared Card are shipped and checked. |
| FIX-1708-003 | Complete | One sanitized SVG feeds `ArkBrand` and generated desktop/mobile icons; generation and derivative integrity are documented and tested. |
| FIX-1708-004 | Complete | Real bootstrap state owns the immediate startup view; recovery and total bootstrap errors pre-empt it without artificial delay. |
| FIX-1708-005 | Complete | Chat, Code, startup, and shared state panels consume one public lifecycle mapping and reduced-motion activity primitive. |
| FIX-1708-006 | Complete | Durable New Chat has draft confirmation, in-flight dispatch protection, failure preservation, focus restoration, and distinct-row database/E2E coverage. |
| FIX-1708-007 | Complete | Responsive navigation exposes Chat/Code, globally bounded pins, real project filtering/creation, persisted sections, expandable search, and deduplicated chats. |
| FIX-1708-008 | Complete | Bounded typed model presentation includes field provenance, metadata confidence/version, capabilities, runtime, availability, and supported actions; curated entries carry reviewed sources. |
| FIX-1708-009 | Complete | `ark-fit-v1` exposes categorical assessment, evidence, explicit confidence, execution scope, and honest remote/unknown handling without a numeric score. |
| FIX-1708-010 | Complete | Settings Models owns managed local lifecycle, cross-provider installed cards, and a provenance/fit-aware curated Ollama card library with free-tag fallback. |
| FIX-1708-011 | Complete | Chat has one interactive provider/model picker beside Send; the header retains route/provider health and generation provenance wiring is unchanged. |
| FIX-1708-012 | Complete | Tools is first-class Settings navigation; real tool grants, secrets, audit integrity, and Code command policy moved there without duplicate controls. |
| FIX-1708-013 | Complete | Device settings persist only the five typed palettes; release gating, review date, immediate application, and full light/dark contrast tests are present. |
| FIX-1708-014 | Complete | Release uses the Windows GUI subsystem; centralized child flags suppress owned consoles while retaining capture, process groups, cancellation, and developer console behavior. |
| FIX-1708-015 | Complete | ADR 0004 retains Brave provisionally after Brave/Zenserp/Kagi/user-endpoint review; Tools discloses account, cost class, destination, and retention before approval. |

Local qualification on 2026-08-17 passed 85 frontend unit tests, 16 component/accessibility tests, typecheck, lint, formatting, architecture, contract, security, design/support, reference-data, supply-chain, production-web, Rust formatting, strict Clippy, 547 Rust tests with zero failures and one pre-existing ignored cross-platform migration test, and an optimized Windows GUI release build. The platform credential-store round trip passed with the unlocked Windows vault. Interactive localhost Playwright execution and reviewed image-baseline generation were not runnable because the saved Codex Browser permission rejects localhost.

## 1. Executive Summary

Manual testing identified a coherent product-quality gap rather than a collection of unrelated cosmetic defects. Ark's underlying architecture is substantially more complete than some of the findings assumed: Projects, conversation pinning, model lifecycle operations, real Tools settings content, and Ark Code all exist. The current UI does not consistently project those capabilities into a calm, compact, understandable product surface.

The recommended outcome is:

- establish neutral, legally distributable typography and semantic visual foundations before restyling feature screens;
- use one owned brand asset/component and an actual-state startup surface;
- make generation activity consistent across Ark Chat and Ark Code;
- fix and specify the durable New Chat lifecycle;
- redesign the sidebar as one responsive navigation system with a real Ark Chat/Ark Code switch, Pinned, Projects, Chats, compact search, and no duplicate shortcuts entry;
- separate the information-rich Models library from the lightweight composer model selector;
- introduce trustworthy metadata provenance and categorical hardware-fit guidance before considering a numeric score;
- reduce Settings density and promote the already-implemented Tools controls into their own section;
- make normal packaged Windows startup console-free without sacrificing structured diagnostics; and
- retain Brave Search provisionally instead of immediately switching to Zenserp, pending an explicit product/provider decision.

The work should not begin with screen-by-screen CSS. The component-test/accessibility harness, design tokens, metadata contracts, and product decisions below are dependencies for safe implementation.

## 2. Current-State Audit

### 2.1 Overall visual quality and design-system state

The UI uses owned primitives under `src/ui`, Tailwind semantic colour mappings, and shared motion durations. This is a sound base, but it is incomplete:

- `src/styles.css` and `tailwind.config.ts` define colour, radius, font-family, breakpoint, and motion foundations, but no named typography scale, elevation scale, model-card pattern, collapsible-section primitive, expandable-search primitive, or shared activity indicator exists.
- Many feature surfaces still compose local border/padding/text combinations. Model rows, sidebar groups, startup feedback, and activity states therefore do not share a documented hierarchy.
- `scripts/check-design-tokens.mjs` catches nonexistent colour classes but does not validate typography, palette contrast, or component states.

**Likely root cause:** UX-009 was implemented incrementally around then-current screens. The newly implemented Phase 5/Ark Code features expanded the surface area faster than the visual system was formalized.

**Related architecture:** UX-001–009, UX-011, TST-004, `src/ui`, `src/styles.css`, `tailwind.config.ts`.

### 2.2 Typography

`body` and Tailwind currently name `Inter` first, but Ark does not import or bundle Inter. The browser therefore normally falls through to the platform stack. Code blocks specify only a size and inherit the surrounding sans-serif stack; they do not have an explicit code-font token. Typography uses ad hoc Tailwind sizes and line heights rather than a documented product scale.

**Likely root cause:** the intended font name was added as configuration without an owned font asset or a platform-specific fallback decision.

**Recommendation:** use a system-first, legally safe stack: `-apple-system`/`BlinkMacSystemFont` on Apple, `Segoe UI Variable`/`Segoe UI` on Windows, and a bundled OFL-licensed Inter Variable fallback for Linux and systems without those fonts. Do not redistribute San Francisco or Segoe. Use an explicit system monospace stack (`ui-monospace`, `SFMono-Regular`, `Cascadia Code`, `Cascadia Mono`, `Menlo`, `Consolas`, `Liberation Mono`, monospace); do not bundle proprietary fonts. Record the Inter OFL notice if the font is added. Define named UI tiers rather than introducing arbitrary sizes: caption 12/16, metadata 13/18, body 14/21, emphasized body 16/24, section title 18/24, view title 24/30, with weights 400/500/600. Validate actual Windows, macOS, and Linux rendering before freezing visual snapshots.

### 2.3 Ark branding and logo

`ConversationSidebar.tsx` renders plain text `Ark`. There is no frontend brand asset/component. Tauri package icons exist, but `src-tauri/icons/icon.png` is a blue circle placeholder and is not an approved reusable Ark mark. COR-012 already records approved source artwork as the branding dependency.

**Likely root cause:** packaging required icon-shaped files before approved brand artwork existed, while frontend navigation was implemented with text only.

**Related architecture:** COR-012, OPS-002, UX-009, `src-tauri/icons`, `ConversationSidebar.tsx`.

### 2.4 Application loading/startup

`ShellState.booting` is real bootstrap state. `useArkController.bootstrap()` waits for `getAppBootstrap` and managed-runtime status, then renders cached data before asynchronous provider refresh. Total bootstrap errors already take priority through `BootstrapFailurePanel`, and workspace errors use the COR-010 recovery banner. The visible loading treatments are currently generic text/spinner states (`MainViewFallback` and Chat's `Loading conversation`), not a coordinated Ark startup experience.

**Likely root cause:** correctness/recovery work landed before brand assets and a startup state projection. A native second splash window is neither present nor required.

**Related architecture:** COR-010, PERF-002, UX-004, `App.tsx`, `useArkController.ts`, `arkStores.ts`.

### 2.5 Thinking/generation activity

Ark Chat uses several independent `Loader2 animate-spin` instances for sending, loading, branches, notes, and actions. Ark Code renders streaming text and a plain `Ark Code is {state}…` line. `StatePanel` also owns its own spinner. The durable states are already typed, but their visual projection is fragmented and does not clearly distinguish preparing, waiting for provider, generating, using a tool, or awaiting the user.

**Likely root cause:** each workflow added a local busy state instead of consuming one shared activity presentation contract.

**Related architecture:** FND-002, CODE-002, UX-004/006/008/009, `ChatView.tsx`, `ChatMessageList.tsx`, `CodeView.tsx`, `statePanel.tsx`.

### 2.6 Dark-theme blue tint

Dark semantic surfaces use hue 220 and primary/ring use hue 200. The main background, cards, muted areas, popovers, borders, and accent therefore read navy even where colour has no semantic meaning.

**Likely root cause:** all dark surfaces were derived from one cool palette. This is a token issue, not an individual component defect.

**Related architecture:** UX-008/009, `src/styles.css`, `tailwind.config.ts`, `check-design-tokens.mjs`.

### 2.7 Model marketplace and model management

Ark currently has two real but visually different model sources:

- connected Ollama installed models come from `/api/tags` and are enriched through `/api/show`; Settings shows them as dense rows with metadata and delete actions;
- the download surface is a combobox over `SUGGESTED_OLLAMA_MODELS`, a bundled 14-entry offline list with approximate sizes and prose descriptions;
- the managed llama.cpp catalog is a reviewed, hash-pinned `config/model-catalog.json`, but it is presented inside the built-in provider form rather than the Models section.

Pull progress, cancellation, malformed-stream handling, refresh, delete confirmation, disk warnings, managed download/resume/cancel/load/unload/delete, and stale-provider handling are already implemented. There is no common `ModelCard`, no unified `Your Models` view across capable providers, and no card-based curated library.

Metadata provenance is implicit. Ollama JSON is stored in opaque `metadataJson`; curated descriptions/sizes have no per-entry source/review fields; managed-catalog fields are reviewed and pinned. The UI does not consistently label `Provided by Ollama`, `Reviewed by Ark`, `Derived by Ark`, or `Unavailable`.

**Likely root cause:** FTR-006 was implemented provider/lifecycle-first, while the addendum's curated picker optimized a narrow pull workflow rather than a marketplace information architecture.

**Related architecture:** ARC-003, FTR-006/009, PERF-004, Settings `OllamaModelsPanel`/`BuiltInRuntimeForm`, `ollamaSuggestedModels.ts`, `model-catalog.json`.

### 2.8 Hardware suitability

`managed_models.rs` has a production preflight with `safe | warning | blocked`, actual free disk and available system RAM, catalog minimum/recommended RAM, acknowledgement, and a justified advanced override. It does not detect GPU VRAM, usable accelerator backend, KV-cache/context memory, GPU-offload amount, or execution-device identity. Diagnostics intentionally reports GPU as unknown. Ollama may run on loopback, LAN, or a remote machine, so scoring it against the desktop's hardware can be wrong.

No 1–10 score currently exists, which is correct. The current evidence can support categorical RAM/disk guidance for a reviewed managed model, but not a defensible numeric score or an Ollama fit claim on a non-loopback route.

**Likely root cause:** FTR-006 completed the safe minimum while PERF-004's broader resource-governance model remains open.

### 2.9 New Chat

The current system is durable-on-create, not ephemeral:

- bootstrap creates and persists a conversation when the database has none;
- every New Chat button/shortcut calls `client.createConversation()`;
- Rust inserts a fresh UUID row titled `New conversation`;
- the controller prepends and selects it, clears the transcript, and focuses the composer.

There is no guard for repeated requests, unsent composer content, or an active generation. The code does not intentionally no-op on an empty chat, so the observed no-op requires a regression reproduction; duplicate titles and rapid list/state updates can make distinct empty rows appear identical. Creation errors leave the current conversation intact through the global error surface.

**Likely root cause:** the lifecycle has never been specified as a product contract, and no component/controller test covers rapid repeated activation or draft/generation cases.

**Related architecture:** COR-004, FTR-002, UX-004/007, `useArkController.createConversation`, `Database::create_conversation`, `ConversationSidebar`.

### 2.10 Sidebar/navigation

The sidebar is one flat virtualized conversation list with a large New Chat button, permanently expanded search input, Show archived checkbox, title plus date/snippet, hover pin/archive actions, and footer entries for Ark Code, Keyboard shortcuts, and Settings.

- Pin persistence exists (`pinned_at`, command/client/controller/UI), but pinned conversations are only sorted ahead within already fetched pages; no dedicated Pinned section or globally correct pinned query exists.
- Projects are fully implemented and bootstrapped. Conversation queries already accept a project filter, but the sidebar does not expose Projects or project-filter state.
- Keyboard shortcuts already have an authoritative Settings section and shared `SHORTCUTS` registry; the sidebar link is now redundant.
- Search focus can be requested by shortcut, but search cannot collapse, has no Escape restoration contract, and consumes permanent width.
- Dates are shown under every title unless replaced by a search snippet.
- Section-collapse state does not exist. Only whole-sidebar/right-panel collapse persists in localStorage.

**Likely root cause:** FTR-002/003 implemented domain behavior after the original flat sidebar, and the navigation was never recomposed around those entities.

### 2.11 Ark Chat/Ark Code mode switch

The finding's statement that Ark Code is not implemented is stale. Ark Code is a first-class `ActiveView` with durable sessions/runs, provider-driven agent execution, inline approvals/diffs/commands, follow-ups, streaming text, cancellation/recovery, and secondary panes. The sidebar exposes Ark Code as a footer button; CodeView exposes a Back to Ark Chat button. State is preserved in `CodeState` when switching.

Disabling Ark Code would regress implemented work and contradict the approved Phase 6.5 interaction contract. The navigation should present a live, accessible Ark Chat/Ark Code sibling switch. If product wants to hide Ark Code from a particular release, that must be a release-capability gate, not a hardcoded false UI.

**Related architecture:** CODE-001–008, especially CODE-007; FND-001; UX-001/002; `ActiveView`, `ConversationSidebar`, `CodeView`.

### 2.12 Console window on launch

`src-tauri/src/main.rs` lacks the standard release-only Windows GUI subsystem attribute. The managed llama.cpp process uses `CREATE_NEW_PROCESS_GROUP` but not a no-window flag. Ark Code invokes direct argument-vector Git/verification processes with captured output. `dev.bat` intentionally opens a development console and should remain diagnostic. Structured app/runtime logs and diagnostics bundles already exist, so console suppression need not discard output.

**Likely root cause:** packaging/process supervision focused on correctness, cancellation, and capture without an explicit per-platform window-visibility policy.

**Related architecture:** ARC-010, OPS-001/002, CODE-005/008, `main.rs`, `sidecar.rs`, `code_*_tools.rs`, `dev.bat`.

### 2.13 Web search provider

The current web-search implementation is Brave-specific end to end:

- `web_search.rs` hardcodes Brave's endpoint, request header, response schema, provider name, errors, and preview;
- the tool secret and `web_search` capability/audit path are generic enough to reuse, but no `WebSearchProvider` port/registry exists;
- generation provenance still names Brave in its source text;
- search remains explicit, off by default, approved, bounded to six citations, and untrusted-context isolated.

Current first-party evidence (checked 2026-08-17):

- [Brave Search API](https://brave.com/search/api/) lists Search at $5/1,000 requests with $5 monthly credit (approximately 1,000 Search requests if pricing/credits remain unchanged). Its [API privacy notice](https://api-dashboard.search.brave.com/privacy-policy) says search query records may be retained for up to 90 days and zero-data-retention is an enterprise option. Payment information is required for plans.
- [Zenserp pricing](https://zenserp.com/pricing-plans/) lists 50 free searches/month; paid service begins at $49.99/month for 25,000 searches. Its API supports header authentication and structured SERP results, but it is a scraper/aggregator rather than an independent index. Zenserp's footer links to a broad [Idera privacy notice](https://www.ideracorp.com/en/legal/privacypolicy), which does not provide a Zenserp-specific search-query retention ceiling.

Zenserp is therefore not currently a stronger free-tier or privacy choice. Neither service is offline/local. Brave should remain the provisional supported backend, with truthful cost/retention disclosure, until the investigation task is approved. A provider abstraction is warranted only if a second backend or user-selectable endpoint is approved.

**Related architecture:** CMP-003/004, SEC-005/009/011, `web_search.rs`, `tools.rs`, `generation.rs`, Tools settings.

### 2.14 Composer model selector

Ark Chat's combined provider/model dropdown and privacy icon are in the header. The composer contains attachments, web-search toggle, send/stop, and route disclosure for remote sends, but no model control. The selection is local ChatView state and is passed to every send/edit/regenerate request. Ark Code already places provider/model controls under its composer.

**Likely root cause:** UX-002 optimized header width before the later composer-direction decision.

**Recommendation:** move the single combined provider/model selector into the Chat composer action row. Retain a compact, non-interactive route/privacy status beside the conversation title (local/LAN/cloud, provider name, health) so route awareness is not conditional on opening the composer menu. Do not retain a second header selector.

### 2.15 Settings density, accent preview, and Tools

Settings has eight responsive tabs and is structurally improved, but panels contain long implementation explanations. AI & Behavior combines application instructions, Projects, Ark Code command allowlist, and Personas. Privacy & Security contains the real `ToolsPanel` plus Companion API. Therefore Tools functionality exists; only the top-level Tools information-architecture entry is missing.

Appearance currently offers only Dark/Light. Device settings are a Rust-owned JSON file with typed full-object updates; an accent preference can extend that authority with serde defaults and contract checks. Arbitrary colours would make the existing contrast guarantees untestable.

**Likely root cause:** features were placed into the closest existing section as they landed, and explanatory implementation copy accumulated during security/correctness work.

**Recommendation:** add a first-class Tools section containing only real Notes/web-search capability grants, web-search credential configuration, audit integrity, and Ark Code verification-command allowlist. Keep Companion API in Privacy & Security. Add a fixed-palette, internal accent preview—not a free-form picker—behind one removable release-capability gate.

### 2.16 Marketplace versus selector

The code does not share the same component between Settings and Chat, which is good. The conceptual boundary should become explicit: Settings owns discovery, comparison, installation, lifecycle, provenance, and hardware guidance; Chat/Code composers list only available compatible models and route status. Marketplace card data must not bloat the composer menu.

### 2.17 Accessibility, responsive behavior, and testing

Ark already has responsive breakpoints, drawers, focus utilities, reduced-motion handling, live regions, and virtualized history. However, the new requested patterns require new semantics and regression coverage. The current frontend tests are DOM-free Node tests; there is no React Testing Library, jsdom, axe, Playwright, or visual-snapshot harness. This is the genuine TST-004 blocker and must be resolved before claiming the requested UI acceptance criteria.

All new surfaces must work at the supported 390×844, 768×1024, 980×720, 1280×720, and large-desktop viewports, 200% zoom, both themes/palettes, keyboard-only operation, and reduced motion. Card grids/carousels must use internal scrolling or responsive wrapping without page-level horizontal overflow.

## 3. Design Decisions

1. **Use native platform typography with an OFL fallback.** Do not bundle Apple or Microsoft proprietary fonts. Bundle Inter Variable only as the Linux/missing-font fallback and record its licence; use the explicit system monospace stack for code.
2. **Neutralize surfaces, not the accent.** Dark background/card/popover/muted/border tokens become near-neutral greys. Brand/accent, focus, warning, success, and destructive tokens retain controlled colour.
3. **One brand source.** Product supplies/approves one SVG/vector Ark mark. `ArkBrand` owns mark/wordmark/compact/theme/a11y behavior. Tauri icon derivatives are generated from the approved source, not copied back into feature folders.
4. **Use an in-webview actual-state startup surface.** It appears immediately while bootstrap is genuinely pending, has no minimum duration, and yields immediately to COR-010 errors. Do not add a second native splash window unless measured startup evidence later requires it.
5. **One activity language.** A shared `ActivityIndicator` maps durable Chat and Code states to concise public labels and an optional subtle visual pulse/dots treatment. It never exposes chain-of-thought.
6. **Keep durable-on-create New Chat for this fix.** The present architecture requires a real conversation identity across settings, attachments, branches, projects, and generation. Repeated deliberate New Chat actions must create/select distinct durable rows. An ephemeral draft would require a new atomic create-and-send use case and is not introduced as a UI patch.
7. **Use the live Ark Chat/Ark Code switch.** Ark Code is implemented. Restyle the navigation into an accessible brand/mode switch and preserve both surfaces' state. A release-capability flag may hide/disable Code only if product explicitly changes its release status.
8. **Sidebar model:** Pinned is a dedicated, bounded global query; Projects is the small existing project list and selects a Chats filter; Chats is the paginated list for All chats or the selected project. Pinned items are not duplicated in Chats. Section rows own their compact create action. Search is global and labels project context in results.
9. **Do not show a hardware score yet.** V1 shows evidence-backed categories and reasons. Numeric 1–10 remains gated until Ark can reliably identify execution device, usable backend, VRAM/offload, weights, KV-cache/context demand, and calibrated performance evidence.
10. **Treat metadata provenance as data, not copy.** Model-display fields carry source kind, source URL/version where applicable, review timestamp, confidence/availability, and derived-method version. UI never silently promotes approximate curated metadata to provider fact.
11. **Curated Ollama library, honestly named.** Keep offline discovery and free-form tags. Cards come from a reviewed manifest; Ark does not scrape Ollama's website at runtime or imply it is a complete live registry.
12. **Move one combined provider/model picker to the composer.** Header retains compact route/provider health disclosure only. Generation requests continue using the selected provider/model through the existing typed path.
13. **Fixed internal accent palettes.** Use a small audited enum (for example Blue, Violet, Teal, Amber, Graphite), immediate semantic-token application, device persistence, and a single release-capability gate. No arbitrary hex input.
14. **Console-free release, observable processes.** Release GUI and child processes are hidden by default on Windows; stdout/stderr remain piped to bounded redacted diagnostics. Development commands remain visible. Add a developer toggle only if child-window visibility is technically useful after reproducing the source.
15. **Keep Brave provisionally.** Do not switch to or add Zenserp now. Complete a documented provider evaluation and privacy/product approval first. Introduce `WebSearchProvider` only when there is a real second implementation/selection need; keep tool grants/audit/secrets generic meanwhile.
16. **Adopt a real UI test stack.** Use Vitest + React Testing Library + jsdom + axe-core for component semantics/state; Playwright Chromium for viewport/keyboard/visual browser flows; retain packaged Tauri E2E as TST-005's separate release layer. Existing pure Node tests need not be migrated merely to satisfy this work.

## 4. Detailed Tasks

### FIX-1708-001 — Establish the UI component-test, accessibility, and visual harness

- **Description:** Resolve TST-004's blocker before broad UI changes by adding the minimum maintained DOM/component and browser-regression infrastructure.
- **Current problem:** current `test:frontend` executes pure modules only; requested focus, aria, reduced-motion, card layout, and visual behavior cannot be proven.
- **User impact:** regressions currently survive until manual testing.
- **Proposed solution:** add Vitest, React Testing Library, jsdom, and axe-core for component tests with `createFakeArkClient`; add Playwright Chromium projects for the declared viewport/theme/reduced-motion matrix and deterministic fixtures. Keep native installed-app E2E separate.
- **Affected files/areas:** `package.json`, lockfile, Vite/Vitest config, CI, test setup/helpers, development fixture client, visual baseline policy.
- **Dependencies:** product/tooling approval; ARC-002 fake client already complete.
- **Relation to master roadmap:** directly continues TST-004; supports UX-001–011 and CODE-007 rather than duplicating them.
- **Implementation notes:** pin browser/runtime versions; mask nondeterministic times/IDs; use semantic assertions before snapshots; define snapshot update review rules.
- **Security/privacy:** fixtures contain synthetic data only; CI artifacts must not contain real workspaces, prompts, paths, or secrets.
- **Accessibility:** axe is a PR gate; keyboard/focus behavior is asserted in rendered components; manual NVDA/VoiceOver remains an RC check.
- **Performance:** keep component suites bounded and shard visual tests only if measured runtime requires it.
- **Testing requirements:** negative-test the CI gate with a deliberate serious axe issue and snapshot drift; verify reduced-motion emulation and each viewport.
- **Acceptance criteria:** component tests render with a fake ArkClient; serious/critical axe findings fail CI; Playwright captures stable light/dark viewport baselines; focus/keyboard tests run in CI; documentation identifies what remains manual/native.
- **Risks:** flaky screenshots and duplicate test runners; mitigate with locked fonts/browser, deterministic fixtures, and narrow snapshot scope.

### FIX-1708-002 — Formalize typography, neutral surfaces, elevation, and shared visual tokens

- **Description:** implement the agreed typography stack/scale and remove blue tint through semantic tokens; add only the card/elevation primitives needed by later tasks.
- **Current problem:** Inter is named but unavailable, code inherits sans-serif, type hierarchy is ad hoc, and dark surfaces are hue-220/navy.
- **User impact:** Ark feels inconsistent, blue-tinted, and less polished across platforms.
- **Proposed solution:** add the licensed Inter Variable fallback and notice; introduce `--font-ui`, `--font-code`, named type/line-height/elevation/surface tokens; convert dark neutral surfaces to near-zero saturation while preserving semantic colour; add shared `Card`/surface variants required by Models and navigation.
- **Affected files/areas:** `src/styles.css`, `tailwind.config.ts`, `src/ui`, font assets/licence notices, design-token checker, Markdown/code styles.
- **Dependencies:** FIX-1708-001; font licence verification.
- **Relation to master roadmap:** expands UX-008/009 and supports PERF-005; no new design framework.
- **Implementation notes:** migrate incrementally; avoid app-wide unrelated spacing changes; test real glyph coverage/metrics for Latin, CJK, Arabic/RTL, emoji, combining marks.
- **Security/privacy:** local packaged font only; no remote font CDN or tracking request.
- **Accessibility:** maintain 4.5:1 normal-text contrast, visible focus, readable line length, 200% zoom, and user text scaling.
- **Performance:** subset/WOFF2 only if licence permits; record compressed size and first-paint impact; no layout-blocking remote load.
- **Testing requirements:** token unit/static tests, contrast matrix, component/visual snapshots, Windows/macOS/Linux manual rendering, production bundle-size comparison.
- **Acceptance criteria:** UI and code use explicit stacks; every named type tier has documented size/weight/line-height; dark background/card/popover/muted/border are visually neutral; no component hardcodes replacement surfaces; contrast and viewport gates pass.
- **Risks:** font metric changes can alter wrapping and snapshots; neutral borders can lose hierarchy if elevations are not tuned together.

### FIX-1708-003 — Create the approved Ark brand asset and `ArkBrand` primitive

- **Description:** replace text-only/placeholder branding with one reusable brand component and generated platform derivatives.
- **Current problem:** frontend has no mark asset; package icon is an unapproved blue circle placeholder.
- **User impact:** the product lacks a recognizable, consistent identity.
- **Proposed solution:** after artwork approval, store a canonical vector source under `src/assets/brand` (or a documented repository-level brand source), implement `ArkBrand` variants for wordmark/compact/icon-only, and generate Tauri raster/ICO/ICNS derivatives through a documented script.
- **Affected files/areas:** brand assets, `src/components/ArkBrand.tsx`, sidebar/mode switch, startup surface, `src-tauri/icons`, packaging docs.
- **Dependencies:** approved artwork (genuine external dependency), FIX-1708-002.
- **Relation to master roadmap:** completes the artwork portion of COR-012 and extends UX-009/OPS-002.
- **Implementation notes:** preferred navigation mark 20–24 px, 8 px mark/word gap, icon-only 24 px in rail; use one-colour/currentColor SVG when possible with approved light/dark variants.
- **Security/privacy:** sanitize/inspect SVG; no external asset URL or embedded script/font.
- **Accessibility:** decorative mark is `aria-hidden` when adjacent to visible Ark text; icon-only mode has `aria-label="Ark"`; minimum contrast is tested.
- **Performance:** inline/component-owned SVG or locally cached asset; no repeated large base64 copies.
- **Testing requirements:** component/axe tests for variants, visual snapshots in all themes/sidebar widths, package icon generation checks, clean bundle verification.
- **Acceptance criteria:** one canonical source owns every mark; wordmark and compact navigation render correctly; no placeholder circle remains in release assets; generated icons pass COR-012 packaging checks.
- **Risks:** implementation is blocked until product supplies/approves artwork; automated derivatives may need platform-specific safe-area adjustments.

### FIX-1708-004 — Add an actual-state Ark startup experience

- **Description:** project real bootstrap state into a minimal branded startup surface without hiding recovery.
- **Current problem:** users see generic loading text/spinners and cannot tell whether Ark is starting normally.
- **User impact:** launch feels delayed or unfinished.
- **Proposed solution:** render `StartupView` immediately while `shell.booting` and authoritative data is unavailable; show `ArkBrand`, one concise state label, and a subtle shared activity treatment. Remove it on the same render that bootstrap becomes usable. `bootstrapError` and workspace recovery pre-empt it.
- **Affected files/areas:** `App.tsx`, shell state/controller, brand/activity primitives, development fixtures.
- **Dependencies:** FIX-1708-002/003/005; COR-010 behavior remains authoritative.
- **Relation to master roadmap:** UX-004/011, PERF-002, COR-010.
- **Implementation notes:** no artificial minimum time; do not wait for background provider refresh; lazy-route fallback uses the same visual language but remains semantically distinct.
- **Security/privacy:** never show workspace paths, error internals, or provider secrets on splash.
- **Accessibility:** `role=status` with a stable concise label; reduced motion uses static state; recovery uses alert/page semantics instead.
- **Performance:** measure first paint and ready removal; startup component must be in the initial chunk and avoid heavy images/animation.
- **Testing requirements:** component tests for immediate ready, delayed bootstrap, total failure, workspace recovery, reduced motion; Playwright startup fixture; cached-shell budget regression.
- **Acceptance criteria:** no delay after readiness; provider refresh does not hold the splash; every COR-010 failure remains visible/actionable; no layout flash between splash and shell; reduced-motion path has no transform animation.
- **Risks:** using managed-runtime/provider refresh as a readiness gate would regress PERF-002; avoid native splash/window synchronization in V1.

### FIX-1708-005 — Consolidate Chat and Code activity indicators

- **Description:** create one typed, accessible activity presentation for preparation, provider wait, generation, tool execution, and user wait.
- **Current problem:** local spinners and plain state strings differ by feature and overuse rotation.
- **User impact:** users cannot reliably understand whether Ark is working, waiting, or needs input.
- **Proposed solution:** add a shared `ActivityIndicator` plus pure state-to-public-label mapping. Replace only generation/agent lifecycle indicators; retain conventional spinners for short button-local operations where appropriate. Use fixed-height dots/pulse or equivalent subtle treatment, with a static reduced-motion variant.
- **Affected files/areas:** `src/ui`, `ChatView`, `ChatMessageList`, `CodeView`, `StatePanel`, lifecycle mapping tests.
- **Dependencies:** FIX-1708-001/002.
- **Relation to master roadmap:** UX-004/006/008/009, FND-002, CODE-002/007.
- **Implementation notes:** labels are versioned product strings; never expose private reasoning. Suggested states: Preparing, Waiting for provider, Generating, Using {tool}, Waiting for approval/clarification, Cancelling.
- **Security/privacy:** tool labels come from trusted tool definitions, never model-provided text.
- **Accessibility:** throttled polite live region announces transitions once; meaning does not rely on colour/motion; long states remain readable.
- **Performance:** no per-token animation/render; stable dimensions prevent layout shift.
- **Testing requirements:** pure mapping tests, component/axe tests, hostile tool-text test, reduced-motion/visual snapshots, long-generation manual soak.
- **Acceptance criteria:** one state mapping is used by Chat and Code; no duplicate “thinking” task remains; waiting-for-user and active work are visually/semantically distinct; activity causes no transcript shift or per-token work.
- **Risks:** too many labels create noise; keep transition announcements coarse and causal.

### FIX-1708-006 — Specify and fix the durable New Chat lifecycle

- **Description:** make New Chat deterministic across click, shortcut, draft, active generation, and failure cases.
- **Current problem:** manual testing reports a repeated empty-chat no-op even though the controller issues durable creates; no acceptance tests define the cases.
- **User impact:** users cannot trust that New Chat created/switched context and may lose a draft.
- **Proposed solution:** first add a failing reproduction at controller/component and real-database boundaries. Preserve durable-on-create semantics: each deliberate activation creates a unique row and selects it. If the current composer has unsent text, show an owned discard/cancel dialog; cancellation preserves draft/focus. An active generation continues in its original conversation and is not implicitly cancelled. A create failure leaves selection, transcript, generation, and draft unchanged.
- **Affected files/areas:** `useArkController`, Chat composer draft ownership, sidebar, shortcuts, ArkClient/database tests, shared Dialog primitive.
- **Dependencies:** FIX-1708-001; COR-004/FND-002 behavior.
- **Relation to master roadmap:** COR-004, UX-004/007/009, FTR-002.
- **Implementation notes:** guard against accidental same-event double dispatch while allowing two completed deliberate activations; use returned UUID identity, not title/timestamp, to verify selection.
- **Security/privacy:** no special impact; do not log draft content in confirmation or errors.
- **Accessibility:** confirmation traps/restores focus; shortcut and pointer paths are identical; successful create focuses composer.
- **Performance:** one bounded DB write per confirmed action; no full-history reload.
- **Testing requirements:** existing conversation, empty conversation, two sequential activations, shortcut, unsent-draft confirm/cancel, active generation, DB failure/rollback, rapid activation ordering, focus restoration.
- **Acceptance criteria:** every confirmed activation returns/selects a distinct ID; cancelled draft discard changes nothing; active generation remains bound to its original conversation; failed creation produces no phantom row or lost draft; tests reproduce and close the manual defect.
- **Risks:** durable empty rows can accumulate; this is an explicit existing product model. Any later ephemeral redesign requires an atomic create-and-send use case and separate plan update.

### FIX-1708-007 — Rebuild the sidebar as one Ark navigation system

- **Description:** implement the brand/mode switch, Pinned/Projects/Chats hierarchy, compact search, density changes, and shortcut cleanup as one responsive feature.
- **Current problem:** flat history does not expose existing Projects/pins well, Code is a footer action, search is permanently large, dates increase density, and shortcuts are duplicated.
- **User impact:** important organization/mode capabilities are hard to discover and the sidebar feels busy.
- **Proposed solution:**
  - header uses `ArkBrand` and an accessible Ark Chat/Ark Code switch; both are enabled in the current build;
  - add independently collapsible Pinned, Projects, and Chats sections with persisted device-local UI state;
  - add a bounded backend pinned query so global ordering is correct across pagination; exclude pinned rows from Chats;
  - Projects lists real FTR-003 entities; selecting one filters the paginated Chats query, and an All chats item clears it;
  - section-header `+` opens real Project creation or New Chat; no fake controls;
  - replace permanent search with an icon-triggered fixed-slot expansion; shortcut opens/focuses it; Escape restores focus without clearing a non-empty query;
  - remove the sidebar shortcuts button; Settings remains authoritative;
  - remove default date text; expose updated time in accessible title/details and show content snippets only for search.
- **Affected files/areas:** `ConversationSidebar`, `App`, controller/catalog/shell stores, ArkClient DTOs, DB query/index methods, project creation flow, settings catalog/localStorage keys, brand/search/collapsible primitives.
- **Dependencies:** FIX-1708-001/002/003/006; FTR-002/003 and ARC-007 are already implemented.
- **Relation to master roadmap:** expands UX-001/002/007/009, FTR-002/003, CODE-007; fixes FTR-002's within-page pin limitation.
- **Implementation notes:** persist only presentation/filter state, not duplicate project/pin truth. Search remains server-side and global; result rows may show a compact project badge. Use virtualization per list and bounded queries.
- **Security/privacy:** search stays local SQLite; no query content in logs/telemetry.
- **Accessibility:** section buttons use `aria-expanded`/`aria-controls`; mode switch announces selection/unavailability reason if capability-gated; project/pin actions have contextual names; search focus/Escape are deterministic.
- **Performance:** no eager load of every project's conversations; dedicated pinned query is bounded; no layout jump during search animation; respect reduced motion.
- **Testing requirements:** DB/contract tests for global pins/project filters; component tests for order/dedup/collapse/persistence/create actions/search focus/Escape/shortcut/date hiding/mode-state preservation; axe/keyboard/viewport/visual tests.
- **Acceptance criteria:** exact visual order is Pinned, Projects, Chats; pinned items are globally correct and not duplicated; real Projects filter Chats; each section persists collapse state; search expands/focuses/collapses correctly; shortcuts entry is absent; dates are absent from default rows but still available; Chat/Code switching preserves each surface.
- **Risks:** nested virtualized lists and project filters can complicate scroll anchoring; keep one active paginated Chats list rather than loading all project children.

### FIX-1708-008 — Introduce a model metadata/provenance presentation contract

- **Description:** normalize display metadata and make source/confidence explicit before building cards.
- **Current problem:** Ollama metadata is opaque JSON, curated entries have undocumented review provenance, and managed catalog metadata has a stronger trust level that UI does not distinguish.
- **User impact:** approximate or unavailable facts can look authoritative, reducing trust.
- **Proposed solution:** define a typed presentation DTO assembled by backend/application logic with optional fields and per-field/source provenance (`provider`, `ark_reviewed`, `ark_derived`, `unavailable`), review/source/version metadata, capabilities, install state, provider/runtime, and supported actions. Move curated suggestions into a validated reviewed manifest with source URL and `reviewedAt`; do not invent missing values.
- **Affected files/areas:** Rust model/provider mapping, curated model manifest/validator, ArkClient/contract schema, `ModelInfo` or a dedicated presentation DTO, Settings model consumers, documentation.
- **Dependencies:** ARC-003/FTR-006; provider-source research for curated entries.
- **Relation to master roadmap:** extends FTR-006/009, UX-011, ARC-002/003.
- **Implementation notes:** keep provider protocol payloads in adapters; presentation assembly must not introduce provider-name switches outside adapter/registry boundaries. “Approximate size” remains visibly approximate.
- **Security/privacy:** remote/provider metadata is untrusted and bounded; no raw HTML; source links use controlled external-link handling.
- **Accessibility:** provenance labels are text, not colour-only; missing data is announced as unavailable rather than omitted ambiguously.
- **Performance:** cache normalized metadata with provider refresh; bound response/string sizes; do not issue per-card network calls.
- **Testing requirements:** Rust mapping/bounds tests, manifest validation, contract drift tests, malformed/partial Ollama fixtures, component rendering for every provenance/missing state.
- **Acceptance criteria:** every displayed fact has a source classification; curated entries have source/review data; malformed metadata degrades per field; provider adapters remain isolated; cards can consume one typed view without parsing `metadataJson` themselves.
- **Risks:** expanding the shared DTO can cause migration churn; prefer a derived API type over persisting duplicate metadata when possible.

### FIX-1708-009 — Build evidence-based hardware-fit guidance

- **Description:** extend existing preflight into a versioned fit assessment without false numeric precision.
- **Current problem:** current managed-model RAM/disk preflight is useful but insufficient for a 1–10 score; Ollama may execute on another machine.
- **User impact:** users can download models that perform poorly, while an invented score would be actively misleading.
- **Proposed solution:** V1 returns categorical `excellent | good | constrained | not_recommended | unknown` plus evidence/reasons/confidence and execution-device scope. Reuse actual RAM/disk and catalog weights. Add requested context/KV-cache estimate only when architecture/runtime data supports it. Show Unknown for non-loopback Ollama or undetected accelerator/VRAM. Add numeric scoring only in a later schema version after cross-platform VRAM/backend/offload detection and benchmark calibration.
- **Affected files/areas:** `managed_models.rs`, diagnostics/hardware DTOs, provider destination classification, model presentation contract, PERF fixtures/docs, Settings cards.
- **Dependencies:** FIX-1708-008; PERF-001/004 methodology; external multi-platform hardware fixtures for a later score.
- **Relation to master roadmap:** directly advances PERF-004 and FTR-006; preserves UX-010's “unknown over guessing.”
- **Implementation notes:** calculations must be pure/versioned and expose inputs; distinguish download fit, load fit, and expected execution mode. Never assume file size equals total runtime memory.
- **Security/privacy:** hardware details remain local unless explicitly included in reviewed diagnostics; no hardware fingerprint sent to model/search providers.
- **Accessibility:** rating includes label and reason; no colour-only gauge; details are progressively disclosed.
- **Performance:** assessment is cached and recomputed on material model/context/hardware change, not every render.
- **Testing requirements:** boundary/property tests for RAM/disk/context estimates; loopback/LAN/remote cases; missing/contradictory metadata; known CPU-only/partial-offload fixtures; UI/a11y tests.
- **Acceptance criteria:** no 1–10 score ships in V1; every category displays evidence and confidence; remote execution yields Unknown unless remote telemetry is explicitly supported; unsafe cases preserve existing acknowledgement/justified override; method/version is documented.
- **Risks:** hardware APIs and unified/shared GPU memory differ by OS; categorical wording must not promise speed without measured throughput.

### FIX-1708-010 — Redesign Settings → Models into Your Models and Curated Ollama Library

- **Description:** create the card-based information architecture while preserving the production lifecycle paths already implemented.
- **Current problem:** installed models are dense rows, library discovery is a dropdown, and managed catalog models are separated under Providers.
- **User impact:** models are difficult to compare and manage, despite robust backend operations.
- **Proposed solution:**
  - `Your Models`: collapsible responsive card rail/grid of installed/available models across configured providers and managed runtime, clearly showing default/selected state, runtime, concise metadata, provenance, fit, and capability-gated select/inspect/load/unload/delete;
  - `Curated Ollama Library`: responsive cards from the reviewed offline manifest, with category/purpose, approximate size, available trustworthy metadata, installed state, fit guidance, and Pull;
  - one details disclosure for deeper architecture/context/licence/source information;
  - reuse existing progress/cancel/retry/refresh/disk-warning paths inline in the affected card and keep the rest of Settings interactive.
- **Affected files/areas:** Settings Models section, new shared `ModelCard`/`ModelGrid`/progress primitives, model DTOs, Ollama/managed operations, fixtures.
- **Dependencies:** FIX-1708-001/002/008/009; FTR-006 lifecycle remains authoritative.
- **Relation to master roadmap:** expands FTR-006, UX-004/009/011, PERF-004; does not create a remote marketplace service.
- **Implementation notes:** horizontally scroll only within a labelled card rail where appropriate; otherwise use responsive CSS grid. Unsupported actions are absent or disabled with reason, driven by capabilities—not provider-name checks.
- **Security/privacy:** downloading remains provider-owned or reviewed-catalog URL-only; keep hash/licence/provenance disclosure and untrusted metadata bounds.
- **Accessibility:** cards have heading/list semantics; actions are reachable in logical order; progress uses `progressbar` values/text; carousel has keyboard controls and does not trap horizontal scrolling.
- **Performance:** virtualize only after profiling; no network request per card; memoize derived presentation; downloads do not block unrelated cards.
- **Testing requirements:** installed/empty/stale/Ollama-unreachable/partial-metadata states; managed and Ollama actions; progress/failure/retry/cancel/completion/refresh/delete/default model; card responsive/zoom/axe/visual tests; provider integration fixtures.
- **Acceptance criteria:** Models renders Your Models before Curated Ollama Library; dropdown is not the primary library; all current lifecycle actions still work; action availability is capability-driven; provenance/fit are visible and honest; no page-level horizontal overflow at declared viewports.
- **Risks:** overloading cards; keep primary facts/actions visible and move provenance/licence/architecture detail behind disclosure.

### FIX-1708-011 — Move the Chat provider/model picker to the composer

- **Description:** relocate the existing combined picker without weakening privacy awareness or request correctness.
- **Current problem:** model selection is remote from Send and occupies header hierarchy.
- **User impact:** selection feels detached from the request and makes the header busier.
- **Proposed solution:** place the one combined provider/model listbox in the composer action row beside Send. Keep a compact header route/provider health badge (not a second selector). Reuse destination-class icons/strings and remote-send disclosure. Ark Code retains its own composer controls under the same shared visual pattern.
- **Affected files/areas:** `ChatView`, provider/model dropdown extraction, composer layout, destination disclosure, responsive tests.
- **Dependencies:** FIX-1708-001/002; model availability state from FTR-009.
- **Relation to master roadmap:** revises UX-002 and supports FTR-004/009/SEC-001.
- **Implementation notes:** extract the existing listbox rather than reimplement it; keep selected model state and send/edit/regenerate payload wiring unchanged.
- **Security/privacy:** route icon derives only from SEC-001 destination classification; remote acknowledgement remains mandatory; model/provider changes cannot spoof local status.
- **Accessibility:** trigger announces current provider/model/route; listbox keyboard semantics/focus restoration pass; compact viewport retains 44 px touch target where touch-first.
- **Performance:** avoid filtering all models repeatedly per token/render; memoize by provider/model collection.
- **Testing requirements:** component interaction and keyboard tests; change then send/edit/regenerate and assert exact request provider/model; unavailable/stale/remote cases; viewport/zoom/visual tests.
- **Acceptance criteria:** only one interactive Chat picker exists; it is adjacent to Send; header still exposes route/provider health; selected model is the one persisted in generation provenance; local/LAN/remote states remain understandable without opening Settings.
- **Risks:** narrow composer rows can crowd attachments/search/stop; use wrapping or an overflow action group without hiding Send/route disclosure.

### FIX-1708-012 — Simplify Settings information architecture and add a real Tools section

- **Description:** reduce normal-path text density and move implemented tool controls into a first-class section.
- **Current problem:** long explanations dominate panels; Tools exists only inside Privacy; Ark Code command allowlist is under AI & Behavior.
- **User impact:** users must read implementation detail to find common controls.
- **Proposed solution:** add `tools` to the settings-section registry; move `ToolsPanel` and `CodeCommandAllowlistPanel` there; show only Notes, Brave web search, grants/revocation/audit, credential state, and enabled Ark Code verification commands. Audit every Settings paragraph as essential, optional help, or developer detail; shorten essential copy and put optional/provenance/implementation detail in accessible disclosures. Preserve security warnings and consequences inline.
- **Affected files/areas:** `settingsSections.ts`, `SettingsView`, settings navigation/store, Tools/command panels, help copy, docs.
- **Dependencies:** FIX-1708-001/002; current CMP-003/004 and CODE-005 implementations.
- **Relation to master roadmap:** UX-009/011, CMP-003/004, CODE-005; no future MCP functionality is invented.
- **Implementation notes:** Companion API remains Privacy & Security; Projects/Personas remain AI & Behavior; advanced diagnostics stay Advanced. Empty/future tools are not listed as functional controls.
- **Security/privacy:** query-retention/cost disclosure, capability scopes, secret-storage status, approval/revocation, and destructive consequences remain explicit.
- **Accessibility:** Settings tabs retain real tab semantics and responsive navigation; disclosures are keyboard/screen-reader operable; heading hierarchy is coherent.
- **Performance:** moving panels must not mount hidden tabs or trigger background network/keychain calls until selected.
- **Testing requirements:** registry/render tests, tab keyboard/focus, deep-link/state persistence, presence/absence of real/future tools, security copy assertions, axe/viewport/visual tests.
- **Acceptance criteria:** Tools is a top-level Settings section; no duplicate tool controls remain; only implemented tools/configuration appear; normal panels are materially shorter while security/privacy consequences remain visible; no hidden tab performs unintended work.
- **Risks:** aggressive copy removal can weaken informed consent; require security-owner copy review.

### FIX-1708-013 — Add a removable, controlled accent-palette preview

- **Description:** enable internal brand-colour evaluation through semantic tokens and typed device persistence.
- **Current problem:** Appearance offers only theme; changing accent requires code edits and can create untested contrast combinations.
- **User impact:** internal testing cannot compare brand directions consistently.
- **Proposed solution:** add a closed `AccentPalette` enum to device settings and contracts, fixed audited palettes, immediate root-token application, and a single release-capability gate controlling the UI. Store the preference in `device_settings.json`; retain a safe default and serde fallback. No arbitrary colour picker.
- **Affected files/areas:** Rust/TS DeviceSettings and contracts, controller/settings store, theme application, semantic CSS tokens, release capabilities/support check, Appearance UI.
- **Dependencies:** FIX-1708-001/002; palette approval.
- **Relation to master roadmap:** ARC-006 settings ownership, UX-008/009, FND-001 release claims.
- **Implementation notes:** palette changes affect primary/accent/ring/selection only, never destructive/warning/success meanings. One flag removes the preview UI later without removing token architecture.
- **Security/privacy:** device-local preference only; no telemetry or external fetch.
- **Accessibility:** every allowed palette passes contrast/focus checks in light/dark; selected palette uses text/pressed semantics, not colour alone.
- **Performance:** apply CSS variables without stylesheet reload; cache initial palette if needed to avoid first-paint flash, following theme's documented authority/cache pattern.
- **Testing requirements:** Rust serialization/default/invalid enum tests, contract checks, semantic-token unit tests, every palette/theme contrast matrix, immediate/persisted/restart behavior, gated-off release test, visual snapshots.
- **Acceptance criteria:** only approved palettes can be saved; all semantic consumers update immediately; preference survives restart; release gate hides the control cleanly; all combinations meet WCAG AA and focus visibility.
- **Risks:** temporary UI may become permanent; assign removal/review date in release capabilities and documentation.

### FIX-1708-014 — Make packaged desktop and child-process startup console-free

- **Description:** identify each visible console source and define release/dev window policy without losing logs or process control.
- **Current problem:** Windows entry point and child launch flags do not guarantee GUI/no-console behavior; manual launch shows a console.
- **User impact:** Ark does not feel like a native desktop app and users may close a required process window.
- **Proposed solution:** reproduce separately for `dev.bat`, unpackaged debug binary, packaged Ark, bundled llama.cpp, Ollama, Git, and verification commands. Add release-only Windows GUI subsystem configuration to Ark. For Ark-owned child processes, apply appropriate Windows no-window creation flags while preserving process groups/job cancellation and piped stdout/stderr. Do not attempt to control external Ollama's own UI. Add a developer visibility setting only if a supervised child can meaningfully expose a separate console; otherwise document `dev.bat`/diagnostics as the developer path.
- **Affected files/areas:** `main.rs`, `sidecar.rs`, Code process helpers, packaging config/docs, possibly DeviceSettings if the toggle is justified.
- **Dependencies:** reproduction matrix; ARC-010/OPS-001 logs already exist.
- **Relation to master roadmap:** COR-012, ARC-010, OPS-001/002, CODE-005/008, TST-005.
- **Implementation notes:** Windows flags must be centralized and tested for compatibility with kill-on-cancel/process-group semantics. Development console remains visible by design.
- **Security/privacy:** never redirect raw output to an insecure world-readable file; retain bounded redaction; developer visibility must not expose secrets.
- **Accessibility:** no normal-user setting is needed if it has no useful effect; any developer toggle has clear label/restart effect.
- **Performance:** flags must not alter startup readiness, process priority, or shutdown timeout.
- **Testing requirements:** Windows release/debug packaged launch smoke, sidecar launch/health/failure/cancel/orphan tests, Ark Code command capture/cancel, diagnostics log assertions, macOS/Linux non-regression.
- **Acceptance criteria:** packaged Windows Ark opens with no console; Ark-owned children open no console by default; all stdout/stderr diagnostics and failure categories remain available; cancellation/orphan cleanup still pass; `dev.bat` remains a visible developer console.
- **Risks:** `CREATE_NO_WINDOW`/process-group combinations can affect control events; prove actual cancellation and cleanup before release.

### FIX-1708-015 — Complete the web-search provider decision and truthful disclosure

- **Description:** make an approved product/privacy/cost decision before replacing Brave or creating a multi-provider system.
- **Current problem:** Brave is hardcoded and its current cost/90-day query-retention implications are not sufficiently visible; Zenserp was suggested without evidence that it improves Ark's constraints.
- **User impact:** users may disclose queries or incur account requirements without adequate understanding; an impulsive switch could worsen quota/privacy.
- **Proposed solution:** keep Brave provisionally. Produce a short ADR/evaluation with dated pricing, free allowance, payment/account requirements, query/IP retention, storage rights, authentication, rate limits, result/citation quality, legal/attribution, failure semantics, and testability for Brave, Zenserp, and at least one credible alternative or user-supplied strategy. Require product/privacy approval. Update Tools/search-preview copy and docs for the approved backend. Only if a second backend/switch is approved, introduce a narrow `WebSearchProvider` port/registry returning Ark's existing bounded `SearchCitation` shape; reuse generic tool grants/secrets/audit.
- **Affected files/areas:** future ADR/docs, `web_search.rs`, `tools.rs`, generation provenance, Tools settings, support/privacy docs, provider fixtures if approved.
- **Dependencies:** product/privacy decision; current CMP-004 security boundary.
- **Relation to master roadmap:** continues CMP-004 and SEC-009/011; a provider port is a subtask only after approval, not speculative architecture.
- **Implementation notes:** current recommendation is **keep Brave**, not support both and not switch to Zenserp. Re-verify pricing/terms at implementation because they are mutable. Treat all search results as untrusted regardless of vendor.
- **Security/privacy:** exact query/destination/retention/cost class disclosed before approval; credentials remain OS-backed; redirects disabled/allowlisted; provider response sizes bounded; no result can authorize tools.
- **Accessibility:** disclosure and errors are concise, focusable, and not colour-only; credential/status forms retain labels.
- **Performance:** preserve connect/request timeouts and six-result cap; compare latency with deterministic fixtures, not live-network CI.
- **Testing requirements:** adapter contract fixtures for success/auth/quota/malformed/oversized/timeout/redirect; secret-boundary and adversarial prompt tests; preview/provenance UI tests; manual account/quota verification outside CI.
- **Acceptance criteria:** ADR records approved choice and evidence date; UI/doc disclosure matches approved terms; no Zenserp code lands without approval; if multiple providers are approved, tool/generation code contains no central vendor-name switch and each adapter passes one contract suite.
- **Risks:** vendor terms/pricing change; SERP scraping legal/quality dependencies; query retention can conflict with user expectations; free quotas are not a product SLA.

## 5. Dependency Order

### Gate A — Product and tooling decisions

1. Approve the UI test stack in FIX-1708-001.
2. Supply/approve brand artwork for FIX-1708-003.
3. Confirm Ark Code remains enabled in the mode switch (recommended) or define an FND-001 release-capability gate.
4. Approve fixed accent palettes and their internal-release visibility.
5. Approve the provisional “keep Brave” disposition and owner for FIX-1708-015.

### Wave 1 — Foundations

1. FIX-1708-001 — test/accessibility/visual harness.
2. FIX-1708-002 — typography, neutral surfaces, elevation/cards.
3. FIX-1708-003 — brand primitive/assets (may proceed in parallel after artwork approval).
4. FIX-1708-008 — model metadata/provenance contract (parallel backend lane).

### Wave 2 — Shared state and behavior

1. FIX-1708-005 — activity indicator, after visual/test foundations.
2. FIX-1708-006 — New Chat lifecycle, independently after the component harness.
3. FIX-1708-009 — hardware-fit categories, after metadata contract.
4. FIX-1708-013 — accent preview, after semantic tokens.
5. FIX-1708-014 — console policy, independent native lane after reproduction.
6. FIX-1708-015 — provider decision/documentation; no adapter implementation before approval.

### Wave 3 — Product surfaces

1. FIX-1708-004 — startup surface after brand/activity foundations.
2. FIX-1708-007 — cohesive sidebar after brand, New Chat, and test foundations.
3. FIX-1708-010 — Models redesign after metadata and hardware fit.
4. FIX-1708-011 — composer picker after visual foundations; can run parallel with Models once shared selector extraction is settled.
5. FIX-1708-012 — Settings/Tools IA after the navigation registry and text hierarchy are available; coordinate with accent and Models placement.

### Release qualification

Run the complete validation matrix below, then update the relevant existing master-task status/acceptance evidence. Do not mark UX-009, TST-004, FTR-006, PERF-004, CODE-007, or COR-012 complete merely because this fixes plan is implemented; each retains broader criteria/external gates in `implementation-plan.md`.

## 6. Validation Matrix

| Layer | Required validation | Applies to |
|---|---|---|
| Static quality | Prettier check, ESLint zero warnings, TypeScript typecheck, design-token check, module-boundary check | All frontend tasks |
| Rust quality | `cargo fmt --check`, strict all-target/all-feature Clippy, Rust unit/integration suite | New Chat DB, pinned query, metadata/fit, settings, console/process, web search |
| Contract/schema | Rust/TypeScript contract fixture; unknown/default enum compatibility; migration/serde fixture where persisted data changes | Model DTOs, DeviceSettings accent, sidebar query DTOs, optional search-provider configuration |
| Frontend unit | Pure state/label/fit/provenance/token/shortcut mappings | 002, 005–013 |
| Component | Fake ArkClient rendering/interactions, focus, semantic roles, state preservation | Startup, sidebar, Models, composer picker, Settings, accent |
| Accessibility | axe serious/critical zero; keyboard-only paths; visible focus; live regions; `aria-expanded`; progress names; 200% zoom | Every UI task |
| Visual regression | light/dark, every approved accent, reduced motion, 390×844, 768×1024, 980×720, 1280×720, large desktop | 002–005, 007, 010–013 |
| Provider/model integration | partial/malformed/stale metadata, Ollama unavailable, progress/cancel/retry/delete, managed model lifecycle | 008–010 |
| Database/application | unique repeated New Chat, rollback, globally ordered pins, project filters, pagination/dedup | 006–007 |
| Security/privacy | external-link/Markdown/CSP/secret-boundary checks; untrusted metadata/search fixtures; route-class integrity; no real user data in artifacts | 003, 008–015 |
| Performance | cached-shell budget, no per-token activity work, sidebar query plans, 1,000-history budget, no per-card network calls, long-generation responsiveness | 004–005, 007, 010–011 |
| Native/process | packaged release/debug launch, hidden console, captured logs, sidecar/command cancel and orphan cleanup on supported OSes | 014 |
| E2E | repeated New Chat/draft; Chat↔Code switch; sidebar sections/search; model download/manage/select/send; Settings Tools/accent/restart | 004, 006–013 |
| Manual product QA | font rendering on Windows/macOS/Linux; NVDA/VoiceOver; actual packaged startup; real Ollama lifecycle; approved palette/brand review; search-provider account/terms check | release candidate |
| Packaging | production Vite build, Tauri bundle on supported matrix, icon/resource/provenance checks, install/uninstall smoke | 002–004, 013–014 |

No task is complete if its focused regression is quarantined, if only a happy-path screenshot exists, or if a packaged-only behavior was verified solely in Vite.

## 7. Deferred / Investigative Items

### 7.1 Product approvals required

1. **Ark Code release visibility:** approve the recommendation to keep the implemented feature enabled in the new switch. Disabling it requires an explicit FND-001 capability/support decision.
2. **Brand artwork:** supply/approve the canonical Ark mark before FIX-1708-003/COR-012 can close.
3. **Test infrastructure:** approve Vitest + React Testing Library + jsdom + axe-core and Playwright as the maintained TST-004 stack.
4. **Accent palettes:** approve the controlled palette set and whether the internal capability is enabled in current packaged test builds.
5. **Search provider:** approve keeping Brave provisionally and the evaluation criteria/owner before any provider replacement or abstraction.

### 7.2 Explicit deferrals

- **Numeric 1–10 device score:** deferred until cross-platform execution-device, VRAM/backend/offload, context/KV-cache, and benchmark calibration evidence exists. V1 categorical guidance is the production recommendation.
- **Complete live Ollama registry:** not available through Ark's current documented API path. Keep a reviewed offline curated library plus free-form tag; investigate a stable official registry API separately if Ollama publishes one suitable for applications.
- **Ephemeral unsaved chat identity:** not part of this fix. It requires a new atomic create-and-send application use case and lifecycle decision, not a frontend-only state.
- **Arbitrary accent picker:** rejected for the internal preview because contrast cannot be guaranteed.
- **Zenserp implementation:** deferred/rejected as the immediate replacement. Its current public evidence does not improve free quota or query-retention clarity.
- **Generic search-provider registry:** deferred until a second provider or user selection is approved. The existing tool capability/audit/secret architecture remains authoritative.
- **Native splash window:** deferred unless measurement shows the in-webview actual-state surface cannot paint soon enough.
- **Always-visible runtime console developer setting:** add only if reproduction proves a supervised child window exists and visibility is useful; otherwise `dev.bat` plus diagnostics is the supported developer path.
- **Ark Code implementation:** out of scope. This plan changes only navigation presentation around the already implemented surface; CODE-007's product approval/component/native E2E gates remain tracked in the master plan.

## 8. Coverage Checklist Against Manual Findings

| Manual finding | Covered by |
|---|---|
| Overall UI quality/design-system ownership | Audit 2.1; FIX-1708-001/002/005 |
| Typography | Audit 2.2; FIX-1708-002 |
| Branding/logo | Audit 2.3; FIX-1708-003 |
| Actual-state loading/splash | Audit 2.4; FIX-1708-004 |
| Consolidated thinking/generation animation | Audit 2.5; FIX-1708-005 |
| Neutral dark background/surfaces | Audit 2.6; FIX-1708-002 |
| Marketplace cards and metadata provenance | Audit 2.7; FIX-1708-008/010 |
| Hardware suitability | Audit 2.8; FIX-1708-009 |
| Repeated New Chat and lifecycle cases | Audit 2.9; FIX-1708-006 |
| Chat/Code switcher | Audit 2.11; FIX-1708-003/007 |
| Pinned/Projects/Chats sections and create actions | Audit 2.10; FIX-1708-007 |
| Remove sidebar shortcuts | Audit 2.10; FIX-1708-007 |
| Expandable search/focus/Escape/persistence | Audit 2.10; FIX-1708-007 |
| Hide dates/default row density | Audit 2.10; FIX-1708-007 |
| Hidden production console with diagnostics | Audit 2.12; FIX-1708-014 |
| Brave cost/Zenserp investigation/provider architecture | Audit 2.13; FIX-1708-015 |
| Composer model selector/privacy | Audit 2.14; FIX-1708-011 |
| Less dense Settings | Audit 2.15; FIX-1708-012 |
| Temporary controlled accent selector | Audit 2.15; FIX-1708-013 |
| Missing top-level Tools section, real tools only | Audit 2.15; FIX-1708-012 |
| Your Models/Ollama Library/download lifecycle | Audit 2.7; FIX-1708-008/009/010 |
| Marketplace vs lightweight selector | Audit 2.16; FIX-1708-010/011 |
| Accessibility/responsive/testing requirements | Audit 2.17; FIX-1708-001 and every task's gates |
