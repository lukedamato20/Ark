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
- For development, at least one supported local runtime: Ollama, an OpenAI-compatible local
  server, or the pinned llama.cpp runtime installed by Ark's verified setup script

Ark never bundles Ollama or model files. Packaged-build CI now installs, executes, and bundles the
target-specific pinned llama.cpp runtime after size, SHA-256, archive, per-file, provenance, and
version/commit verification. Release visibility stays off until those packaged jobs are green on
every declared target. See the [support matrix](docs/support-matrix.md) for exact artifact claims.

The development and packaged-build setup paths read only `config/native-artifacts.json`, verify the pinned
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

In Settings → Storage, you can enter an absolute folder path for a portable workspace. Ark validates that the folder can be created and written to, then uses it after you close and reopen the app. Check "Copy current conversation data to the new location" to seed the new workspace with a verified copy of your current database — the copy is verified before Ark switches to it, and the original is left untouched either way. Leave it unchecked to start the new workspace empty.

Ark can also create a standalone, verified backup of the current workspace database at any time (Settings → Backup & Restore), independent of switching workspaces — see [docs/secrets-and-backups.md](docs/secrets-and-backups.md).

Ark Code uses a separate **Repository** boundary. A Project can optionally bind an existing code
directory under Settings → AI & Behavior → Projects. Binding or switching is immediate, stores
only the canonical path, and never moves Workspace data. Ark rejects a Repository that overlaps
the storage Workspace and confines future Ark Code filesystem paths to the bound Repository.

## Companion API

Ark includes a disabled-by-default, authenticated integration API that binds to `127.0.0.1` on an
OS-assigned port. Under Settings → Companion API, generate and save the one-time-revealed bearer
token before enabling the API, then use the URL shown there. Every route—including health and the contract document—
requires `Authorization: Bearer <token>`.

The machine-readable OpenAPI 3.1 contract is available in
[`docs/companion-api.openapi.json`](docs/companion-api.openapi.json) and, while the API is running,
from authenticated `GET /v1/openapi.json`. Paired-LAN/phone access is not implemented yet; the
current listener is loopback-only.

Read operations list conversations, active message paths, sanitized provider summaries, and Ark's
cached model inventory. `POST /v1/conversations`, `PATCH /v1/conversations/{conversationId}`,
`POST /v1/conversations/{conversationId}/messages`, and `POST /v1/messages/{messageId}/cancel`
create/update conversations and drive the same durable generation lifecycle as the desktop UI.
Every mutation requires a unique `Idempotency-Key`; matching retries—even after an application
restart—return the original transaction result without duplicating a turn or provider request,
while reuse for a different request fails with `409`. Message state and streaming content are
read by polling the active message path. Selecting a non-local provider has the same outbound-data
implications as selecting it in Ark's desktop composer.

## Planning Docs

- [Implementation plan](implementation-plan.md)
- [Architecture and module ownership](docs/architecture/README.md)
- [Versioned desktop support and capability matrix](docs/support-matrix.md)
- [Quality and performance evidence baseline](docs/quality-baseline.md)
- [Conversation import format and limits](docs/import-format.md)
- [Credential storage, export, and restore behavior](docs/secrets-and-backups.md)
- [Local data-at-rest protection and threat model](docs/data-at-rest.md)
- [Diagnostics logs and crash capture](docs/diagnostics-and-logs.md)
- [Troubleshooting](docs/troubleshooting.md)
- [Building and installing a local desktop package](docs/installing.md)
- [Privacy and data flow](docs/privacy-and-data-flow.md)
- [Companion API OpenAPI contract](docs/companion-api.openapi.json)
- [Security policy and vulnerability reporting](SECURITY.md)
- [Incident response](docs/incident-response.md)
- [Secure development checklist](docs/secure-development-checklist.md)
- [Remaining features](docs/remaining-features.md)

## Keyboard Shortcuts

- `Ctrl/Cmd + N`: create a new chat
- `Ctrl/Cmd + F`: focus conversation search
- `Ctrl/Cmd + ,`: open settings
- `Ctrl/Cmd + Enter`: send the current message

## Known MVP Limitations

- Ark supports Ollama, an OpenAI-compatible local inference host, the setup-script-installed
  managed llama.cpp host, and opt-in OpenAI. No cloud provider is configured or selected by default.
- Additional cloud providers, RAG, document chat, memory, agents, voice, and image generation are intentionally not implemented.
- Command palette is deferred.
- GPU detection is not implemented; diagnostics use observed benchmark results and basic system information.
- Workspace path changes require an app restart and do not automatically migrate existing data.
- Edit and retry create append-only branches. Branch browsing/switching beyond the active branch is still limited.
- Conversation JSON import validates schema, message roles/statuses, and branch references, but full workspace backup/restore is later-phase work.
- API keys are optional for local providers and required for curated OpenAI. Values are stored in
  the operating-system credential store,
  never SQLite, localStorage, conversation exports, diagnostics, or automatic clipboard writes.

## Roadmap

The [implementation plan](implementation-plan.md) is the source of truth for remaining hardening,
feature, performance, mobile, and release work. The support matrix deliberately does not claim
roadmap capabilities before their acceptance evidence exists.
