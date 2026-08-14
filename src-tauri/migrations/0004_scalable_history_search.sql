-- ARC-007: bounded history retrieval, future project filtering, and indexed title/content
-- search. `project_id` is deliberately only a nullable filter seam here; FTR-003 owns the
-- project entity and mutations. Keeping it nullable avoids inventing a project model while
-- allowing the history query contract to remain stable when that feature arrives.
ALTER TABLE conversations ADD COLUMN project_id TEXT;

CREATE INDEX idx_conversations_history
    ON conversations(archived, updated_at DESC, id DESC);
CREATE INDEX idx_conversations_project_history
    ON conversations(project_id, archived, updated_at DESC, id DESC);
CREATE INDEX idx_messages_conversation_parent_created
    ON messages(conversation_id, parent_message_id, created_at DESC, id DESC);

-- One FTS row represents a conversation title and one row represents each message. The
-- unicode61 tokenizer provides Unicode case folding; remove_diacritics=2 makes Latin-diacritic
-- search deterministic while retaining non-Latin tokens. The unindexed identity columns let
-- query results map back to conversations/messages without making UUIDs searchable terms.
CREATE VIRTUAL TABLE conversation_search USING fts5(
    conversation_id UNINDEXED,
    message_id UNINDEXED,
    title,
    content,
    tokenize = 'unicode61 remove_diacritics 2'
);

INSERT INTO conversation_search(conversation_id, message_id, title, content)
SELECT id, NULL, title, '' FROM conversations;

INSERT INTO conversation_search(conversation_id, message_id, title, content)
SELECT conversation_id, id, '', content FROM messages;

CREATE TRIGGER conversations_search_insert AFTER INSERT ON conversations BEGIN
    INSERT INTO conversation_search(conversation_id, message_id, title, content)
    VALUES (new.id, NULL, new.title, '');
END;

CREATE TRIGGER conversations_search_title_update AFTER UPDATE OF title ON conversations BEGIN
    DELETE FROM conversation_search
    WHERE conversation_id = old.id AND message_id IS NULL;
    INSERT INTO conversation_search(conversation_id, message_id, title, content)
    VALUES (new.id, NULL, new.title, '');
END;

CREATE TRIGGER conversations_search_delete AFTER DELETE ON conversations BEGIN
    DELETE FROM conversation_search WHERE conversation_id = old.id;
END;

CREATE TRIGGER messages_search_insert AFTER INSERT ON messages BEGIN
    INSERT INTO conversation_search(conversation_id, message_id, title, content)
    VALUES (new.conversation_id, new.id, '', new.content);
END;

CREATE TRIGGER messages_search_content_update AFTER UPDATE OF content ON messages BEGIN
    DELETE FROM conversation_search WHERE message_id = old.id;
    INSERT INTO conversation_search(conversation_id, message_id, title, content)
    VALUES (new.conversation_id, new.id, '', new.content);
END;

CREATE TRIGGER messages_search_delete AFTER DELETE ON messages BEGIN
    DELETE FROM conversation_search WHERE message_id = old.id;
END;
