-- FTR-002: `pinned_at` (not just a boolean) records when a conversation was pinned, so pin
-- order is deterministic (most-recently-pinned first) wherever it's displayed. Pinned-first
-- display ordering is applied client-side over each already-fetched page rather than folded
-- into the backend's keyset-paginated ORDER BY — see `build_conversation_page_query`'s own
-- comment on why — so no new index is needed here; the existing `idx_conversations_history`/
-- `idx_conversations_project_history` indexes already serve every query this column appears in.
ALTER TABLE conversations ADD COLUMN pinned_at TEXT;
