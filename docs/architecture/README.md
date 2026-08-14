# Ark architecture

This document defines where production code belongs and the allowed dependency direction. The
goal is reviewable use-case-sized modules, not layers for their own sake.

## Frontend

```text
main/App (composition)
  ├─ app (effects and use-case coordination)
  ├─ features/components (rendering and feature-local interaction)
  ├─ state (normalized server snapshots and reconciliation)
  ├─ lib (ArkClient port/adapter and pure utilities)
  ├─ ui (design primitives)
  └─ types (shared DTOs)
```

- `src/main.tsx` constructs the real Tauri-backed `ArkClient` and providers. `App.tsx`
  composes screen containers and may depend on every frontend area.
- `src/app` coordinates ArkClient commands/events and state invalidation. It must not import
  features, shell components, or UI primitives.
- `src/features/<feature>` owns a user workflow and its local ephemeral state. A feature may
  compose another feature in one direction (for example Settings owns Diagnostics), but cycles
  are forbidden. Features never import application orchestration or Tauri APIs.
- `src/components` holds app-shell presentation shared by screens. It does not own server
  state or import features.
- `src/state` owns immutable, normalized server snapshots and reconciliation helpers. It can
  depend on shared DTOs and pure libraries, never UI/features/application orchestration.
- `src/lib` contains the ArkClient port, its sole Tauri transport adapter, contexts, and pure
  utilities. It cannot depend on state or UI. Only `src/lib/ArkClient.ts` may import
  `@tauri-apps/api`.
- `src/ui` contains style-system primitives and depends only on pure libraries/types.
- `src/types` contains the Rust/TypeScript contract DTOs and is the lowest frontend layer.

Run `pnpm architecture:check` to resolve every relative TypeScript import, enforce these
directions and the Tauri boundary, and reject circular frontend imports. CI runs the same check.

## Rust

```text
lib.rs / commands (Tauri composition and transport)
  └─ application services (generation, import/export, diagnostics,
       provider management, workspace bootstrap)
       ├─ domain/protocol (chat, export, validation, errors)
       └─ infrastructure (db, providers, security, sidecar,
            device settings, workspace)
```

- `commands` translates Tauri parameters/state into calls to application-service functions and
  maps results back; business transactions do not belong there.
- Application-service modules coordinate one workflow against plain `&AppState`, which keeps
  them constructible in tests without a Tauri runtime.
- `chat`, `export`, `validation`, and `errors` define domain records and invariants.
- `db`, `providers`, `sidecar`, `device_settings`, `workspace`, and `security` own
  external-system details. Provider protocol implementations are reached through the
  `Provider` trait/registry; SQLite access is reached through `Database`.
- `lib.rs` is the composition root: it owns `AppState`, startup/shutdown wiring, migrations,
  and command registration.

Rust's compiler resolves the module graph; strict clippy and characterization/integration tests
are the enforcement gates. Extract a new module only when it owns a concrete use case or external
boundary. Preserve behavior with a focused test before or alongside each extraction.

## Change placement

1. Put shared wire-shape changes in Rust DTOs, `src/types/ark.ts`, and the contract fixture/check.
2. Put business behavior in a domain or application-service module, with commands kept thin.
3. Put transport/storage/process details behind the existing client, database, provider, or
   sidecar boundary.
4. Keep transient visual state inside its feature; promote it to `src/state` only when multiple
   consumers or authoritative reconciliation require it.
5. Run focused tests first, then the complete validation suite at an item/phase boundary.
