-- CODE-007 / ADR 0003: authoritative Ark Code sessions and immutable agent-run attempts.
-- External filesystem, Git, provider, and process I/O never occurs inside these transactions;
-- the rows below persist intent, preconditions, verification evidence, and sequenced events.

CREATE TABLE code_sessions (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    title TEXT NOT NULL CHECK(length(trim(title)) BETWEEN 1 AND 120),
    archived INTEGER NOT NULL DEFAULT 0 CHECK(archived IN (0, 1)),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX idx_code_sessions_project_updated
    ON code_sessions(project_id, archived, updated_at DESC);

CREATE TABLE code_agent_runs (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL REFERENCES code_sessions(id) ON DELETE CASCADE,
    parent_run_id TEXT REFERENCES code_agent_runs(id) ON DELETE SET NULL,
    provider_id TEXT NOT NULL CHECK(length(provider_id) BETWEEN 1 AND 128),
    model_id TEXT NOT NULL CHECK(length(model_id) BETWEEN 1 AND 512),
    repository_path_snapshot TEXT NOT NULL CHECK(length(repository_path_snapshot) BETWEEN 1 AND 32768),
    repository_identity_hash TEXT NOT NULL CHECK(length(repository_identity_hash) = 64),
    state TEXT NOT NULL CHECK(state IN (
        'queued', 'planning', 'awaiting_approval', 'executing_tool', 'observing',
        'completed', 'failed', 'cancelled', 'interrupted'
    )),
    max_steps INTEGER NOT NULL CHECK(max_steps BETWEEN 1 AND 64),
    max_active_ms INTEGER NOT NULL CHECK(max_active_ms BETWEEN 1000 AND 3600000),
    max_tokens INTEGER NOT NULL CHECK(max_tokens BETWEEN 256 AND 1000000),
    max_cost_microunits INTEGER CHECK(max_cost_microunits IS NULL OR max_cost_microunits >= 0),
    steps_used INTEGER NOT NULL DEFAULT 0 CHECK(steps_used BETWEEN 0 AND max_steps),
    active_elapsed_ms INTEGER NOT NULL DEFAULT 0 CHECK(active_elapsed_ms >= 0),
    reserved_tokens INTEGER NOT NULL DEFAULT 0 CHECK(reserved_tokens >= 0),
    actual_tokens INTEGER NOT NULL DEFAULT 0 CHECK(actual_tokens >= 0),
    actual_cost_microunits INTEGER CHECK(actual_cost_microunits IS NULL OR actual_cost_microunits >= 0),
    next_event_sequence INTEGER NOT NULL DEFAULT 0 CHECK(next_event_sequence >= 0),
    executor_lease_id TEXT,
    executor_lease_expires_at TEXT,
    cancel_requested_at TEXT,
    terminal_reason TEXT CHECK(terminal_reason IS NULL OR length(terminal_reason) <= 2048),
    recovery_outcome TEXT CHECK(recovery_outcome IS NULL OR recovery_outcome IN (
        'applied', 'not_applied', 'diverged', 'unknown'
    )),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    completed_at TEXT,
    CHECK(parent_run_id IS NULL OR parent_run_id <> id)
);

CREATE INDEX idx_code_agent_runs_session_created
    ON code_agent_runs(session_id, created_at DESC);
CREATE INDEX idx_code_agent_runs_recovery
    ON code_agent_runs(state, executor_lease_expires_at);

CREATE TABLE code_agent_steps (
    id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL REFERENCES code_agent_runs(id) ON DELETE CASCADE,
    step_index INTEGER NOT NULL CHECK(step_index >= 0),
    state TEXT NOT NULL CHECK(state IN ('reserved', 'dispatched', 'completed', 'failed', 'interrupted')),
    prompt_manifest_json TEXT NOT NULL CHECK(length(prompt_manifest_json) <= 262144),
    reserved_tokens INTEGER NOT NULL CHECK(reserved_tokens >= 0),
    actual_tokens INTEGER CHECK(actual_tokens IS NULL OR actual_tokens >= 0),
    reserved_cost_microunits INTEGER CHECK(reserved_cost_microunits IS NULL OR reserved_cost_microunits >= 0),
    actual_cost_microunits INTEGER CHECK(actual_cost_microunits IS NULL OR actual_cost_microunits >= 0),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE(run_id, step_index)
);

CREATE TABLE code_tool_invocations (
    id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL REFERENCES code_agent_runs(id) ON DELETE CASCADE,
    step_id TEXT NOT NULL REFERENCES code_agent_steps(id) ON DELETE CASCADE,
    provider_call_id TEXT,
    tool_name TEXT NOT NULL CHECK(length(tool_name) BETWEEN 1 AND 128),
    canonical_arguments_json TEXT NOT NULL CHECK(length(canonical_arguments_json) <= 262144),
    call_hash TEXT NOT NULL CHECK(length(call_hash) = 64),
    scope_json TEXT NOT NULL CHECK(length(scope_json) <= 16384),
    idempotency_policy TEXT NOT NULL CHECK(idempotency_policy IN ('idempotent', 'requires_fresh_approval')),
    state TEXT NOT NULL CHECK(state IN (
        'proposed', 'approved', 'executing', 'applied', 'failed', 'denied', 'interrupted'
    )),
    preview TEXT CHECK(preview IS NULL OR length(preview) <= 65536),
    preview_hash TEXT CHECK(preview_hash IS NULL OR length(preview_hash) = 64),
    precondition_hash TEXT CHECK(precondition_hash IS NULL OR length(precondition_hash) = 64),
    approved_call_hash TEXT CHECK(approved_call_hash IS NULL OR length(approved_call_hash) = 64),
    approved_preview_hash TEXT CHECK(approved_preview_hash IS NULL OR length(approved_preview_hash) = 64),
    approved_precondition_hash TEXT CHECK(approved_precondition_hash IS NULL OR length(approved_precondition_hash) = 64),
    approved_by TEXT,
    approved_at TEXT,
    approval_expires_at TEXT,
    execution_lease_id TEXT,
    execution_started_at TEXT,
    verification_plan_json TEXT CHECK(verification_plan_json IS NULL OR length(verification_plan_json) <= 65536),
    verification_outcome TEXT CHECK(verification_outcome IS NULL OR verification_outcome IN (
        'applied', 'not_applied', 'diverged', 'unknown'
    )),
    verification_evidence_json TEXT CHECK(verification_evidence_json IS NULL OR length(verification_evidence_json) <= 131072),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE(run_id, step_id, tool_name, call_hash)
);

CREATE INDEX idx_code_tool_invocations_run_state
    ON code_tool_invocations(run_id, state, created_at);

CREATE TABLE code_observations (
    id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL REFERENCES code_agent_runs(id) ON DELETE CASCADE,
    step_id TEXT NOT NULL REFERENCES code_agent_steps(id) ON DELETE CASCADE,
    invocation_id TEXT REFERENCES code_tool_invocations(id) ON DELETE CASCADE,
    kind TEXT NOT NULL CHECK(kind IN ('tool_result', 'tool_error', 'model_text', 'system')),
    content TEXT NOT NULL CHECK(length(content) <= 131072),
    content_hash TEXT NOT NULL CHECK(length(content_hash) = 64),
    provenance_json TEXT NOT NULL CHECK(length(provenance_json) <= 65536),
    created_at TEXT NOT NULL
);

CREATE INDEX idx_code_observations_run_created
    ON code_observations(run_id, created_at);

CREATE TABLE code_run_events (
    run_id TEXT NOT NULL REFERENCES code_agent_runs(id) ON DELETE CASCADE,
    sequence INTEGER NOT NULL CHECK(sequence >= 0),
    schema_version INTEGER NOT NULL DEFAULT 1 CHECK(schema_version = 1),
    kind TEXT NOT NULL CHECK(length(kind) BETWEEN 1 AND 64),
    state TEXT NOT NULL CHECK(state IN (
        'queued', 'planning', 'awaiting_approval', 'executing_tool', 'observing',
        'completed', 'failed', 'cancelled', 'interrupted'
    )),
    summary TEXT NOT NULL CHECK(length(summary) <= 4096),
    created_at TEXT NOT NULL,
    PRIMARY KEY(run_id, sequence)
);

CREATE TABLE code_idempotency_receipts (
    run_scope TEXT NOT NULL,
    operation TEXT NOT NULL CHECK(length(operation) BETWEEN 1 AND 64),
    idempotency_key TEXT NOT NULL CHECK(length(idempotency_key) BETWEEN 1 AND 128),
    request_hash TEXT NOT NULL CHECK(length(request_hash) = 64),
    response_entity_id TEXT NOT NULL CHECK(length(response_entity_id) BETWEEN 1 AND 128),
    created_at TEXT NOT NULL,
    PRIMARY KEY(run_scope, operation, idempotency_key)
);

