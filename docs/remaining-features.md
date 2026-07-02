# Ark Remaining Features

This document summarizes what is already implemented, what remains for the MVP foundation, and what should stay in later phases. The full source of truth remains `implementation-plan.md`; this file is the shorter execution checklist.

## Current Implementation Snapshot

Implemented foundation:

- Tauri desktop shell with React, TypeScript, Tailwind CSS, shadcn-style local UI components, Framer Motion, and Lucide icons.
- Dark and light theme support.
- Sidebar, chat area, and reserved/collapsible right panel.
- Conversation create, rename, delete, search, history loading, and local persistence.
- SQLite schema and migration runner for MVP tables.
- Append-only message model with edit/regenerate branch preservation.
- Assistant branch alternatives and branch switching for regenerated responses.
- Ollama provider through the Rust provider runtime abstraction.
- Streaming responses with incremental SQLite persistence.
- Stop generation with partial content preservation.
- Provider/model selector in the chat header.
- Provider/settings screen for Ollama.
- Diagnostics benchmark with observed local performance.
- Markdown and code rendering with syntax highlighting.
- Conversation Markdown export.
- Conversation JSON export/import with validation.
- Configurable workspace folder with portable-workspace restart flow.
- Basic keyboard shortcuts for new chat, search, settings, and send.
- Frontend code splitting for chat/settings screens.

## Highest Priority MVP Gaps

### 1. Local Inference Host Mode

Goal: allow users to choose between Ollama and a user-managed local inference host from the same provider/runtime selector.

Required work:

- Seed a second local provider record:
  - `id`: `local_inference_host`
  - `name`: `Local inference host`
  - `provider_type`: `local_inference_host`
  - default base URL: `http://localhost:8080`
- Add a `LlamaCppServerProvider` or `LocalInferenceHostProvider` Rust adapter.
- Extend `ProviderRuntime` with the local inference host variant.
- Implement health/readiness check for the local host.
- Implement model listing through `GET /v1/models` when available.
- Implement streaming chat through `POST /v1/chat/completions`.
- Parse Server-Sent Events into normalized Ark stream chunks.
- Keep local-host-specific parsing out of the UI.
- Add setup guidance explaining that users must start `llama-server` externally with their desired GPU/CPU flags.
- Update diagnostics to benchmark whichever local provider is selected.

Out of scope for this milestone:

- Bundling llama.cpp.
- Downloading model files.
- Starting/stopping `llama-server` from Ark.
- Embedded inference inside Ark.
- Automatic GPU backend selection.

### 2. Multi-Provider Local UI Flow

Goal: make the existing provider/model UI genuinely work with more than one seeded local provider.

Required work:

- Update settings to select which seeded provider is being configured.
- Store and refresh model lists per provider.
- Update chat provider selection so model options are scoped to the selected provider.
- Update setup banners to show provider-specific guidance.
- Ensure conversation provider/model metadata is updated when switching providers.
- Keep all provider-specific labels and instructions in normalized data or provider-specific setup components, not in generic chat logic.

### 3. Manual Local Runtime Validation

Goal: verify the app works in real desktop use, not only through build checks.

Manual test matrix:

- Ollama running with `llama3.2`.
- Ollama unavailable.
- Selected Ollama model missing.
- llama.cpp server running with a GGUF model.
- llama.cpp server unavailable.
- Streaming, stop, retry, edit/regenerate, and branch switching.
- Conversation export/import round trip.
- Diagnostics benchmark for each local provider.
- Dark/light mode.
- Keyboard navigation and visible focus states.
- Offline launch with no internet.

## Important MVP Polish

### Long-History Performance

Current state: the chat UI renders the active message list directly. This is fine for early use but may become slow with large histories.

Recommended work:

- Add incremental loading or virtualization for long conversation histories.
- Avoid rerendering the full message list for every streamed token.
- Consider storing streaming content updates in a small focused state path, then reconciling with persisted messages.

### Deeper Branch Navigation

Current state: regenerated assistant alternatives can be listed and selected, but branch UX is limited when inactive branches have deeper follow-up histories.

Recommended work:

- Represent branch alternatives that have descendants.
- Allow switching to a branch leaf, not only an immediate assistant sibling.
- Make branch controls clear without turning the chat UI into a tree editor.

### Diagnostics Tests

Current state: diagnostics exist and can run a benchmark, but coverage is mostly manual.

Recommended work:

- Unit test diagnostics guidance mapping.
- Unit test benchmark result shaping.
- Add provider integration tests behind an opt-in flag or local test setup.

## Later Phases

### Phase 2 Provider Expansion

- Cloud OpenAI-compatible provider.
- OpenAI provider.
- Azure OpenAI provider.
- OpenRouter-compatible provider.
- Secure API key storage.
- User-created provider records.
- Fallback provider settings.

### Phase 3 Local Knowledge and Documents

- Document import.
- Text extraction.
- Chunking.
- Local embeddings.
- Local vector search.
- Document-grounded chat with citations.

### Phase 4 Memory

- Explicit user-approved memories.
- Memory review screen.
- Per-chat memory controls.
- Memory audit trail.

### Phase 5 Local Tools

- Calculator.
- Read-only local file search.
- Note creation.
- Git inspection.
- Terminal tool with explicit approval.

### Phase 6 Backup and Restore

- Full workspace export archive.
- Full workspace import.
- Backup verification.
- Offline restore documentation.
- Optional model path remapping.

### Phase 7 Advanced Assistant Modes

- Command palette.
- Prompt library.
- Coding/research/document modes.
- Voice and local speech features.

## Current Non-Goals

Do not implement these until their phase is approved:

- RAG/document chat.
- Memory.
- Agents.
- Voice.
- Image generation.
- Web search.
- Terminal execution.
- Cloud sync.
- User accounts.
- Telemetry.
- Bundled model downloads.
- Bundled local runtime installers.
