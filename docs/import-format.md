# Conversation/workspace import format and limits

Ark exports conversation and workspace schema version 2 JSON. Version 1 remains supported for
pre-attachment exports. Unknown fields are ignored within a supported schema version so additive
metadata remains forward compatible; a higher schema version is rejected before any database
mutation because Ark cannot safely assume changed semantics. The complete field and hashing
contract is documented in [export-format.md](export-format.md).

The production import ceilings are deliberately conservative:

| Limit | Value | Enforcement |
|---|---:|---|
| JSON payload | 50 MiB | browser `File.size` before `File.text`; Rust before deserialization |
| Messages | 20,000 | Rust validation |
| Content per message | 2,000,000 Unicode scalar values | Rust validation |
| Parent branch depth | 2,048 messages | Rust validation |
| Attachments per conversation | 10,000 | Rust validation |
| Content per attachment | 2 MiB | Rust validation, including metadata digest/size verification |

These build-time constants are configurable only within the hard ceilings in
`src-tauri/src/export/mod.rs`; changing one requires boundary tests, resource evidence, and review.
FTR-008 owns a future streaming archive format for data that legitimately exceeds them.

Before committing, Ark presents a dry-run summary with conversation/message/attachment counts, maximum
depth, ID conflicts, unavailable-provider mappings, transient-state normalization, and estimated
storage. Imported IDs are remapped to fresh local IDs while original IDs and existing metadata
are retained as provenance. Unknown providers map to the default local provider and are never
created from untrusted export configuration.

Each conversation import is one SQLite transaction, including its attachments and remapped
message links. A workspace bundle commits one complete conversation at a time so an interruption
never leaves a partial conversation but does retain earlier completed entries. Progress is reported every 100 messages (and at
completion). Cancellation is checked throughout the loop and immediately before final branch
selection; cancellation, validation failure, progress-delivery failure, or a database error rolls
back the new conversation and every message.
