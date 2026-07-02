CREATE TABLE IF NOT EXISTS schema_migrations (
    version INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    applied_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS conversations (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    provider_id TEXT,
    model_id TEXT,
    current_message_id TEXT,
    system_prompt TEXT,
    temperature REAL,
    max_tokens INTEGER,
    streaming_enabled INTEGER NOT NULL DEFAULT 1,
    archived INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS messages (
    id TEXT PRIMARY KEY,
    conversation_id TEXT NOT NULL,
    parent_message_id TEXT,
    revision_of_message_id TEXT,
    path_index INTEGER NOT NULL,
    role TEXT NOT NULL CHECK(role IN ('system', 'user', 'assistant', 'tool')),
    content TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'complete' CHECK(status IN ('pending', 'streaming', 'complete', 'failed', 'cancelled')),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    provider_id TEXT,
    model_id TEXT,
    token_count INTEGER,
    error_message TEXT,
    metadata_json TEXT,
    FOREIGN KEY (conversation_id) REFERENCES conversations(id) ON DELETE CASCADE,
    FOREIGN KEY (parent_message_id) REFERENCES messages(id) ON DELETE SET NULL,
    FOREIGN KEY (revision_of_message_id) REFERENCES messages(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_messages_conversation_path ON messages(conversation_id, path_index);
CREATE INDEX IF NOT EXISTS idx_messages_parent ON messages(parent_message_id);
CREATE INDEX IF NOT EXISTS idx_messages_revision ON messages(revision_of_message_id);

CREATE TABLE IF NOT EXISTS providers (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    provider_type TEXT NOT NULL,
    base_url TEXT,
    api_key_ref TEXT,
    default_model_id TEXT,
    default_temperature REAL,
    default_max_tokens INTEGER,
    streaming_enabled INTEGER NOT NULL DEFAULT 1,
    is_local INTEGER NOT NULL DEFAULT 1,
    is_enabled INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS models (
    id TEXT PRIMARY KEY,
    provider_id TEXT NOT NULL,
    name TEXT NOT NULL,
    display_name TEXT,
    context_window INTEGER,
    supports_streaming INTEGER NOT NULL DEFAULT 1,
    supports_tools INTEGER NOT NULL DEFAULT 0,
    supports_vision INTEGER NOT NULL DEFAULT 0,
    supports_embeddings INTEGER NOT NULL DEFAULT 0,
    is_available INTEGER NOT NULL DEFAULT 1,
    last_seen_at TEXT,
    metadata_json TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (provider_id) REFERENCES providers(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_models_provider ON models(provider_id);

CREATE TABLE IF NOT EXISTS app_settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
