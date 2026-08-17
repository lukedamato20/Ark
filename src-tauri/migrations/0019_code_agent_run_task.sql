-- CODE-007: an Ark Code run needs to know what to investigate. Additive — migration 0018 stays
-- untouched since it may already be applied. Existing rows (none in real use yet) default to ''.
ALTER TABLE code_agent_runs ADD COLUMN task TEXT NOT NULL DEFAULT ''
    CHECK (length(task) <= 8192);
