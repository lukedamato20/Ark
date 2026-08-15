-- CMP-001: a text-file attachment a user attaches to an outgoing message. Scoped deliberately to
-- text content only in this pass (no images/vision — see implementation-plan.md's CMP-001 entry
-- for why that is a separate, larger lift touching the shared `ChatMessage` DTO both provider
-- adapters use). Unlike `projects`/`personas` (unconstrained references, soft-unassign
-- semantics), an attachment has hard ownership of a conversation and, once sent, a specific
-- message — so this uses real `FOREIGN KEY ... ON DELETE CASCADE` constraints, matching how
-- `messages` itself already references `conversations` (see `0001_mvp.sql`), rather than
-- inventing a new unconstrained style for a case that doesn't need one.
--
-- Content is stored directly as a TEXT column, not a filesystem path or BLOB — this codebase has
-- no existing precedent for storing file bytes anywhere (see the CMP-001 investigation in
-- implementation-plan.md), and for text-only attachments the extracted text *is* the useful
-- payload; the original file's exact bytes are not preserved for re-download. Storing it as a
-- normal column means backup/restore/export need no new code at all — they already copy the
-- whole SQLite database, which now includes attachments for free.
--
-- `message_id` starts `NULL`: an attachment is uploaded ("staged") before the message it will be
-- sent with even exists, so the user can preview and remove it pre-send. `send_chat_message`
-- links it to the newly created user message inside the same transaction. A staged attachment
-- (`message_id IS NULL`) can be deleted directly by the user (the "remove before send" of
-- CMP-001's own acceptance criteria); one already linked to a sent message is not offered for
-- deletion, matching the append-only-history posture the rest of this schema already has for
-- messages themselves.
CREATE TABLE IF NOT EXISTS attachments (
    id TEXT PRIMARY KEY,
    conversation_id TEXT NOT NULL,
    message_id TEXT,
    file_name TEXT NOT NULL,
    byte_size INTEGER NOT NULL,
    sha256 TEXT NOT NULL,
    content TEXT NOT NULL,
    created_at TEXT NOT NULL,
    FOREIGN KEY (conversation_id) REFERENCES conversations(id) ON DELETE CASCADE,
    FOREIGN KEY (message_id) REFERENCES messages(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_attachments_conversation ON attachments(conversation_id, created_at);
CREATE INDEX IF NOT EXISTS idx_attachments_message ON attachments(message_id);
