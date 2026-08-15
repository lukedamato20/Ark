-- FTR-003: personas are the second half of this plan item, deferred out of migration 0008
-- (projects) to keep that pass reviewable on its own. A persona is a reusable, named instruction
-- identity (e.g. "terse code reviewer") a conversation can be assigned to, distinct from a
-- project: a project groups conversations by *subject*, a persona defines *how the assistant
-- behaves* — the two are independently assignable to the same conversation.
--
-- Unlike a project's `instructions` (a plain mutable column), a persona's prompt content is
-- versioned and immutable per acceptance criterion 2 ("prompt versions are immutable... and do
-- not silently alter past provenance"): editing a persona's instructions never rewrites the row
-- a past generation's provenance points at, it inserts a new `persona_versions` row and moves
-- `personas.current_version_id` to it. `personas.name` is ordinary mutable metadata (identity,
-- not prompt content) and is updated in place, matching how a project's `name` is never
-- versioned either.
--
-- No FK constraints anywhere here, matching this schema's existing unconstrained-reference style
-- for `conversations.provider_id`/`project_id` — enforced instead by application code
-- (`Database::set_conversation_persona` checks the persona exists first; `current_version_id` is
-- only ever set by `Database::create_persona`/`update_persona`, which insert the version it
-- points at in the same transaction).
CREATE TABLE IF NOT EXISTS personas (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    current_version_id TEXT NOT NULL,
    archived_at TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS persona_versions (
    id TEXT PRIMARY KEY,
    persona_id TEXT NOT NULL,
    version_number INTEGER NOT NULL,
    instructions TEXT NOT NULL,
    default_temperature REAL,
    default_max_tokens INTEGER,
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_persona_versions_persona
    ON persona_versions(persona_id, version_number DESC);

CREATE INDEX IF NOT EXISTS idx_personas_archived_name
    ON personas(archived_at, name ASC);

-- A conversation's assigned persona, independent of its assigned project (see module doc above).
-- `NULL` means unassigned, matching `project_id`'s existing convention.
ALTER TABLE conversations ADD COLUMN persona_id TEXT;
