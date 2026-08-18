-- G2/RC-03: add the completion_rejected observation kind.
-- SQLite does not support altering CHECK constraints in place, so the table is recreated.
PRAGMA foreign_keys = OFF;

CREATE TABLE code_observations_v2 (
    id                TEXT NOT NULL PRIMARY KEY,
    run_id            TEXT NOT NULL REFERENCES code_agent_runs(id) ON DELETE CASCADE,
    step_id           TEXT NOT NULL REFERENCES code_agent_steps(id) ON DELETE CASCADE,
    invocation_id     TEXT REFERENCES code_tool_invocations(id) ON DELETE SET NULL,
    kind              TEXT NOT NULL CHECK(kind IN (
                          'tool_result', 'tool_error', 'model_text', 'system',
                          'completion_rejected'
                      )),
    content           TEXT NOT NULL,
    content_hash      TEXT NOT NULL,
    provenance_json   TEXT NOT NULL DEFAULT '{}',
    created_at        TEXT NOT NULL
);

INSERT INTO code_observations_v2 SELECT * FROM code_observations;
DROP TABLE code_observations;
ALTER TABLE code_observations_v2 RENAME TO code_observations;

CREATE INDEX IF NOT EXISTS code_observations_run_id      ON code_observations(run_id);
CREATE INDEX IF NOT EXISTS code_observations_step_id     ON code_observations(step_id);
CREATE INDEX IF NOT EXISTS code_observations_invocation  ON code_observations(invocation_id)
    WHERE invocation_id IS NOT NULL;

PRAGMA foreign_keys = ON;
