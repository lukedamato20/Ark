-- CODE-005: durable ownership for isolated session repositories, Git checkpoints, and the
-- user-configured verification-command allowlist. No model-provided executable or arguments are
-- persisted here: command definitions are local-user settings referenced by immutable IDs.

CREATE TABLE code_session_repositories (
    session_id TEXT PRIMARY KEY REFERENCES code_sessions(id) ON DELETE CASCADE,
    root_path TEXT NOT NULL UNIQUE CHECK(length(root_path) BETWEEN 1 AND 32768),
    repository_identity_hash TEXT NOT NULL CHECK(length(repository_identity_hash) = 64),
    branch_name TEXT NOT NULL CHECK(length(branch_name) BETWEEN 1 AND 512),
    base_commit_oid TEXT NOT NULL CHECK(length(base_commit_oid) BETWEEN 40 AND 64),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE code_git_checkpoints (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL REFERENCES code_sessions(id) ON DELETE CASCADE,
    run_id TEXT NOT NULL REFERENCES code_agent_runs(id) ON DELETE CASCADE,
    invocation_id TEXT NOT NULL UNIQUE REFERENCES code_tool_invocations(id) ON DELETE CASCADE,
    commit_oid TEXT NOT NULL CHECK(length(commit_oid) BETWEEN 40 AND 64),
    parent_commit_oid TEXT NOT NULL CHECK(length(parent_commit_oid) BETWEEN 40 AND 64),
    tree_oid TEXT NOT NULL CHECK(length(tree_oid) BETWEEN 40 AND 64),
    message TEXT NOT NULL CHECK(length(message) BETWEEN 1 AND 200),
    created_at TEXT NOT NULL,
    UNIQUE(session_id, commit_oid)
);

CREATE INDEX idx_code_git_checkpoints_session_created
    ON code_git_checkpoints(session_id, created_at DESC);

CREATE TABLE code_command_allowlist (
    id TEXT PRIMARY KEY,
    label TEXT NOT NULL CHECK(length(trim(label)) BETWEEN 1 AND 80),
    program TEXT NOT NULL CHECK(length(trim(program)) BETWEEN 1 AND 1024),
    arguments_json TEXT NOT NULL CHECK(length(arguments_json) BETWEEN 2 AND 16384),
    timeout_seconds INTEGER NOT NULL CHECK(timeout_seconds BETWEEN 1 AND 1800),
    enabled INTEGER NOT NULL DEFAULT 1 CHECK(enabled IN (0, 1)),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX idx_code_command_allowlist_enabled_label
    ON code_command_allowlist(enabled, label COLLATE NOCASE);
