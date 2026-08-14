# Conversation import format and limits

Ark exports schema version 1 JSON and accepts that version with unknown fields ignored for
forward-compatible additive metadata. There are no older supported schema versions yet. A newer
schema version is rejected before any database mutation.

The production import ceilings are deliberately conservative:

| Limit | Value | Enforcement |
|---|---:|---|
| JSON payload | 50 MiB | browser `File.size` before `File.text`; Rust before deserialization |
| Messages | 20,000 | Rust validation |
| Content per message | 2,000,000 Unicode scalar values | Rust validation |
| Parent branch depth | 2,048 messages | Rust validation |

These build-time constants are configurable only within the hard ceilings in
`src-tauri/src/export/mod.rs`; changing one requires boundary tests, resource evidence, and review.
FTR-008 owns a future streaming archive format for data that legitimately exceeds them.

Before committing, Ark presents a dry-run summary with conversation/message counts, maximum
depth, ID conflicts, unavailable-provider mappings, transient-state normalization, and estimated
storage. Imported IDs are remapped to fresh local IDs while original IDs and existing metadata
are retained as provenance. Unknown providers map to the default local provider and are never
created from untrusted export configuration.

The full import is one SQLite transaction. Progress is reported every 100 messages (and at
completion). Cancellation is checked throughout the loop and immediately before final branch
selection; cancellation, validation failure, progress-delivery failure, or a database error rolls
back the new conversation and every message.
