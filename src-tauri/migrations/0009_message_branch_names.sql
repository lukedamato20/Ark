-- FTR-005: an optional, user-assigned label for a specific message revision — surfaced in the
-- branch/alternatives switcher so a reader can recognize "the concise one" or "with citations"
-- without re-reading every sibling's content preview. Lives on `messages` (not a separate table)
-- because a "branch" in this schema's append-only design is just a specific message revision —
-- there is no separate branch entity to name. Nullable: `NULL` means unnamed, the existing
-- default presentation (a plain "Response N" ordinal label).
ALTER TABLE messages ADD COLUMN branch_name TEXT;
