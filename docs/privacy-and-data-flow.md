# Privacy and data flow

What Ark stores, what it sends anywhere, and under what conditions — the single place this is
stated end to end. Individual mechanisms are documented in more depth in the linked docs; this
page's job is to make sure nothing falls between them.

## What stays on this machine, always

- **Conversations and messages** — plaintext SQLite by default, with an optional
  SQLCipher-encrypted workspace (see [docs/data-at-rest.md](data-at-rest.md) for exactly what
  that does and does not protect against).
- **Provider credentials** — never in SQLite or browser storage; only an opaque reference is
  persisted, with the real value in the OS credential store (see
  [docs/secrets-and-backups.md](secrets-and-backups.md)).
- **Runtime/diagnostic logs** — bounded (a capped, single-rotation local file plus a smaller
  in-memory copy of recent entries), redacted before being written or buffered at all (paths,
  bearer tokens, cookies, sync tokens, and other auth-shaped values never enter either sink in the
  first place — see [docs/runtime-diagnostics-policy.md](runtime-diagnostics-policy.md) and
  [docs/diagnostics-and-logs.md](diagnostics-and-logs.md)). Nothing is sent anywhere unless the
  user explicitly exports and shares a diagnostics bundle.
- **Project Repository bindings** — a Project may store the canonical path of a codebase selected
  for Ark Code. Binding probes the directory with a collision-safe temporary file and removes it
  immediately; it does not scan, copy, or upload Repository contents. The Repository may not
  overlap Ark's private storage Workspace, and Ark Code paths must pass the canonical containment
  resolver before a tool can use them.
- **No telemetry, no behavioral analytics, no account.** This has been true since the project's
  original MVP scope and remains an explicit, actively-preserved property — see the "Explicit
  current non-actions" list in `implementation-plan.md` Section 7.

## What can leave this machine, and exactly when

| What | Leaves the machine when | Where it goes | Disclosed how |
|---|---|---|---|
| Chat messages / conversation content | Every message sent to a non-local provider | The selected provider's API endpoint | The provider/model picker shows a destination-class badge computed server-side in Rust (SEC-001); before send, the composer names the endpoint, route, model, and outbound context categories |
| Runtime/model downloads | Only when the user explicitly installs the runtime or selects Download for a catalog model | Runtime artifacts use the pinned source in `config/native-artifacts.json`; model files use the immutable publisher source and redirect-host allowlist in `config/model-catalog.json` | Settings shows runtime/model provenance, exact source, license, size, and SHA-256; model bytes remain in device-local storage and are never uploaded |
| Companion API / local integrations | Only when the user explicitly enables it in Settings | Stays on this machine: the current server binds loopback (`127.0.0.1`) only | Settings states the network scope, shows the live loopback URL, and requires a one-time-revealed bearer token for every route |
| Future phone access | Only after the user explicitly enables paired-LAN mode and pairs a device (MOB-009; not implemented yet) | Will stay on the local network — LAN-only by design, no cloud relay | The future pairing screen and paired-device list will be the disclosure and revocation surface |
| Anything else | Never | — | — |

Cloud provider selection is the one place data leaves the machine as a normal part of using the
app, and it is opt-in per provider, never a default (`config/release-capabilities.json` records
the capability, but the database seeds no cloud provider). Curated OpenAI uses the fixed official
HTTPS endpoint; advanced compatible endpoints are explicitly labelled unverified. Provider
retention, privacy terms, and billing apply independently of Ark.

## The companion API and LAN pairing specifically

Because this is the newest and most likely to be misunderstood: the companion API currently opens
an authenticated loopback-only HTTP server for integrations on the same computer. It is off by
default, has no unauthenticated route, and is not reachable from another device. An authenticated
integration can list sanitized provider/model selection data and submit or cancel chat generation;
if it selects a non-local provider, that message leaves the machine under the same disclosure and
provider privacy terms described in the table above. Provider endpoints, keychain references, and
raw adapter metadata are not returned by the API. MOB-009 will add
the separately controlled paired-LAN mode and per-device tokens described by SEC-010; that mode is
not implemented yet. Neither mode uses a cloud relay or account. If a user later wants LAN-mode
access from outside the home network, that is their own VPN (Tailscale, WireGuard, etc.) layered
entirely outside Ark — Ark's companion API does not provide or configure remote access.

## Workspace and Repository are different boundaries

**Workspace** means Ark's own app-data location: SQLite, attachments, backups, and related private
state. Changing it is a storage operation and requires restart. **Repository** means the optional
user codebase bound to one Project for Ark Code. Binding, switching, or removing a Repository is
immediate and never moves or changes Workspace data. Ark rejects overlap in either direction so a
repository-scoped tool cannot inherit access to Ark's private app data from a broad path.

## Crash reporting

Opt-in only, off by default (OPS-001). When enabled (Settings → Diagnostics bundle), an uncaught
crash is recorded — redacted, the same as every other log line — to the local diagnostics log
file described above, so it can be included in a diagnostics bundle after Ark restarts. There is
no crash-report *service*: no Sentry, no telemetry endpoint, no background upload, and no "basic
telemetry while the real feature is built" — the only way any of this data ever leaves the device
is the manual, reviewed diagnostics bundle export, same as everything else on this page. See
[docs/diagnostics-and-logs.md](diagnostics-and-logs.md) for the full disclosure.

## If you're not sure whether something is disclosed here

Treat that as a real gap in this document, not as permission to assume the safer answer. Check
`docs/dependency-advisory-review.md`, `docs/data-at-rest.md`, `docs/secrets-and-backups.md`,
`docs/runtime-diagnostics-policy.md`, and `implementation-plan.md`'s SEC-* task entries — if the
behavior genuinely isn't documented anywhere, that's worth filing as an issue against this
document specifically.
