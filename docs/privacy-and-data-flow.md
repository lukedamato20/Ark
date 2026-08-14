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
- **Runtime/diagnostic logs** — in-memory only, bounded, redacted before being buffered at all
  (paths, bearer tokens, and auth-shaped values never enter the buffer in the first place — see
  [docs/runtime-diagnostics-policy.md](runtime-diagnostics-policy.md)). Nothing is written to
  disk or sent anywhere unless the user explicitly exports a diagnostics bundle.
- **No telemetry, no behavioral analytics, no account.** This has been true since the project's
  original MVP scope and remains an explicit, actively-preserved property — see the "Explicit
  current non-actions" list in `implementation-plan.md` Section 7.

## What can leave this machine, and exactly when

| What | Leaves the machine when | Where it goes | Disclosed how |
|---|---|---|---|
| Chat messages / conversation content | Every message sent to a non-local provider | The selected provider's API endpoint | The provider/model picker shows a destination-class badge (loopback / private LAN / public) computed server-side in Rust (SEC-001), never silently upgraded by the UI |
| Model file downloads | Only when the user explicitly configures/downloads a runtime or model | The pinned, hash-verified source named in `config/native-artifacts.json` | `docs/dependency-advisory-review.md` and the runtime-provenance UI (Settings) show exactly what was verified and from where |
| Companion API / phone access | Only if a device has been explicitly paired (MOB-009) | Stays on the local network — LAN-only by design, no cloud relay | The pairing screen and the paired-device list in Settings are the entire disclosure surface; there is nothing happening a user didn't explicitly initiate by scanning a code |
| Anything else | Never | — | — |

Cloud provider selection is the one place data leaves the machine as a normal part of using the
app, and it is opt-in per provider, never a default (`config/release-capabilities.json`:
`cloudProviders: false` until FTR-007 changes that, and even then, no remote provider is
enabled by default — see FTR-007's own acceptance criteria).

## The companion API and LAN pairing specifically

Because this is the newest and most likely to be misunderstood: enabling the companion API
(FTR-010) and pairing a phone (MOB-009) does not send anything to a third party. It opens a
local HTTP server other devices *on the same network* can reach, gated by a per-device pairing
token (SEC-010). No cloud relay, no account, no vendor sees any of this traffic. If a user wants
access from outside the home network, that is their own VPN (Tailscale, WireGuard, etc.) layered
entirely outside Ark — Ark's companion API doesn't know or care how a request reached it,
consistent with the Phase 8 scope decision recorded in `implementation-plan.md`.

## Crash reporting

Opt-in only, not yet implemented (OPS-001). Until it exists, no crash data is collected or sent
anywhere, full stop — there is no "basic telemetry while the real feature is built," which
would contradict the no-telemetry property above.

## If you're not sure whether something is disclosed here

Treat that as a real gap in this document, not as permission to assume the safer answer. Check
`docs/dependency-advisory-review.md`, `docs/data-at-rest.md`, `docs/secrets-and-backups.md`,
`docs/runtime-diagnostics-policy.md`, and `implementation-plan.md`'s SEC-* task entries — if the
behavior genuinely isn't documented anywhere, that's worth filing as an issue against this
document specifically.
