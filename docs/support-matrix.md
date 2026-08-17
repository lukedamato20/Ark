# Ark desktop support matrix

Capability set: `desktop-local-v1` (schema 1)

Review status: **approved** by Luke D'Amato, 2026-08-14. These are the release candidates
encoded in `config/release-capabilities.json`; they must not be described as qualified release
support until the matching CI/platform evidence for each declared runner is green.

## Candidate platforms and window

| Platform artifact | Declared runtime target | Candidate minimum | Required packaged-build CI runner |
|---|---|---|---|
| Windows | `win32-x64` | Windows 10 22H2 | `windows-latest` |
| macOS | `darwin-arm64` | macOS 12 | `macos-latest` |
| Linux | `linux-x64` | Ubuntu 22.04 LTS | `ubuntu-latest` |

The reviewed native-artifact manifest also pins additional architectures, but Ark does not claim
packaged support for them until a matching runner installs, executes, and bundles that exact
artifact. CI for each target above runs the fail-closed installer, re-hashes every installed file,
executes `llama-server --version` and checks its source commit, then builds the real Tauri bundle.

The current desktop minimum is 980×640 CSS pixels. UX-001 must update both this matrix and
`tauri.conf.json` when the responsive minimum is qualified.

## Provider and runtime delivery

| Provider | Visible | Delivery/runtime claim | Network/privacy enforcement |
|---|---:|---|---|
| Ollama | Yes | Ollama must be installed and started separately | Loopback by default; other destinations require Rust-classified disclosure/acknowledgment |
| Local inference host | Yes | A compatible external server must be started separately | Loopback/LAN is labelled; public destinations require disclosure/acknowledgment |
| OpenAI | Yes, only after the user adds it | Optional remote service using Ark's curated Chat Completions adapter and fixed official HTTPS API endpoint; never seeded or selected by default | Creation requires outbound-data acknowledgment; credentials remain in the OS credential store; the composer discloses endpoint, route, model, and context before every remote send |
| Built-in llama.cpp host | Development only pending CI evidence | Release bundles are now built from a pinned, verified runtime resource; visibility remains off until all three packaged-build jobs are green | OS-assigned upstream port is isolated behind Ark's loopback-only authenticating proxy; every route requires a random per-launch bearer token and upstream CORS headers are removed |

The implementation no longer exposes llama.cpp's upstream HTTP surface directly: SEC-002's
authenticated, CORS-sanitizing loopback proxy is the only endpoint persisted or returned to the
webview. The release UI remains hidden until the packaged-build workflow is green for each
declared runtime target. A successful Ark package contains the verified runtime; replacing Ark
with a newer signed package is also the runtime update boundary, so the application never fetches
or executes an independently mutable "latest" binary.

In development builds, Ark's checked-in model catalog records the publisher, immutable source
commit, license, exact byte size, SHA-256, quantization, context, architecture, and reviewed
runtime/platform compatibility. Catalog downloads are resumable and are atomically promoted only
after size, digest, and GGUF validation. A manually obtained absolute GGUF remains available as an
advanced path and is clearly distinguished from catalog verification.

## Available in this capability set

- Local chat, streaming, edit/regenerate branches, conversation JSON/Markdown export, and
  validated JSON import.
- Local workspace SQLite storage and local provider diagnostics.
- Opt-in OpenAI and advanced user-supplied OpenAI-compatible remote endpoints. Ark does not seed
  a remote provider, and does not infer prices, context limits, privacy, or retention terms from
  an endpoint's model list.

## Explicitly unavailable

Accounts, sync, attachments/vision, RAG, tools/agents, web search, voice, automations, and a
mobile client are not release capabilities in this set. Placeholder
context panels are labelled as future work and do not claim functionality.

The machine-readable JSON is authoritative. Run `pnpm support:check` after changing this file,
the UI gate, Tauri window configuration, CI runner matrix, README, or onboarding/runtime copy.
