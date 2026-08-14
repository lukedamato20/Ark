# Protocol versioning and deprecation policy

ARC-002 acceptance criterion: "Protocol versioning/deprecation policy is documented." This
covers the two things that cross the Tauri IPC boundary between the Rust backend and the
TypeScript frontend: **commands** (request/response, invoked via `ArkClient`) and **events**
(fire-and-forget, delivered via `ArkClient`'s `onX` subscription methods).

## The contract

Every DTO shared between the two sides has exactly one canonical field list, recorded in
[`contract/schema.json`](../contract/schema.json). Two independent checks assert against it:

- `src-tauri/src/contract.rs` (`#[cfg(test)]`-only) — constructs a sample instance of each Rust
  struct, serializes it, and asserts the JSON key set matches the fixture entry.
- `scripts/check-contract.mjs` (run via `pnpm run contract:check`, wired into CI's `frontend`
  job) — parses `src/types/ark.ts` with the TypeScript compiler API and asserts each interface's
  declared property names match the same fixture entry.

Neither check reads the other language's source. A change to a Rust struct's fields fails the
Rust test; a change to the corresponding TypeScript interface fails the TS check; a change to
only one side (the actual drift this exists to catch) fails whichever side didn't change,
because the fixture no longer matches it. **Both sides must be edited in the same change, along
with `contract/schema.json` itself.**

## Changing a command's request or response shape

1. Decide whether the change is **additive** (new optional field, new field with a sensible
   default when absent) or **breaking** (renamed/removed/retyped field, new required field).
2. Update the Rust struct, the TypeScript interface in `src/types/ark.ts`, and
   `contract/schema.json` together.
3. Additive changes need nothing else — an older frontend simply never reads the new field.
4. Breaking changes to a command are safe in this codebase's actual deployment model: the
   frontend and backend ship as one signed bundle (see `COR-012`/`SEC-004`) and are always the
   same version at runtime — there is no scenario where an old frontend talks to a new backend
   command, or vice versa. A breaking command change therefore just needs the whole app rebuilt
   and the contract fixture updated; there is no cross-version compatibility matrix to maintain.

## Changing an event's shape (`StreamEvent`, `OllamaPullProgress`)

Events are different from commands in one respect worth calling out even though the
single-bundle deployment model above still applies today: an event payload is the one shape in
this protocol that has an explicit, checked version number, because event delivery is
fire-and-forget and a handler has no request/response round-trip to fall back on if it
misinterprets a payload.

`StreamEvent` carries `schemaVersion` (Rust: `chat::STREAM_EVENT_SCHEMA_VERSION`; TypeScript:
`KNOWN_STREAM_EVENT_SCHEMA_VERSION` in `src/lib/ArkClient.ts`). The frontend's event handlers
(installed via `ArkClient`'s `onStreamDelta`/`onStreamComplete`/etc.) drop — with a
`console.warn`, not a crash — any event whose `schemaVersion` is higher than
`KNOWN_STREAM_EVENT_SCHEMA_VERSION`. This is what "unknown-version handling" means concretely.

To change `StreamEvent`'s shape:

- **Additive, backward-compatible** (new optional field an older handler can safely ignore):
  update the struct/interface/fixture as above; leave `schemaVersion` unchanged.
- **Breaking** (a field's meaning changes, or an older handler would misinterpret the new
  payload if it didn't know to ignore it): bump `STREAM_EVENT_SCHEMA_VERSION` /
  `KNOWN_STREAM_EVENT_SCHEMA_VERSION` together, and add an entry to the changelog below. Because
  frontend and backend always ship together, this guard exists less for cross-version
  compatibility today and more so that a genuine payload-shape bug in a future refactor fails
  loudly (an unexpected version showing up) instead of silently corrupting UI state.

`OllamaPullProgress` does not currently carry a schema version — it is a lower-stakes,
purely-additive-so-far progress payload. If it ever needs a breaking change, give it the same
`schemaVersion` treatment as `StreamEvent` at that point rather than speculatively adding it now.

## Deprecating a command or event

1. Mark the old command/event as deprecated in its Rust doc comment and its `ArkClient` method's
   TSDoc comment, stating the replacement and the intended removal point (a version or a dated
   milestone).
2. Migrate every frontend call site to the replacement in the same change where practical.
3. Remove the deprecated command/event (Rust command handler, `ArkClient` method, `contract/schema.json`
   entry, and generated `tauri::generate_handler!` registration) only once no call site references it —
   `cargo clippy` and `pnpm run contract:check` both fail loudly if something is missed (an unused
   command is still compiled, but an orphaned contract fixture entry with no matching struct
   fails `contract.rs`; a stale TypeScript interface with no fixture entry is not currently
   flagged — this is a known gap, see Known gaps below).

## Known gaps

- **Enum/status string contract coverage.** `contract/schema.json` covers struct field names,
  not the closed sets of string values for fields like `Message.status`
  (`MessageStatus` in TypeScript) or `ProviderConfig.destinationClass` (`DestinationClass`).
  Rust represents these as plain `String`, not a real enum, so there is no single Rust
  declaration to check a fixture against yet. Tightening this (likely alongside a broader
  status-modeling pass under the `COR-002`/`ARC` family) is future work, not part of ARC-002.
- **Fixture entries with no corresponding TypeScript interface.** `scripts/check-contract.mjs`
  reports every `contract/schema.json` entry that has no matching Rust struct fails
  `contract.rs`, but a TypeScript interface removed from `src/types/ark.ts` without removing its
  Rust struct is only caught by the TS import graph failing to compile at whatever now-unresolved
  call site used it — not by the contract check itself. In practice this is caught by
  `pnpm run typecheck` regardless.
