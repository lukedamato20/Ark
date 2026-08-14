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
- At least one supported local runtime: Ollama or an OpenAI-compatible local server

Ark does not bundle Ollama, llama.cpp, or model files. The setup-script-installed llama.cpp
launcher remains available for development, but is hidden and disabled in release builds until
the pinned upstream server can meet Ark's complete endpoint-authentication and browser-origin
isolation requirements. See the [support matrix](docs/support-matrix.md) for exact artifact
claims.

The development setup scripts read only `config/native-artifacts.json`, verify the pinned
artifact's checked-in size and SHA-256 before extraction, reject unsafe archive entries, and
atomically install it with per-file provenance. `pnpm supply-chain:check` verifies the archive
safety tests plus the checked-in CycloneDX SBOM and third-party notices.

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
pnpm format:check
pnpm lint
pnpm typecheck
pnpm architecture:check
pnpm support:check
pnpm contract:check
pnpm test:frontend
pnpm build
```

Run Rust validation:

```powershell
cd src-tauri
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo build
```

## Workspace Storage

Ark stores its local SQLite database as `ark.sqlite3` inside the active workspace folder. By default, that workspace lives under the platform app data directory.

In Settings → Storage, you can enter an absolute folder path for a portable workspace. Ark validates that the folder can be created and written to, then uses it after you close and reopen the app.

The MVP does not automatically move an existing database into the new workspace. Export conversations first or manually copy `ark.sqlite3` if you want to move current data.

## Planning Docs

- [Implementation plan](implementation-plan.md)
- [Architecture and module ownership](docs/architecture/README.md)
- [Versioned desktop support and capability matrix](docs/support-matrix.md)
- [Quality and performance evidence baseline](docs/quality-baseline.md)
- [Conversation import format and limits](docs/import-format.md)
- [Credential storage, export, and restore behavior](docs/secrets-and-backups.md)
- [Local data-at-rest protection and threat model](docs/data-at-rest.md)
- [Remaining features](docs/remaining-features.md)

## Keyboard Shortcuts

- `Ctrl/Cmd + N`: create a new chat
- `Ctrl/Cmd + F`: focus conversation search
- `Ctrl/Cmd + ,`: open settings
- `Ctrl/Cmd + Enter`: send the current message

## Known MVP Limitations

- Ark supports Ollama, an OpenAI-compatible local inference host, and the setup-script-installed
  managed llama.cpp host. No cloud provider is enabled.
- Cloud providers, RAG, document chat, memory, agents, voice, image generation, and local tools are intentionally not implemented.
- Command palette is deferred.
- GPU detection is not implemented; diagnostics use observed benchmark results and basic system information.
- Workspace path changes require an app restart and do not automatically migrate existing data.
- Edit and retry create append-only branches. Branch browsing/switching beyond the active branch is still limited.
- Conversation JSON import validates schema, message roles/statuses, and branch references, but full workspace backup/restore is later-phase work.
- API keys are not needed by the current local providers. Ark's credential boundary is ready for
  future authenticated providers: values are stored in the operating-system credential store,
  never SQLite, localStorage, conversation exports, diagnostics, or automatic clipboard writes.

## Roadmap

The [implementation plan](implementation-plan.md) is the source of truth for remaining hardening,
feature, performance, mobile, and release work. The support matrix deliberately does not claim
roadmap capabilities before their acceptance evidence exists.
