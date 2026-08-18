-- CODE-007: bounded, durable in-progress assistant text. Completed assistant text remains an
-- immutable observation; this column exists only so a reopened/refetched active run can render
-- provider deltas without treating process events as authoritative state.

ALTER TABLE code_agent_steps
    ADD COLUMN streaming_text TEXT CHECK(streaming_text IS NULL OR length(streaming_text) <= 131072);
