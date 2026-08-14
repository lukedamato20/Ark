-- ARC-006: `conversations.streaming_enabled` was a per-conversation copy of
-- `providers.streaming_enabled`, snapshotted at conversation-creation time and never read back
-- to make any decision — generation always streams unconditionally regardless of either flag's
-- value. It is a pure, dead duplicate of the provider-level setting (which remains: whether a
-- given provider should use streaming is a real, distinct, provider-scoped preference).
-- SQLite has supported `DROP COLUMN` natively since 3.35.0 — no table-rebuild dance needed here,
-- unlike migration 0002's CHECK-constraint change.
ALTER TABLE conversations DROP COLUMN streaming_enabled;
