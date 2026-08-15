-- CMP-004: a 4th secret-reference family for `secret_store.rs`, alongside the existing
-- provider/workspace-key/companion-api-token families. One row per tool_id -- today only
-- "web_search" -- keyed generically so a future secret-scoped tool needs no schema change,
-- only a new `built_in_tools()` entry. `secret_ref` is the opaque `tool-secret:v1:<uuid>`
-- reference into the OS keyring; the actual credential value never lives in this table.
CREATE TABLE IF NOT EXISTS tool_secrets (
    tool_id TEXT PRIMARY KEY,
    secret_ref TEXT NOT NULL
);
