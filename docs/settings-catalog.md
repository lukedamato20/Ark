# Settings catalog

ARC-006 acceptance criterion: "Settings catalog defines owner, default, validation, migration,
export/sync behavior, and UI location." This is the catalog. Every durable setting Ark has —
plus the schema fields the ARC-006 audit found dead or ambiguous and what was deliberately done
about each — is listed below, grouped by owner/scope.

## Scopes

Ark recognizes five settings scopes in practice today (of the six the plan names —
`device`, `workspace`, `provider`, `project`, `conversation`, `secret` — `project` doesn't apply
yet, since Ark has no project/workspace-folder concept beyond the single workspace database
itself):

- **Device** — this machine, this OS user account. Never portable, never synced through the
  workspace database. Persisted at the OS's per-user application-config directory as
  `device_settings.json` (see `src-tauri/src/device_settings.rs`), independent of which
  workspace is currently open.
- **Workspace** — the portable SQLite database file itself (`app_settings` table, plus
  first-class columns on `conversations`/`providers`). Travels with the file if the user copies
  it to another machine or a portable drive.
- **Provider** — a row in the `providers` table. Configuration for one configured provider
  instance (base URL, default model, temperature, etc.).
- **Conversation** — a row in the `conversations` table. Per-conversation overrides.
- **Secret** — never stored directly; only an opaque reference is persisted (see
  `providers.apiKeyRef` below). The actual credential lives in Windows Credential Manager,
  macOS Keychain, or Linux Secret Service through the `SecretStore` port.

## Catalog

| Setting | Owner/Scope | Default | Validation | Persistence | UI location |
|---|---|---|---|---|---|
| Theme (`dark`/`light`) | Device | `dark` | Must be `"dark"` or `"light"` (checked both client-side before the call and server-side in `device_settings::update_device_settings`) | `device_settings.json` (`theme`), mirrored into `localStorage["ark.theme"]` as an instant-first-paint cache only — see [Theme: cache vs. source of truth](#theme-cache-vs-source-of-truth) | Settings → Appearance |
| Built-in runtime model path | Device | `null` (not set) | Validated as an existing `.gguf` file (`validation::validate_model_path`) when actually starting the runtime; the settings write itself accepts any string | `device_settings.json` (`builtInModelPath`) | Settings → Provider → Built-in runtime |
| Built-in model provenance | Device | absent until a model is verified | Source/license are required bounded text; canonical regular GGUF file is streamed through SHA-256 before launch | OS app-config `model-provenance.json`, atomically replaced; path/source/license/hash/size/verification time only, never model content | Settings → Provider → Built-in runtime provenance card |
| Crash capture enabled (OPS-001) | Device | `false` (opt-in) | Boolean; no validation needed | `device_settings.json` (`crashCaptureEnabled`); `#[serde(default)]` so a pre-OPS-001 file still parses | Settings → Diagnostics bundle. See [docs/diagnostics-and-logs.md](diagnostics-and-logs.md) for the full retention/consent/revocation disclosure. |
| Sidebar collapsed | Device (UI view state) | expanded | — | `localStorage["ark.sidebar"]` only — see [Sidebar/right-panel collapse state](#sidebarright-panel-collapse-state) | Chat, sidebar toggle button |
| Right panel collapsed | Device (UI view state) | expanded | — | `localStorage["ark.rightPanel"]` only | Chat, right-panel toggle button |
| Provider base URL, provider class (`is_local`), insecure-remote development exception, default model, temperature, max tokens, streaming enabled | Provider | Seeded per provider type at first launch; local class and insecure-remote exception off unless explicitly changed (see `db::seed_defaults`) | `validation::validate_temperature`/`validate_max_tokens`; URL/class/TLS policy enforced on save and again before adapter construction via `security` (SEC-001) | SQLite `providers` table (`is_local`, `allow_insecure_remote`, and generation fields) | Settings → Provider |
| Provider API key reference | Provider (secret reference only) | `null` | Must be Ark's versioned opaque `secret:v1:<UUID>` format; raw values are accepted only by the write-only IPC command and never returned | SQLite `providers.api_key_ref`; credential value remains in the OS credential store | Settings → Provider → API credential for providers whose capability declares authentication |
| Provider capabilities (streaming, model listing/pull/delete, auth requirement, etc.) | Provider (computed, not a setting) | N/A | N/A | Not persisted — computed from `provider_type` on every read (`ProviderCapabilities::for_provider_type`, ARC-003) | Drives UI affordances (e.g. hiding the Ollama pull/delete panel) rather than being directly editable |
| Conversation system prompt | Conversation | `null` | N/A today — no write path exists | SQLite `conversations.system_prompt` | Not yet exposed in UI — reserved for a future `FTR` item |
| Theme (legacy, pre-ARC-006) | *(removed as a write target)* | — | — | SQLite `app_settings` key `appearance.theme` — no longer written; read exactly once, as a migration seed, by `workspace_bootstrap::get_app_bootstrap` the first time `device_settings.json` doesn't exist yet. The row itself is left in place, not deleted. | — |

## Resolved ambiguity: what ARC-006 found and did

The plan's own `Reason` field for this item named specific dead/ambiguous schema concepts found
during the original architecture audit. Each is resolved here:

- **`conversations.streaming_enabled`** — a per-conversation copy of `providers.streaming_enabled`,
  snapshotted at conversation-creation time. Nothing ever read it back to make a decision;
  generation always streams unconditionally. **Removed** via migration `0003` — a genuine dead
  duplicate, not a reserved future feature. `providers.streaming_enabled` remains as the one
  real setting (a provider-level user preference), distinct from `ProviderCapabilities.streaming`
  (a fixed protocol fact, not a preference — see the capabilities row above).
- **`conversations.system_prompt`** — always `null`; no command ever writes it. **Kept**,
  documented as reserved: a per-conversation custom system prompt is a clearly-intended, likely
  near-term feature (a `FTR`-family item, not architecture), and the column's shape won't need to
  change when that feature ships. Removing and later re-adding an identical column would be
  churn with no benefit.
- **`providers.api_key_ref`** — now populated only by `secret_store::upsert_provider_secret`
  after the operating-system credential write succeeds. The stored value is a versioned opaque
  UUID reference, never the credential itself; application exports clear it because it is
  device-local. The UI receives masked metadata and can replace/delete the value, but no IPC
  command returns the secret. No current provider declares `requires_auth`; the control becomes
  visible when FTR-007 adds one rather than falsely implying today's local providers use a key.
- **`app_settings` table** — a generic key-value table that, before this item, was used for
  exactly one key (`appearance.theme`) and nothing else. **Kept** as the workspace-scoped
  settings mechanism (a legitimate, general extensibility point matching the "workspace" scope
  above), but no longer written to for theme. It is empty of any actual key today; a future
  genuinely workspace-scoped setting (e.g. a per-workspace default provider) would use it.

## Notes

### Theme: cache vs. source of truth

Theme is a device setting, but the app also needs to paint the correct theme *before* the async
`get_app_bootstrap` call resolves, or the user sees a flash of the wrong theme on every launch.
`localStorage["ark.theme"]` exists purely to serve that instant-paint read
(`App.tsx`'s `useState(() => getStoredTheme())`) — it is a cache, not the source of truth.
`device_settings.json` (Rust-managed, OS-config-directory-scoped) is authoritative: every write
goes through `ArkClient.updateDeviceSettings`, which persists there; `localStorage` is updated
alongside it purely so the *next* launch's instant paint is already correct.

### Sidebar/right-panel collapse state

These are transient UI layout state (was the panel open the last time the user looked?), not
configuration a user deliberately sets the way they set a theme or a model path. They are
intentionally kept out of the device-settings JSON file and the backend entirely — pure
`localStorage`, matching how a browser remembers scroll position. Nothing durable is lost if
they're cleared; they just revert to the default expanded state.

### Legacy localStorage/DB migration

- **Theme**: migrates automatically, once, entirely server-side. `workspace_bootstrap::get_app_bootstrap`
  reads the legacy `app_settings["appearance.theme"]` value only when `device_settings.json`
  doesn't exist yet, seeds the new file with it if present, and never consults SQLite for this
  again afterward (`device_settings::resolve_device_settings`, unit-tested directly). No frontend
  action needed — `localStorage["ark.theme"]` already held the same value via the pre-ARC-006 code
  path and continues to.
- **Built-in model path**: had no backend copy before ARC-006 (localStorage-only). There is
  nothing to migrate *from* SQLite; a user's existing `localStorage["ark.builtIn.modelPath"]`
  value, if any, is simply superseded going forward — the field starts at `null` in
  `device_settings.json` until the user next starts the built-in runtime, at which point the
  path they enter is persisted through the new path. This was accepted as a one-time,
  low-friction re-entry (a single file path) rather than adding frontend-side migration
  complexity for a setting that previously had no durability guarantee at all (it was already
  lost on `localStorage` clear).
