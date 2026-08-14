# Ark desktop support matrix

Capability set: `desktop-local-v1` (schema 1)

Review status: **approved** by Luke D'Amato, 2026-08-14. These are the release candidates
encoded in `config/release-capabilities.json`; they must not be described as qualified release
support until the matching CI/platform evidence for each declared runner is green.

## Candidate platforms and window

| Platform artifact | Candidate minimum | Required CI runner |
|---|---|---|
| Windows | Windows 10 22H2 | `windows-latest` |
| macOS | macOS 12 | `macos-latest` |
| Linux | Ubuntu 22.04 LTS | `ubuntu-latest` |

The current desktop minimum is 980×640 CSS pixels. UX-001 must update both this matrix and
`tauri.conf.json` when the responsive minimum is qualified.

## Provider and runtime delivery

| Provider | Visible | Delivery/runtime claim | Network/privacy enforcement |
|---|---:|---|---|
| Ollama | Yes | Ollama must be installed and started separately | Loopback by default; other destinations require Rust-classified disclosure/acknowledgment |
| Local inference host | Yes | A compatible external server must be started separately | Loopback/LAN is labelled; public destinations require disclosure/acknowledgment |
| Built-in llama.cpp host | Development only | Disabled and hidden in release builds; a developer may install the pinned runtime with the setup script | OS-assigned loopback port and random per-launch bearer token, but the pinned upstream server still exempts health/model metadata routes from authentication and reflects browser origins |

The built-in path accepts an absolute GGUF path in development builds. Ark does not ship this
managed runtime in release builds because pinned llama.cpp b9859 cannot enforce authentication
and restrictive CORS on every endpoint. SEC-002 records the exact upstream constraint; a safe
authenticated proxy or an upstream release with the missing controls is required before the
capability can be enabled. Ark also does not yet discover, verify, download, or license-manage
model files; FTR-006/SEC-004/007 own those capabilities.

## Available in this capability set

- Local chat, streaming, edit/regenerate branches, conversation JSON/Markdown export, and
  validated JSON import.
- Local workspace SQLite storage and local provider diagnostics.

## Explicitly unavailable

Cloud-provider credentials, accounts, sync, attachments/vision, RAG, tools/agents, web search,
voice, automations, and a mobile client are not release capabilities in this set. Placeholder
context panels are labelled as future work and do not claim functionality.

The machine-readable JSON is authoritative. Run `pnpm support:check` after changing this file,
the UI gate, Tauri window configuration, CI runner matrix, README, or onboarding/runtime copy.
