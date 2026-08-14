-- Adds 'interrupted' as a valid message status, distinct from user-initiated 'cancelled'.
-- SQLite cannot ALTER a CHECK constraint in place, so the table is rebuilt.
PRAGMA foreign_keys=OFF;

BEGIN TRANSACTION;

CREATE TABLE messages_new (
    id TEXT PRIMARY KEY,
    conversation_id TEXT NOT NULL,
    parent_message_id TEXT,
    revision_of_message_id TEXT,
    path_index INTEGER NOT NULL,
    role TEXT NOT NULL CHECK(role IN ('system', 'user', 'assistant', 'tool')),
    content TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'complete' CHECK(status IN ('pending', 'streaming', 'complete', 'failed', 'cancelled', 'interrupted')),
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

INSERT INTO messages_new (
    id, conversation_id, parent_message_id, revision_of_message_id, path_index, role,
    content, status, created_at, updated_at, provider_id, model_id, token_count,
    error_message, metadata_json
)
SELECT
    id, conversation_id, parent_message_id, revision_of_message_id, path_index, role,
    content, status, created_at, updated_at, provider_id, model_id, token_count,
    error_message, metadata_json
FROM messages;

DROP TABLE messages;
ALTER TABLE messages_new RENAME TO messages;

CREATE INDEX IF NOT EXISTS idx_messages_conversation_path ON messages(conversation_id, path_index);
CREATE INDEX IF NOT EXISTS idx_messages_parent ON messages(parent_message_id);
CREATE INDEX IF NOT EXISTS idx_messages_revision ON messages(revision_of_message_id);

COMMIT;

PRAGMA foreign_keys=ON;
