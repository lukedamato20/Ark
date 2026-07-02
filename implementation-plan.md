# Ark - Multi-Phase Implementation Plan

## 1. Project Vision

Build a local-first personal AI assistant that remains usable even if major cloud AI providers such as ChatGPT, Claude, Gemini, or Azure OpenAI become unavailable, expensive, restricted, or discontinued.

Ark is not just a local chatbot. It is a personal AI infrastructure layer: a private desktop application that can use local and cloud models interchangeably while keeping the user's data under their control.

## 2. Core Goals

The application must be:

- Offline-first
- Provider-agnostic
- Model-configurable
- Privacy-focused by default
- Portable and restorable from backup
- Plug-and-play on supported desktop devices that meet local model requirements
- Long-term maintainable
- Simple enough to run locally without complex infrastructure

## 3. Confirmed Technology Stack

### Product Name

- Ark

### Desktop Application

- Tauri
- React
- TypeScript
- Tailwind CSS
- shadcn/ui
- Framer Motion
- Lucide Icons

Use shadcn/ui as a source-component foundation that Ark owns and can adapt. Do not introduce large UI frameworks such as Material UI, Ant Design, Bootstrap, Chakra UI, or Mantine.

Use Framer Motion sparingly for focused transitions only. It should not become a default wrapper around the application.

### Supported Platforms

- Windows desktop
- macOS desktop
- Linux desktop

Ark should target mainstream desktop devices supported by Tauri and the selected local model runtime. Ark should not promise that every model runs well on every device; local AI performance depends on CPU, RAM, GPU/accelerator support, disk space, runtime support, and selected model size.

### Core/System Layer

- Rust through Tauri commands

### Local Data Storage

- SQLite
- JSON configuration files where appropriate
- Markdown/TXT for exportable plain-text content
- Configurable Ark workspace folder
- Portable workspace mode for backup-drive or synced-folder use
- Rust SQLite access through `rusqlite` with an in-repo migration runner for the MVP

### Local Model Runtime

- Phase 1: Ollama
- Phase 1 local inference host: user-managed llama.cpp `llama-server` or another OpenAI-compatible local host
- Later phase: Ark-managed local runtime process and/or embedded inference runtime

For the MVP, Ark should guide users through installing Ollama, local inference hosts, and models themselves. Ark should not bundle Ollama, llama.cpp, model files, or model installers until a later packaging decision is made.

### Provider Interface

- OpenAI-compatible API where possible
- Provider/runtime abstraction layer to support Ollama, local inference hosts, and later cloud models with the same internal interface
- Rust provider traits are the source of truth for provider behavior
- TypeScript request/response types should mirror or be generated from the Rust command boundary to avoid frontend/backend drift
- Ollama must have a dedicated adapter; do not assume its OpenAI-compatible endpoints behave exactly like OpenAI
- Local inference hosts such as llama.cpp `llama-server` must have their own adapter even when they expose OpenAI-compatible endpoints, because health checks, model listing, streaming frames, and runtime setup guidance differ by host.

### Initial Local Runtime Strategy

Use Ollama first because it is simple to install, easy to run locally, and exposes an API surface suitable for the first provider implementation. Add a second local mode for a user-managed local inference host, initially targeting llama.cpp `llama-server`, behind the same provider/runtime abstraction. Ark should connect to this local host by base URL and should not initially manage the process or bundle model files.

## 4. Product Principles

### 4.1 Offline-First

The application must still function without internet access. Cloud providers, online search, and external tools are optional enhancements, not core dependencies.

### 4.2 Provider-Agnostic

The UI must not care where a response comes from. Ollama, llama.cpp, OpenAI, Azure OpenAI, Anthropic, OpenRouter, or any other provider should be accessed through a common internal interface.

### 4.3 No Vendor Lock-In

Avoid proprietary or opaque formats. Store user-owned content in simple, durable formats:

- SQLite
- JSON
- Markdown
- TXT

### 4.4 Privacy by Default

No chat, document, memory, prompt, or local file content should leave the machine unless the user explicitly enables a cloud provider or online tool.

The Rust core is the privacy policy boundary. The UI must clearly display provider/model choices, but the core must enforce whether a provider may use external network access and must reject cloud or external requests unless the relevant provider is enabled by user configuration.

### 4.5 Local Ownership

The user owns and can export:

- Chats
- Messages
- Documents
- Embeddings
- Prompts
- Memories
- Provider settings
- App configuration
- Backups

### 4.6 Safe Local Tools

Any potentially risky action must require explicit user approval. This includes:

- Terminal access
- File writing
- File deletion
- Git changes
- External network requests
- Reading sensitive local folders

### 4.7 Portability

The application should eventually be restorable from a backup drive containing:

- SQLite database
- Config files
- Prompt library
- Memory store
- Document index
- Local model references
- Exported chats

Ark should use a configurable workspace directory. The default workspace can live in the platform app data directory, but the user must be able to choose a portable workspace folder for backup-drive use. The workspace should contain the SQLite database, config files, exports, and later document indexes. Local model binaries are referenced, not copied into the workspace by default.

### 4.8 Plug-and-Play Desktop Experience

Ark should be usable immediately after installation on supported desktop platforms, even before a local model runtime is configured.

The app should:

- Launch without requiring internet access.
- Detect whether Ollama is installed and reachable.
- Detect whether the selected model is available.
- Keep non-model features usable when local inference is unavailable.
- Show clear setup guidance instead of failing silently.
- Avoid requiring command-line knowledge for normal setup.
- Explain when device performance may limit local model quality or speed.

### 4.9 System Readiness and Performance

Ark should include a local diagnostics and performance test so users can understand what to expect from their device.

The test should prefer observed local performance over fragile hardware assumptions. GPU detection is useful when reliable, but tokens per second, time to first token, streaming behavior, and model availability are more important than perfectly identifying every accelerator.

## 5. High-Level Architecture

```text
┌──────────────────────────────────────┐
│              React UI                 │
│  Chat, settings, model picker, files  │
└──────────────────┬───────────────────┘
                   │
                   ▼
┌──────────────────────────────────────┐
│            Tauri Commands             │
│  Secure bridge between UI and core    │
└──────────────────┬───────────────────┘
                   │
                   ▼
┌──────────────────────────────────────┐
│              Rust Core                │
│  Providers, storage, config, tools    │
└──────────────────┬───────────────────┘
                   │
        ┌──────────┼──────────┐
        ▼          ▼          ▼
   SQLite DB   Config Files   Provider Layer
                              │
                ┌─────────────┼─────────────┐
                ▼             ▼             ▼
             Ollama       llama.cpp       Cloud APIs
```

## 6. Main Modules

### 6.1 UI Module

Responsible for:

- Chat interface
- Conversation sidebar
- Reserved/collapsible right panel
- Model/provider selector
- Settings screen
- Diagnostics/performance screen
- First-launch/setup guidance
- Import/export controls
- Markdown rendering
- Code block rendering
- Syntax highlighting
- Loading states
- Error display
- Offline/online indicator
- Runtime setup guidance
- Dark and light theme support

### 6.2 Core Module

Responsible for:

- Provider orchestration
- Chat request handling
- Streaming response handling
- SQLite persistence
- Config management
- Import/export logic
- System readiness checks
- Local benchmark execution
- Safe tool execution in later phases

### 6.3 Provider Module

Responsible for:

- Normalising requests across different model providers
- Sending chat completions
- Streaming responses
- Handling provider-specific errors
- Managing model options

### 6.4 Storage Module

Responsible for:

- SQLite migrations
- Conversation persistence
- Message persistence
- Provider settings
- Model settings
- Prompt library storage in later phases
- Memory storage in later phases
- Document index storage in later phases

### 6.5 Document Module — Later Phase

Responsible for:

- File ingestion
- Text extraction
- Chunking
- Embeddings
- Local vector search
- Source citation mapping

### 6.6 Tool Module — Later Phase

Responsible for:

- Safe execution of local tools
- Permission prompts
- Audit logs
- Tool sandboxing where practical

## 7. Initial Folder Structure

```text
ark/
├── src/                         # React frontend
│   ├── components/
│   ├── features/
│   │   ├── chat/
│   │   ├── conversations/
│   │   ├── diagnostics/
│   │   ├── onboarding/
│   │   ├── settings/
│   │   └── providers/
│   ├── hooks/
│   ├── lib/
│   ├── ui/
│   ├── types/
│   └── main.tsx
│
├── src-tauri/                   # Tauri/Rust backend
│   ├── src/
│   │   ├── main.rs
│   │   ├── commands/
│   │   ├── config/
│   │   ├── db/
│   │   ├── providers/
│   │   ├── chat/
│   │   ├── workspace/
│   │   ├── export/
│   │   └── errors.rs
│   ├── migrations/
│   ├── Cargo.toml
│   └── tauri.conf.json
│
├── docs/
│   ├── implementation-plan.md
│   ├── architecture.md
│   └── provider-spec.md
│
├── tests/
├── package.json
├── README.md
└── .gitignore
```

## 8. Core Data Model

The initial migration should include only MVP tables. Later-phase tables are documented here for planning, but should not be created until their phase is implemented.

All timestamps should be stored as UTC ISO-8601 strings. SQLite foreign keys must be enabled on every connection.

### 8.1 conversations

Stores chat sessions and the current active message branch.

```sql
CREATE TABLE conversations (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    provider_id TEXT,
    model_id TEXT,
    current_message_id TEXT,
    system_prompt TEXT,
    temperature REAL,
    max_tokens INTEGER,
    streaming_enabled INTEGER NOT NULL DEFAULT 1,
    archived INTEGER NOT NULL DEFAULT 0
);
```

### 8.2 messages

Stores all chat messages as an append-only message tree.

Editing a prior message, regenerating an assistant response, or branching a conversation must not overwrite existing message content. Instead, Ark should create new messages with `parent_message_id` and, when applicable, `revision_of_message_id`, then update `conversations.current_message_id` to point at the selected branch leaf.

```sql
CREATE TABLE messages (
    id TEXT PRIMARY KEY,
    conversation_id TEXT NOT NULL,
    parent_message_id TEXT,
    revision_of_message_id TEXT,
    path_index INTEGER NOT NULL,
    role TEXT NOT NULL CHECK(role IN ('system', 'user', 'assistant', 'tool')),
    content TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'complete' CHECK(status IN ('pending', 'streaming', 'complete', 'failed', 'cancelled')),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    provider_id TEXT,
    model_id TEXT,
    token_count INTEGER,
    error_message TEXT,
    metadata_json TEXT,
    FOREIGN KEY (conversation_id) REFERENCES conversations(id) ON DELETE CASCADE,
    FOREIGN KEY (parent_message_id) REFERENCES messages(id) ON DELETE SET NULL,
    FOREIGN KEY (revision_of_message_id) REFERENCES messages(id) ON DELETE SET NULL
);
```

Recommended MVP indexes:

```sql
CREATE INDEX idx_messages_conversation_path ON messages(conversation_id, path_index);
CREATE INDEX idx_messages_parent ON messages(parent_message_id);
CREATE INDEX idx_messages_revision ON messages(revision_of_message_id);
```

### 8.3 providers

Stores provider configurations.

```sql
CREATE TABLE providers (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    provider_type TEXT NOT NULL,
    base_url TEXT,
    api_key_ref TEXT,
    default_model_id TEXT,
    default_temperature REAL,
    default_max_tokens INTEGER,
    streaming_enabled INTEGER NOT NULL DEFAULT 1,
    is_local INTEGER NOT NULL DEFAULT 1,
    is_enabled INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
```

### 8.4 models

Stores available models.

```sql
CREATE TABLE models (
    id TEXT PRIMARY KEY,
    provider_id TEXT NOT NULL,
    name TEXT NOT NULL,
    display_name TEXT,
    context_window INTEGER,
    supports_streaming INTEGER NOT NULL DEFAULT 1,
    supports_tools INTEGER NOT NULL DEFAULT 0,
    supports_vision INTEGER NOT NULL DEFAULT 0,
    supports_embeddings INTEGER NOT NULL DEFAULT 0,
    is_available INTEGER NOT NULL DEFAULT 1,
    last_seen_at TEXT,
    metadata_json TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (provider_id) REFERENCES providers(id) ON DELETE CASCADE
);
```

Recommended MVP indexes:

```sql
CREATE INDEX idx_models_provider ON models(provider_id);
```

### 8.5 app_settings

Stores general application settings.

```sql
CREATE TABLE app_settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
```

### 8.6 schema_migrations

Tracks applied database migrations if the chosen migration runner does not provide its own metadata table.

```sql
CREATE TABLE schema_migrations (
    version INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    applied_at TEXT NOT NULL
);
```

### 8.7 prompt_templates — Later Phase

```sql
CREATE TABLE prompt_templates (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    content TEXT NOT NULL,
    tags_json TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
```

### 8.8 memories — Later Phase

```sql
CREATE TABLE memories (
    id TEXT PRIMARY KEY,
    content TEXT NOT NULL,
    source TEXT,
    enabled INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
```

### 8.9 documents — Later Phase

```sql
CREATE TABLE documents (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    original_path TEXT,
    imported_path TEXT,
    mime_type TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
```

### 8.10 document_chunks — Later Phase

```sql
CREATE TABLE document_chunks (
    id TEXT PRIMARY KEY,
    document_id TEXT NOT NULL,
    chunk_index INTEGER NOT NULL,
    content TEXT NOT NULL,
    metadata_json TEXT,
    created_at TEXT NOT NULL,
    FOREIGN KEY (document_id) REFERENCES documents(id) ON DELETE CASCADE
);
```

## 9. Provider Interface Design

Provider behavior should be defined in Rust first. TypeScript types are frontend-facing DTOs for Tauri commands and should mirror the Rust command boundary. This does not limit UI/UX design; it only prevents the UI from depending on provider-specific response shapes.

The UI may present rich provider, model, streaming, health, and error states, but it must consume normalized Ark data structures rather than Ollama-specific or OpenAI-specific payloads.

### 9.0 Rust Provider Traits

The provider layer should split chat generation, model discovery, and health checks so providers can implement only the capabilities they actually support.

```rust
pub trait ChatProvider {
    fn provider_id(&self) -> &str;
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse, ProviderError>;
    async fn stream_chat(
        &self,
        request: ChatRequest,
        events: StreamEventSink,
        cancel: CancellationToken,
    ) -> Result<ChatResponse, ProviderError>;
}

pub trait ModelDiscoveryProvider {
    async fn list_models(&self) -> Result<Vec<ModelInfo>, ProviderError>;
}

pub trait ProviderHealthCheck {
    async fn health(&self) -> ProviderHealthStatus;
}
```

Ollama should be implemented as an `OllamaProvider` adapter. A user-managed local inference host should be implemented as a separate local adapter, initially `LlamaCppServerProvider`, even if it uses OpenAI-compatible endpoints. A generic cloud OpenAI-compatible adapter belongs in Phase 2.

The trait snippet is conceptual. During implementation, use an object-safe pattern for the provider registry, such as boxed futures or `async_trait`, only if dynamic dispatch is needed.

### 9.0A Local Runtime Modes

Ark should support two local inference modes behind the same provider/runtime abstraction:

1. Ollama mode.
   - Ark connects to the local Ollama service.
   - Ollama manages model files, loading, CPU/GPU use, and runtime process behavior.
   - Ark provides health checks, model refresh, streaming chat, setup guidance, and benchmark reporting.
2. Local inference host mode.
   - Ark connects to a user-managed local HTTP inference server such as llama.cpp `llama-server`.
   - The user installs the runtime, downloads/selects a GGUF model, and starts the server with desired CPU/GPU flags.
   - Ark stores the provider base URL and selected model metadata, then sends normalized chat requests through the same chat UI.
   - Ark should show setup guidance for starting the local host and should not initially bundle or launch the runtime process.

For the first local inference host implementation, target llama.cpp `llama-server` using OpenAI-compatible endpoints where available:

- Health/readiness check: prefer a lightweight host-specific health endpoint when available, with a fallback to model listing.
- Model listing: `GET /v1/models` when available.
- Chat streaming: `POST /v1/chat/completions` with `stream = true`, parsing Server-Sent Events into normalized Ark stream events.
- Non-streaming response support may be added later, but streaming must remain the primary chat path.

The UI must not contain Ollama-specific or llama.cpp-specific branching. It should display provider name, provider type, local/cloud status, model list, health status, and setup guidance supplied by normalized command results.

Ark-managed local runtime processes, bundled runtime binaries, direct embedded inference, model downloads, and automatic GPU backend selection are not part of the first local inference host milestone.

### 9.1 Internal Chat Request

```ts
export interface ChatRequest {
  providerId: string;
  model: string;
  messages: ChatMessage[];
  temperature?: number;
  maxTokens?: number;
  stream?: boolean;
  systemPrompt?: string;
}
```

### 9.2 Internal Chat Message

```ts
export interface ChatMessage {
  role: 'system' | 'user' | 'assistant' | 'tool';
  content: string;
}
```

### 9.3 Internal Chat Response

```ts
export interface ChatResponse {
  message: ChatMessage;
  providerId: string;
  model: string;
  usage?: {
    inputTokens?: number;
    outputTokens?: number;
    totalTokens?: number;
  };
  metadata?: Record<string, unknown>;
}
```

### 9.4 Provider Capabilities

Each provider should expose metadata:

```ts
export interface ProviderCapabilities {
  supportsStreaming: boolean;
  supportsTools: boolean;
  supportsVision: boolean;
  supportsEmbeddings: boolean;
  isLocal: boolean;
}
```

### 9.5 Provider Settings

Each provider should support:

- Provider name
- Provider type
- Base URL
- API key where required
- Default model
- Temperature
- Max tokens
- Context window
- Streaming toggle
- Enabled/disabled flag

In Phase 1, the settings UI should be structured generically and should support the seeded local provider records for Ollama and local inference host mode. Creating arbitrary additional provider records is deferred to Phase 2, but users must be able to switch between the seeded local modes and configure each mode's base URL and default model.

### 9.6 Streaming Contract

Streaming must be designed as an explicit Tauri event protocol.

Recommended event flow:

1. The UI invokes a typed command such as `send_chat_message`.
2. The Rust core persists the user message.
3. The Rust core creates an assistant message with `status = 'streaming'` and empty content.
4. The provider adapter streams normalized chunks to the core.
5. For each received chunk, the core appends the chunk to the assistant message in SQLite and emits a normalized stream event to the UI.
6. On success, the core marks the assistant message `complete`.
7. On cancellation, the core preserves partial content and marks the assistant message `cancelled`.
8. On failure, the core preserves partial content, stores a concise error, and marks the assistant message `failed`.

Recommended event names:

- `chat:stream-start`
- `chat:stream-delta`
- `chat:stream-complete`
- `chat:stream-error`
- `chat:stream-cancelled`

The UI should render streamed chunks from normalized events and then reconcile against the stored message record. The UI must not parse provider-native stream frames.

### 9.7 Cancellation and Recovery

The MVP must include a stop-generation action. Cancellation should use a Rust-side cancellation token keyed by active request ID or assistant message ID.

Interrupted streams must remain recoverable:

- Partial assistant content stays visible.
- The message status explains whether it failed or was cancelled.
- The user can retry/regenerate from the same parent message.
- Regeneration creates a new assistant branch instead of overwriting the interrupted message.

## 9A. UI/UX Direction

Ark should feel like a modern personal AI workspace, not an enterprise dashboard, admin portal, IDE, or system utility.

The experience should be:

- Fast
- Fluid
- Responsive
- Minimal
- Professional
- Calm
- Powerful
- Privacy-focused
- Offline-first

Ark may take broad inspiration from products such as ChatGPT, Claude, Obsidian, Raycast, and Arc Browser, but it must not directly copy distinctive product patterns, branding, layout, iconography, or interaction details.

### 9A.1 Core Design Philosophy

The interface should communicate:

- Ownership
- Independence
- Reliability
- Simplicity

The user should feel that Ark belongs to them and runs on their machine. The app should remain approachable for both technical and non-technical users.

### 9A.2 Visual Style

Theme support:

- Dark mode and light mode are both required for the MVP.
- Dark mode should be treated as the primary design target.
- Light mode should be complete and usable, not an afterthought.

Overall aesthetic:

- Modern
- Clean
- Premium
- Minimal

Avoid:

- Excessive borders
- Excessive shadows
- Excessive gradients
- Bright accent colors
- Skeuomorphic effects
- Decorative visual effects that do not improve usability

Prefer:

- Soft contrast
- Clear typography
- Spacious layouts
- Subtle visual hierarchy
- Muted borders
- Restrained accent colors
- Consistent spacing and radius tokens

Design tokens should be defined early for color, spacing, typography, border radius, borders, focus states, and surfaces. UI elements should generally use compact, professional radii and avoid overly rounded decorative shapes.

### 9A.3 Layout Structure

MVP layout:

- Left sidebar:
  - New Chat
  - Conversation List
  - Search
  - Settings
- Main area:
  - Chat header
  - Provider selector
  - Model selector
  - Local/cloud/offline status indicators
  - Messages
  - Composer
- Right panel:
  - Present as a visible reserved area
  - Collapsible
  - Empty or minimal in the MVP
  - Reserved for future documents, memory, chat details, and tools

The layout should support future features without requiring a redesign. It should be desktop-first but handle narrower desktop windows gracefully. Mobile app support is out of scope for the MVP.

### 9A.4 Sidebar Behavior

The sidebar is a core part of the experience.

Requirements:

- Smooth collapse and expand
- Persistent width/collapsed state
- Searchable conversation list
- Fast switching between chats
- Clear active conversation state
- Empty/loading/error states for conversation data

The user should always know where they are.

### 9A.5 Chat Experience

The chat screen is the most important screen.

Requirements:

- Streaming responses
- Markdown rendering
- Code block rendering
- Syntax highlighting using a carefully chosen lightweight dependency
- Copy actions
- Message actions
- Stop-generation action
- Regenerate action
- Edit previous user message action
- Clear branch controls when alternate responses exist
- Empty, loading, error, interrupted, cancelled, and offline states

Messages should be easy to scan. Whitespace should improve readability without wasting space. Long messages and long sessions must remain readable and performant.

### 9A.6 Provider and Model Selection

Provider and model selection must be visible in the chat header or an equivalent always-accessible surface.

The user must always know:

- Which provider is active
- Which model is active
- Whether the active provider/model is local or cloud
- Whether Ark is online, offline, or using a local-only runtime
- Whether the selected model is available

This information must not be buried only in settings.

### 9A.7 First-Launch and Setup Experience

The first launch should never dead-end.

The MVP should include minimal setup guidance for:

- Ollama missing
- Ollama unreachable
- No local model installed
- Selected model missing
- Diagnostics benchmark unavailable

Setup guidance should be clear enough for non-technical users and should avoid requiring command-line knowledge for normal use.

### 9A.8 Settings Experience

Settings should feel like a control center, not a configuration dump.

MVP categories:

- General
- Providers
- Models
- Storage
- Privacy
- Diagnostics
- Advanced

The Advanced section should contain only clearly named settings with direct user value. It must not become a dumping ground for unclear toggles or internal implementation details.

### 9A.9 Diagnostics Experience

Diagnostics should present practical expectations, not just raw numbers.

The performance page should show:

- Readiness status
- Ollama reachability
- Selected model availability
- Basic system information where reliable
- Time to first token
- Tokens per second
- Total benchmark response time
- Streaming support
- Practical guidance such as "good for small models", "expect slower responses", or "selected model may exceed available memory"

Benchmark results should be local-only and should not store sensitive prompt content.

### 9A.10 Keyboard-First UX

Keyboard navigation should be considered from the beginning.

MVP architecture should make these future shortcuts easy to add:

- New chat
- Search
- Settings
- Chat switching
- Command palette

The command palette is not required for the MVP and should be deferred.

### 9A.11 Animation Principles

Performance is more important than visual effects.

Use Framer Motion sparingly and intentionally for:

- Sidebar collapse/expand
- Modal appearance
- Toast notifications
- Provider selector transitions
- Settings panel transitions
- Chat transitions where helpful

Preferred animation styles:

- Fade
- Slide
- Subtle scale

Avoid:

- Bounce animations
- Spring-heavy effects
- Overly playful transitions
- Long animation durations

Recommended timing:

- Small interactions: 100-180ms
- Panels/modals: 180-260ms

Respect reduced-motion accessibility preferences.

### 9A.12 Accessibility

Accessibility is an implementation requirement.

Support:

- Keyboard navigation
- Visible focus states
- Screen reader semantics where practical
- Reduced motion preferences
- Sufficient contrast in both dark and light themes
- Predictable focus management for modals, panels, selectors, and settings

### 9A.13 UI Performance Requirements

The UI must remain responsive with:

- Large conversation histories
- Many chats
- Large message counts
- Long sessions
- Streaming responses

Use virtualization or incremental rendering when message/conversation counts make it necessary. Avoid frontend state shapes that require rerendering the entire chat history on every streamed token.

## 10. Phase 1 — MVP Foundation

### Goal

Create the first working local-first desktop assistant using Tauri, React, SQLite, Ollama, and a user-managed local inference host mode.

### Included Features

- Tauri desktop app
- Windows/macOS/Linux desktop support target
- React chat UI
- Tailwind CSS and shadcn/ui foundation
- Dark and light theme support
- Conversation sidebar
- Reserved/collapsible right panel
- Create conversation
- Rename conversation
- Delete conversation
- Edit previous user messages
- Regenerate assistant responses
- Preserve alternate responses as conversation branches
- Send user message
- Stream assistant response
- Stop/cancel streamed generation
- Store messages in SQLite
- Ollama provider integration
- Local inference host provider integration, initially targeting llama.cpp `llama-server`
- Generic provider settings screen seeded with Ollama and local inference host records
- Comprehensive settings screen for workspace, privacy, provider, model, and chat behavior
- Model selection
- Markdown rendering
- Code block rendering
- Syntax highlighting for code blocks
- Error handling
- Offline/local runtime status indicator
- First-launch setup/readiness guidance
- Setup guidance for missing Ollama, unreachable local inference host, or missing local models
- Minimal diagnostics and performance benchmark page
- Basic import/export of conversations

### Excluded From MVP

- Bundled Ollama installer
- Bundled llama.cpp installer or runtime binaries
- Bundled model files
- RAG/document chat
- Memory
- Agent workflows
- Terminal tools
- Web search
- Command palette
- Voice
- Image generation
- Multi-user accounts
- Cloud sync

### Phase 1 Tasks

#### 1. Project Setup

- Create Tauri app with React and TypeScript.
- Configure build scripts.
- Set up ESLint and formatting.
- Set up Tailwind CSS.
- Set up shadcn/ui source components.
- Set up Lucide Icons.
- Set up Framer Motion for targeted transitions only.
- Define initial design tokens for dark theme, light theme, spacing, radius, borders, typography, focus states, and surfaces.
- Add basic project README.
- Add initial app shell layout.

#### 2. SQLite Setup

- Add SQLite dependency in Rust.
- Create migration system.
- Add initial tables:
  - conversations
  - messages
  - providers
  - models
  - app_settings
  - schema_migrations if not supplied by the migration runner
- Add MVP indexes for conversation/message loading.
- Seed default local provider records:
  - Ollama
  - Local inference host
- Add database initialization on app startup.
- Store the database inside the configured Ark workspace folder.

#### 3. Chat UI

- Build three-column layout where appropriate:
  - sidebar
  - chat panel
  - reserved/collapsible right panel
- Implement conversation list.
- Implement message list.
- Implement message composer.
- Implement loading and streaming states.
- Implement empty, error, offline, cancelled, and interrupted states.
- Implement stop-generation control.
- Implement branch selection UI for regenerated responses.
- Implement visible provider, model, and local/cloud status in the chat header.
- Implement dark/light theme switching.

#### 4. Conversation Management

- Create conversation.
- Rename conversation.
- Delete conversation.
- Edit a previous user message by creating a new branch.
- Regenerate an assistant response by creating a new branch.
- Load conversation messages.
- Load the currently selected active branch.
- Persist conversation metadata.

#### 5. Ollama Provider

- Add default local provider:
  - name: Ollama
  - type: ollama
  - base URL: http://localhost:11434
- Implement chat completion call.
- Implement streaming response.
- Persist streamed assistant chunks incrementally as they arrive.
- Implement cancellation for active streams.
- Implement provider health check.
- Implement model listing if available.
- If Ollama is not running, keep model selection disabled and show concise setup guidance.

#### 5A. Local Inference Host Provider

- Add default local inference host provider:
  - name: Local inference host
  - type: local_inference_host
  - default base URL: http://localhost:8080
- Implement provider health check.
- Implement model listing through OpenAI-compatible `/v1/models` when available.
- Implement streaming chat through OpenAI-compatible `/v1/chat/completions`.
- Parse Server-Sent Events into normalized Ark stream events.
- Keep all provider-specific parsing inside the Rust adapter.
- If the local host is not running, keep model selection disabled and show concise setup guidance.
- Explain that the user must install/start the local host and choose GPU/CPU flags outside Ark for this milestone.

#### 6. Provider Settings

- Add settings page.
- Add workspace path and portable workspace controls.
- Add privacy/network controls.
- Allow editing provider base URL.
- Allow selecting default model.
- Allow configuring:
  - temperature
  - max tokens
  - streaming on/off
- Keep the Phase 1 provider UI generic and manage the seeded Ollama and local inference host providers.
- Allow switching between seeded local providers from both chat and settings.
- Load model lists per selected provider instead of assuming one global model list.

#### 7. First-Launch and Setup Guidance

- Add a minimal first-launch readiness state.
- Explain when Ollama is missing, unreachable, or has no selected model.
- Explain when a local inference host is unreachable, has no model list, or needs to be started externally.
- Keep the app usable when local inference is unavailable.
- Provide clear setup guidance without requiring command-line knowledge for normal use.
- Link setup state to provider/model health checks.

#### 8. Diagnostics and Performance Testing

- Add a Diagnostics section in Settings.
- Show supported platform and basic runtime readiness.
- Show basic system information where reliable, such as OS, CPU architecture, RAM, disk availability, and GPU/accelerator name.
- Check whether the selected local provider is reachable.
- Check whether the selected model is installed and available.
- Run a short local benchmark prompt against the selected model.
- Report approximate time to first token.
- Report approximate tokens per second.
- Report total benchmark response time.
- Report whether streaming works.
- Show practical model guidance based on observed results.
- Avoid storing sensitive benchmark prompt content.

#### 9. Markdown and Code Rendering

- Render assistant messages as Markdown.
- Render code blocks with language labels.
- Add syntax highlighting with a carefully chosen lightweight dependency.
- Add copy code button.
- Preserve plain text fallback.

#### 10. Error Handling

Handle common errors:

- Ollama not running
- Local inference host not running
- Model not installed
- Provider unreachable
- Invalid response
- Streaming interrupted
- Stream cancelled by user
- SQLite write failure
- Benchmark failed or unavailable

#### 11. Import/Export

- Export conversation to Markdown.
- Export conversation to JSON.
- Import JSON conversation export.

#### 12. Testing

- Unit test provider request mapping.
- Unit test mock provider streaming.
- Unit test database operations.
- Unit test conversation creation/deletion.
- Unit test edit/regenerate branch creation.
- Unit test stream cancellation persistence.
- Unit test diagnostics result mapping.
- Manual test dark and light themes.
- Manual test keyboard navigation and focus states on primary flows.
- Manual test reduced-motion behavior.
- Manual test Ollama streaming.
- Manual test local inference host streaming with a running llama.cpp server.
- Manual test offline behavior.
- Manual test diagnostics with Ollama unavailable.
- Manual test diagnostics with local inference host unavailable.
- Manual test diagnostics with missing selected model.

### Phase 1 Acceptance Criteria

Phase 1 is complete when:

- The app launches on desktop.
- The app targets Windows, macOS, and Linux desktop builds.
- Dark and light themes are available.
- The app shell includes a sidebar, main chat area, and reserved/collapsible right panel.
- A user can configure Ollama.
- A user can configure a local inference host such as llama.cpp server.
- A user receives setup guidance when Ollama, local inference host, or a selected model is missing.
- A user can select a local model.
- The active provider, model, and local/cloud status are visible while chatting.
- A user can create a chat.
- A user can send a message.
- A local model can stream a response.
- A user can stop a streamed response without losing partial content.
- Conversations are persisted locally.
- A user can edit a prior message and regenerate from that point without overwriting earlier content.
- A user can run a local diagnostics/performance test and see approximate performance expectations.
- Primary flows are keyboard navigable and have visible focus states.
- The app works without internet.
- A conversation can be exported to Markdown and JSON.

## 11. Phase 2 — Provider Abstraction and Cloud Support

### Goal

Make the app genuinely provider-agnostic.

### Features

- Generic OpenAI-compatible provider adapter
- Ollama provider refinement
- OpenAI provider
- Azure OpenAI provider
- OpenRouter-compatible provider
- Per-provider API key storage
- Per-chat provider/model selection
- Provider health checks
- Provider capability display

### Tasks

- Define Rust provider trait.
- Implement provider registry.
- Normalise request/response format.
- Add secure API key storage using OS keychain if possible.
- Add provider creation/edit/delete UI.
- Add model refresh/list models action.
- Add per-chat provider override.
- Add fallback provider option.

### Acceptance Criteria

- Multiple providers can be configured.
- User can switch provider per chat.
- Local and cloud providers use the same UI flow.
- API keys are not stored in plaintext if OS-level secure storage is available.
- Provider failures are clearly shown to the user.

## 12. Phase 3 — Local Knowledge and Document Chat

### Goal

Allow the assistant to use local documents without sending them to the cloud by default.

### Features

- Import TXT files
- Import Markdown files
- Import PDFs
- Extract document text
- Chunk documents
- Generate local embeddings
- Store document chunks
- Local vector search
- Chat with selected documents
- Source citations
- Document library
- Full document deletion

### Tasks

- Add document import UI.
- Add file copy/import strategy.
- Add text extraction pipeline.
- Add chunking module.
- Add embeddings provider interface.
- Add local embedding model support.
- Add vector search using SQLite vector extension or lightweight local vector store.
- Add document retrieval before chat completion.
- Add source references in assistant answers.
- Add document delete and cleanup.

### Acceptance Criteria

- User can import a local document.
- App extracts and chunks text.
- App creates local embeddings.
- User can ask questions about selected documents.
- Assistant includes source references.
- Documents can be fully removed from local storage.

## 13. Phase 4 — Local Memory

### Goal

Add user-controlled local memory similar to modern AI assistants, but stored privately.

### Features

- Add memory
- Edit memory
- Delete memory
- Disable memory globally
- Enable/disable memory per chat
- Search memory
- Memory review screen
- Memory injection into prompts

### Tasks

- Add memories table.
- Add memory UI.
- Add memory search.
- Add explicit memory commands.
- Add automatic memory suggestions only after user approval.
- Add per-chat memory toggle.
- Add memory audit trail.

### Acceptance Criteria

- User can manually store memories.
- User can edit/delete memories.
- Memory is never sent to cloud providers unless the user chooses that provider for a chat.
- Memory use is transparent in the UI.

## 14. Phase 5 — Local Tools

### Goal

Give the assistant useful local tools with strict permission boundaries.

### Initial Tools

- Calculator
- Local file search
- Local note creation
- Git repository reader
- Terminal tool with explicit approval
- Optional web search tool

### Safety Rules

- Destructive actions require confirmation.
- Terminal commands require approval before execution.
- File write/delete actions require approval.
- External network calls require approval unless explicitly enabled.
- Tool usage should be logged.

### Tasks

- Create tool interface.
- Add tool registry.
- Add permission prompts.
- Add calculator tool.
- Add read-only file search tool.
- Add note creation tool.
- Add Git read-only inspection tool.
- Add terminal tool behind strict permissions.
- Add tool execution logs.

### Acceptance Criteria

- Assistant can call approved tools.
- User can see what tool is being called and why.
- Risky tools cannot run silently.
- Tool execution is logged locally.

## 15. Phase 6 — Resilience, Backup, and Portability

### Goal

Make the assistant portable and restorable from backup.

### Features

- Full workspace export
- Full workspace import
- Backup verification
- Config backup
- Prompt library backup
- Memory backup
- Conversation backup
- Document index backup
- Portable model folder references
- Offline restore documentation

### Tasks

- Define workspace export format.
- Implement export as compressed archive.
- Include SQLite database.
- Include config JSON.
- Include prompt library.
- Include memory store.
- Include document index metadata.
- Include optional documents depending on user choice.
- Include model path references.
- Implement restore flow.
- Add backup integrity check.

### Acceptance Criteria

- User can export full workspace.
- User can restore workspace on another machine.
- Restored app can run with local models if available.
- Backup format is documented.

## 16. Phase 7 — Advanced Assistant Modes

### Goal

Expand the app into a powerful private AI workstation.

### Features

- Coding assistant
- Research assistant
- Document summarizer
- Command palette
- Prompt library
- Agent workflows
- Voice input
- Local speech-to-text
- Local text-to-speech
- Vision model support
- Image generation support

### Tasks

- Add mode selector.
- Add command palette.
- Add coding workspace.
- Add repository indexing.
- Add prompt template manager.
- Add workflow runner.
- Add Whisper/local STT support.
- Add local TTS support.
- Add vision model provider capability.
- Add image model provider capability.

### Acceptance Criteria

- User can use specialised modes.
- Modes still follow provider-agnostic design.
- Local-first behavior remains the default.

## 17. Security and Privacy Requirements

### Local Data

- Store all user content locally by default.
- Do not sync anything automatically.
- Do not send telemetry unless explicitly added and opt-in.
- Prefer no telemetry at all for the first versions.

### API Keys

- Do not store API keys in plain SQLite if avoidable.
- Use OS keychain/credential storage where practical.
- If plaintext fallback is necessary, warn the user clearly.
- The MVP should not require API keys because it only ships with the local Ollama provider.
- Future cloud providers should store only key references in SQLite, never raw secrets when OS credential storage is available.

### Cloud Provider Use

Before sending data to a cloud provider, the UI should make it clear which provider and model are being used.

The Rust core must also enforce cloud-provider enablement. The UI is not the only privacy control.

### Tool Permissions

Risky local actions must require user confirmation.

### Logs

Logs should avoid storing sensitive message content unless debug mode is explicitly enabled.

### Tauri Security

The Tauri command surface should stay narrow and explicit.

- Do not expose broad filesystem access to the frontend.
- Keep database access inside the Rust core.
- Use a strict Content Security Policy.
- Avoid remote assets in the MVP.
- Validate all command inputs in Rust.
- Redact message content, API keys, local paths, and provider payloads from normal logs.
- Treat the frontend as an untrusted caller for privacy-sensitive operations.

## 18. Configuration Rules

The settings page should be comprehensive but still MVP-focused. It should expose the concepts users need to control privacy, portability, provider behavior, and chat defaults without introducing later-phase features.

Settings categories:

- Workspace path and portable workspace mode
- Appearance and theme
- Privacy and external network behavior
- Provider configuration
- Model defaults
- Chat defaults
- Diagnostics and performance testing
- Import/export behavior
- Logging/debug behavior

Each provider/model should support:

- Model name
- Base URL
- API key reference
- Temperature
- Max tokens
- System prompt
- Context window
- Streaming on/off
- Tool support flag
- Vision support flag
- Embedding support flag

## 19. Export Formats

### Conversation Markdown Export

Should include:

- Conversation title
- Created date
- Provider/model used
- Messages from the active branch in chronological order
- A note when alternate branches exist

### Conversation JSON Export

Should include:

- Conversation metadata
- Messages
- Message parent/revision relationships for branches
- Provider/model metadata
- Export schema version

Conversation JSON import must validate:

- Supported export schema version
- Required conversation and message fields
- Valid message roles and statuses
- Valid parent/revision references for branches
- No path traversal or filesystem side effects
- ID conflicts with existing local data

If imported IDs conflict, Ark should generate new local IDs and preserve original IDs in import metadata.

### Workspace Export — Later Phase

Should include:

- Schema version
- App version
- SQLite database
- Config JSON
- Prompt templates
- Memories
- Document metadata
- Optional document files
- Model path references

## 20. Testing Strategy

### Unit Tests

- Provider request mapping
- Provider response parsing
- Mock provider streaming and cancellation
- SQLite CRUD operations
- Conversation lifecycle
- Conversation edit/regenerate branch behavior
- Settings persistence
- Import/export serialization

### Integration Tests

- Ollama health check
- Ollama chat completion
- Streaming response handling
- Diagnostics readiness check
- Local benchmark execution
- Provider switching
- Database migration

### Manual Tests

- App launches without internet
- App handles Ollama not running
- App handles missing model
- App shows setup guidance for missing Ollama/model
- App runs diagnostics benchmark with an available local model
- App supports dark and light themes
- App handles narrow desktop windows without layout overlap
- App primary flows work with keyboard navigation
- App respects reduced-motion preferences
- App remains responsive with many conversations and long message histories
- App handles long responses
- App handles interrupted streams
- App exports and imports chats correctly

### Later Security Tests

- API key storage behavior
- Tool permission prompts
- File deletion confirmation
- Terminal command approval
- Workspace restore safety

## 21. Development Milestones

### Milestone 1 — App Skeleton

- Tauri app runs
- React UI loads
- Basic layout exists
- Tailwind CSS and shadcn/ui foundation exists
- Dark and light theme tokens exist
- Sidebar, main chat area, and reserved/collapsible right panel exist
- Configurable Ark workspace is initialized
- SQLite initializes
- Initial MVP migrations run
- Seeded Ollama provider exists
- Seeded local inference host provider exists
- Typed Tauri command boundary exists

### Milestone 2 — Local Chat Works

- Ollama provider works
- Local inference host provider works through a user-managed llama.cpp server
- Active provider, model, and local/cloud status are visible in the chat header
- User can send message
- Assistant streams response
- Stream chunks are persisted incrementally
- Stop generation works
- Messages are stored with status
- Streaming UI handles loading, interrupted, cancelled, and error states

### Milestone 3 — Conversation Management

- Create, rename, delete conversations
- Edit previous user messages
- Regenerate assistant responses
- Preserve alternate branches
- Branch controls are clear when alternate responses exist
- Sidebar works
- Conversation history loads correctly

### Milestone 4 — Settings and Models

- Provider settings screen exists
- Workspace/privacy/appearance/chat settings exist
- Diagnostics/performance page exists
- First-launch setup/readiness guidance exists
- User can update Ollama URL
- User can update local inference host URL
- User can select model
- Health check works
- Model selection is disabled with setup guidance when the selected local provider is unavailable
- User can run a short benchmark and see approximate performance

### Milestone 5 — Export and Polish

- Markdown export works
- JSON export/import works
- Errors are user-friendly
- MVP is stable enough for daily local use

## 22. Codex Implementation Guidance

When asking Codex or another coding agent to implement the project, use these rules:

- Follow this implementation plan exactly.
- Do not add features outside the current phase unless explicitly requested.
- Prefer simple, maintainable code over clever abstractions.
- Keep provider logic isolated from the UI.
- Keep database access isolated in the Rust core.
- Use typed request/response objects between frontend and Tauri commands.
- Treat Rust command DTOs/provider traits as the source of truth; keep TypeScript types mirrored or generated from that boundary.
- Use Tailwind CSS, shadcn/ui source components, and Lucide Icons for the frontend foundation.
- Use Framer Motion sparingly for purposeful transitions; prioritize responsiveness over animation.
- Keep Ark feeling like a personal AI workspace, not an enterprise dashboard, admin portal, IDE, or system utility.
- Support both dark and light themes in the MVP.
- Keep the active provider, model, local/cloud state, and offline/runtime status visible in the chat experience.
- Include first-launch setup/readiness guidance for missing Ollama, missing local inference host, or missing models.
- Include the reserved/collapsible right panel in the shell, but do not implement future documents, memory, or tools in the MVP.
- Defer the command palette until after the MVP while keeping keyboard navigation extensible.
- Implement visible focus states, keyboard navigation, and reduced-motion support from the beginning.
- Add meaningful errors instead of silent failures.
- Implement migrations from the beginning.
- Persist streamed assistant chunks incrementally and preserve partial content on cancellation/failure.
- Implement edits, regenerations, and branches append-only; do not overwrite prior messages.
- Do not hardcode Ollama beyond the default seeded provider; the UI must work with Ollama and local inference host mode through the same provider/runtime abstraction.
- Keep all user data local.
- Enforce privacy and provider/network rules in the Rust core, not only in the UI.
- Guide users through Ollama/local-host/model setup in the MVP; do not bundle local runtimes or model files.
- Prefer observed benchmark results over fragile GPU assumptions when reporting performance expectations.
- Do not introduce cloud dependencies into the MVP.
- Do not add authentication or accounts.
- Do not add telemetry.

## 23. MVP Definition

The MVP is complete when the user can:

1. Install and open the app.
2. Use Ark on a supported Windows, macOS, or Linux desktop device.
3. Use both dark and light themes.
4. See a sidebar, main chat area, and reserved/collapsible right panel.
5. Get setup guidance if Ollama or a selected model is missing.
6. Configure Ollama as a local provider.
7. Configure a user-managed local inference host as a local provider.
8. Select a local model.
9. Always see the active provider, model, and local/cloud state while chatting.
10. Run a local diagnostics/performance test.
11. Create a conversation.
12. Send a message.
13. Receive a streamed response from the local model.
14. Stop a streamed response while preserving partial content.
15. Save and reload conversation history.
16. Rename and delete conversations.
17. Edit a previous message and regenerate without overwriting older content.
18. Use primary flows with keyboard navigation and visible focus states.
19. Export conversations to Markdown and JSON.
20. Use the app without internet access.

## 24. Non-Goals for MVP

Do not include the following in the first version:

- User accounts
- Cloud sync
- Mobile app
- Bundled local model runtime installers
- Bundled model downloads or model files
- RAG/document chat
- Memory
- Agents
- Voice
- Image generation
- Web search
- Terminal execution
- Command palette
- Multi-user support
- Plugin marketplace

## 25. Final Recommendation

Start with a narrow but solid MVP:

```text
Tauri + React + TypeScript
Tailwind CSS + shadcn/ui
Rust core
SQLite storage
Ollama provider
Streaming chat
Conversation history
Provider/model settings
Dark/light themes
Visible provider/model/locality status
Reserved/collapsible right panel
Setup guidance
Diagnostics/performance benchmark
Markdown and code rendering
Import/export
```

This creates the foundation for the real long-term goal: a private, portable, provider-agnostic AI assistant that remains useful even if commercial AI services disappear or become unavailable.
