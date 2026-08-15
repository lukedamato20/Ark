-- FTR-003: a project groups conversations under a shared name, instructions text, and default
-- provider/model/temperature/max_tokens. `conversations.project_id` already existed as a
-- nullable filter seam with no table for it to reference (see 0004_scalable_history_search.sql's
-- own header comment) — this is that table. No FK constraint is added on `project_id`, matching
-- `conversations.provider_id`'s existing unconstrained-reference style in this schema (enforced
-- instead by application code, e.g. `Database::set_conversation_project` checking the project
-- exists before assigning it).
--
-- `archived_at` (nullable timestamp, not a boolean) follows the same convention `pinned_at`
-- established in 0007_conversation_pinning.sql: an ISO timestamp both records the state and,
-- incidentally, when it changed, for the same one-column price as a boolean.
CREATE TABLE IF NOT EXISTS projects (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    instructions TEXT,
    default_provider_id TEXT,
    default_model_id TEXT,
    default_temperature REAL,
    default_max_tokens INTEGER,
    archived_at TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_projects_archived_updated
    ON projects(archived_at, updated_at DESC, id DESC);
