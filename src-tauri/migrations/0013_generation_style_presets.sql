-- UX: Ark-level "response style"/"tone" behavioral presets — distinct from real provider
-- parameters like temperature (see generation.rs's `response_style_instruction`/`tone_instruction`
-- for the fixed mapping to instruction text). Resolve at the same three tiers as `system_prompt`/
-- `instructions` already do (conversation, then persona, then project — `generation.rs`'s
-- `resolve_text_settings`), so these two nullable columns are added to exactly the same three
-- tables `system_prompt`/`instructions` already live on: `conversations`, `projects`, and
-- `persona_versions` (not the mutable `personas` row — a persona's behavioral preset is versioned
-- immutable content, matching `instructions`/`default_temperature`/`default_max_tokens`'s existing
-- placement, not mutable identity metadata like `name`).
--
-- CHECK constraints are defense in depth alongside `validation::validate_response_style`/
-- `validate_tone`'s own allow-lists in Rust — SQLite enforces the same six/five-value sets at the
-- storage layer so a bug bypassing Rust validation still cannot persist a nonsense value.
ALTER TABLE conversations ADD COLUMN response_style TEXT
    CHECK (response_style IS NULL OR response_style IN ('balanced', 'concise', 'detailed', 'explanatory', 'technical', 'creative'));
ALTER TABLE conversations ADD COLUMN tone TEXT
    CHECK (tone IS NULL OR tone IN ('neutral', 'professional', 'friendly', 'direct', 'casual'));

ALTER TABLE projects ADD COLUMN response_style TEXT
    CHECK (response_style IS NULL OR response_style IN ('balanced', 'concise', 'detailed', 'explanatory', 'technical', 'creative'));
ALTER TABLE projects ADD COLUMN tone TEXT
    CHECK (tone IS NULL OR tone IN ('neutral', 'professional', 'friendly', 'direct', 'casual'));

ALTER TABLE persona_versions ADD COLUMN response_style TEXT
    CHECK (response_style IS NULL OR response_style IN ('balanced', 'concise', 'detailed', 'explanatory', 'technical', 'creative'));
ALTER TABLE persona_versions ADD COLUMN tone TEXT
    CHECK (tone IS NULL OR tone IN ('neutral', 'professional', 'friendly', 'direct', 'casual'));
