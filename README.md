# Ark

Ark is a local-first personal AI assistant and personal AI infrastructure desktop app. The MVP is a Tauri + React + TypeScript application with a Rust core, SQLite storage, and an Ollama provider adapter.

## MVP Scope

- Desktop app shell for Windows, macOS, and Linux
- React + TypeScript frontend
- Tailwind CSS, shadcn-style local components, Lucide icons, and restrained Framer Motion transitions
- Dark and light themes
- SQLite local storage through the Rust core
- Conversation create, delete, history loading, and message persistence
- Ollama health check, model refresh, and streaming chat
- Provider/model selectors visible in the chat header
- Settings control center for appearance, provider, storage, privacy, and diagnostics
- Configurable workspace folder with portable-workspace restart flow
- Markdown and code block rendering with syntax highlighting
- Conversation export to Markdown and JSON
- Conversation JSON import
- Minimal local diagnostics benchmark

## Prerequisites

Install these before running Ark:

- Node.js and pnpm
- Rust
- Tauri desktop prerequisites for your operating system
- Ollama, installed separately from Ark

Ark does not bundle Ollama or model files in the MVP.

## Ollama Setup

Start Ollama and install at least one local model:

```powershell
ollama pull llama3.2
```

Ark defaults to `http://localhost:11434` for Ollama.

## Run In Development

Install dependencies:

```powershell
pnpm install
```

Run the desktop app:

```powershell
pnpm tauri:dev
```

Run frontend validation:

```powershell
pnpm typecheck
pnpm build
```

Run Rust validation:

```powershell
cd src-tauri
cargo check
```

## Workspace Storage

Ark stores its local SQLite database as `ark.sqlite3` inside the active workspace folder. By default, that workspace lives under the platform app data directory.

In Settings → Storage, you can enter an absolute folder path for a portable workspace. Ark validates that the folder can be created and written to, then uses it after you close and reopen the app.

The MVP does not automatically move an existing database into the new workspace. Export conversations first or manually copy `ark.sqlite3` if you want to move current data.

## Planning Docs

- [Implementation plan](implementation-plan.md)
- [Remaining features](docs/remaining-features.md)

## Keyboard Shortcuts

- `Ctrl/Cmd + N`: create a new chat
- `Ctrl/Cmd + F`: focus conversation search
- `Ctrl/Cmd + ,`: open settings
- `Ctrl/Cmd + Enter`: send the current message

## Known MVP Limitations

- Ollama is the only implemented provider.
- Cloud providers, RAG, document chat, memory, agents, voice, image generation, and local tools are intentionally not implemented.
- Command palette is deferred.
- GPU detection is not implemented; diagnostics use observed benchmark results and basic system information.
- Workspace path changes require an app restart and do not automatically migrate existing data.
- Edit and retry create append-only branches. Branch browsing/switching beyond the active branch is still limited.
- Conversation JSON import validates schema, message roles/statuses, and branch references, but full workspace backup/restore is later-phase work.
- API keys are not needed for the MVP.

## Recommended Next Steps

1. Complete validation and fix any platform-specific Tauri build issues.
2. Improve branch browsing/switching on top of the existing append-only message model.
3. Add focused diagnostics result tests and provider integration tests.
4. Improve long-history chat performance with virtualization or incremental loading.
5. Expand provider abstraction in Phase 2 without changing the chat UI contract.
