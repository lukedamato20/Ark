use crate::chat::{BranchAlternative, Conversation, Message};
use crate::config::{
    BUILT_IN_PROVIDER_BASE_URL, BUILT_IN_PROVIDER_ID, BUILT_IN_PROVIDER_NAME,
    BUILT_IN_PROVIDER_TYPE, DEFAULT_MAX_TOKENS, DEFAULT_OLLAMA_BASE_URL, DEFAULT_PROVIDER_ID,
    DEFAULT_PROVIDER_NAME, DEFAULT_PROVIDER_TYPE, DEFAULT_TEMPERATURE,
    LOCAL_INFERENCE_HOST_BASE_URL, LOCAL_INFERENCE_HOST_PROVIDER_ID,
    LOCAL_INFERENCE_HOST_PROVIDER_NAME, LOCAL_INFERENCE_HOST_PROVIDER_TYPE,
};
use crate::errors::AppError;
use crate::providers::{ModelInfo, ProviderConfig};
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;
use uuid::Uuid;

const MVP_MIGRATION: &str = include_str!("../../migrations/0001_mvp.sql");

pub struct Database {
    connection: Connection,
}

impl Database {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, AppError> {
        let connection = Connection::open(path)?;
        connection.pragma_update(None, "foreign_keys", "ON")?;

        let db = Self { connection };
        db.run_migrations()?;
        db.seed_defaults()?;
        Ok(db)
    }

    fn run_migrations(&self) -> Result<(), AppError> {
        self.connection.execute_batch(MVP_MIGRATION)?;
        self.connection.execute(
            "INSERT OR IGNORE INTO schema_migrations(version, name, applied_at) VALUES (?1, ?2, ?3)",
            params![1_i64, "0001_mvp", now()],
        )?;
        Ok(())
    }

    fn seed_defaults(&self) -> Result<(), AppError> {
        let timestamp = now();
        self.seed_provider(
            DEFAULT_PROVIDER_ID,
            DEFAULT_PROVIDER_NAME,
            DEFAULT_PROVIDER_TYPE,
            DEFAULT_OLLAMA_BASE_URL,
            &timestamp,
        )?;
        self.seed_provider(
            LOCAL_INFERENCE_HOST_PROVIDER_ID,
            LOCAL_INFERENCE_HOST_PROVIDER_NAME,
            LOCAL_INFERENCE_HOST_PROVIDER_TYPE,
            LOCAL_INFERENCE_HOST_BASE_URL,
            &timestamp,
        )?;
        self.seed_provider(
            BUILT_IN_PROVIDER_ID,
            BUILT_IN_PROVIDER_NAME,
            BUILT_IN_PROVIDER_TYPE,
            BUILT_IN_PROVIDER_BASE_URL,
            &timestamp,
        )?;
        Ok(())
    }

    fn seed_provider(
        &self,
        id: &str,
        name: &str,
        provider_type: &str,
        base_url: &str,
        timestamp: &str,
    ) -> Result<(), AppError> {
        let existing: Option<String> = self
            .connection
            .query_row("SELECT id FROM providers WHERE id = ?1", params![id], |row| row.get(0))
            .optional()?;

        if existing.is_none() {
            self.connection.execute(
                "INSERT INTO providers (
                    id, name, provider_type, base_url, default_temperature, default_max_tokens,
                    streaming_enabled, is_local, is_enabled, created_at, updated_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 1, 1, 1, ?7, ?7)",
                params![id, name, provider_type, base_url, DEFAULT_TEMPERATURE, DEFAULT_MAX_TOKENS, timestamp],
            )?;
        }

        Ok(())
    }

    pub fn list_conversations(&self) -> Result<Vec<Conversation>, AppError> {
        let mut statement = self.connection.prepare(
            "SELECT id, title, created_at, updated_at, provider_id, model_id, current_message_id,
                system_prompt, temperature, max_tokens, streaming_enabled, archived
             FROM conversations
             WHERE archived = 0
             ORDER BY updated_at DESC",
        )?;

        let rows = statement.query_map([], map_conversation)?;
        collect_rows(rows)
    }

    pub fn get_conversation(&self, id: &str) -> Result<Conversation, AppError> {
        self.connection
            .query_row(
                "SELECT id, title, created_at, updated_at, provider_id, model_id, current_message_id,
                    system_prompt, temperature, max_tokens, streaming_enabled, archived
                 FROM conversations
                 WHERE id = ?1",
                params![id],
                map_conversation,
            )
            .optional()?
            .ok_or_else(|| AppError::not_found("Conversation"))
    }

    pub fn create_conversation(&self, title: Option<String>) -> Result<Conversation, AppError> {
        let timestamp = now();
        let id = Uuid::new_v4().to_string();
        let provider = self.get_provider(DEFAULT_PROVIDER_ID)?;
        let conversation_title = title
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "New conversation".to_string());

        self.connection.execute(
            "INSERT INTO conversations (
                id, title, created_at, updated_at, provider_id, model_id, temperature, max_tokens, streaming_enabled, archived
            ) VALUES (?1, ?2, ?3, ?3, ?4, ?5, ?6, ?7, ?8, 0)",
            params![
                id,
                conversation_title,
                timestamp,
                provider.id,
                provider.default_model_id,
                provider.default_temperature,
                provider.default_max_tokens,
                provider.streaming_enabled as i64,
            ],
        )?;

        self.get_conversation(&id)
    }

    pub fn rename_conversation(&self, id: &str, title: &str) -> Result<Conversation, AppError> {
        let trimmed = title.trim();
        if trimmed.is_empty() {
            return Err(AppError::invalid_input("Conversation title cannot be empty."));
        }

        self.connection.execute(
            "UPDATE conversations SET title = ?1, updated_at = ?2 WHERE id = ?3",
            params![trimmed, now(), id],
        )?;

        self.get_conversation(id)
    }

    pub fn delete_conversation(&self, id: &str) -> Result<(), AppError> {
        let affected = self
            .connection
            .execute("DELETE FROM conversations WHERE id = ?1", params![id])?;
        if affected == 0 {
            return Err(AppError::not_found("Conversation"));
        }
        Ok(())
    }

    pub fn get_active_messages(&self, conversation_id: &str) -> Result<Vec<Message>, AppError> {
        let conversation = self.get_conversation(conversation_id)?;
        let Some(current_message_id) = conversation.current_message_id else {
            return Ok(Vec::new());
        };

        self.get_message_path(&current_message_id)
    }

    pub fn get_message_path(&self, leaf_message_id: &str) -> Result<Vec<Message>, AppError> {
        let mut messages = Vec::new();
        let mut next_id = Some(leaf_message_id.to_string());

        while let Some(message_id) = next_id {
            let message = self.get_message(&message_id)?;
            next_id = message.parent_message_id.clone();
            messages.push(message);
        }

        messages.reverse();
        Ok(messages)
    }

    pub fn get_all_conversation_messages(&self, conversation_id: &str) -> Result<Vec<Message>, AppError> {
        let mut statement = self.connection.prepare(
            "SELECT id, conversation_id, parent_message_id, revision_of_message_id, path_index, role,
                content, status, created_at, updated_at, provider_id, model_id, token_count,
                error_message, metadata_json
             FROM messages
             WHERE conversation_id = ?1
             ORDER BY path_index ASC, created_at ASC",
        )?;

        let rows = statement.query_map(params![conversation_id], map_message)?;
        collect_rows(rows)
    }

    pub fn get_assistant_alternatives(
        &self,
        conversation_id: &str,
        message_id: &str,
    ) -> Result<Vec<BranchAlternative>, AppError> {
        let message = self.get_message(message_id)?;
        if message.conversation_id != conversation_id || message.role != "assistant" {
            return Err(AppError::invalid_input(
                "Only assistant messages in this conversation can have alternatives.",
            ));
        }

        let parent_message_id = message
            .parent_message_id
            .as_deref()
            .ok_or_else(|| AppError::invalid_input("Assistant message has no parent message."))?;
        let active_ids = self
            .get_active_messages(conversation_id)?
            .into_iter()
            .map(|message| message.id)
            .collect::<Vec<_>>();

        let mut statement = self.connection.prepare(
            "SELECT id, revision_of_message_id, created_at, status, content,
                    EXISTS(SELECT 1 FROM messages c WHERE c.parent_message_id = messages.id) AS has_descendants
             FROM messages
             WHERE conversation_id = ?1 AND parent_message_id = ?2 AND role = 'assistant'
             ORDER BY path_index ASC, created_at ASC",
        )?;

        let rows = statement.query_map(params![conversation_id, parent_message_id], |row| {
            let message_id: String = row.get(0)?;
            let content: String = row.get(4)?;
            Ok(BranchAlternative {
                is_active: active_ids.iter().any(|id| id == &message_id),
                message_id,
                revision_of_message_id: row.get(1)?,
                created_at: row.get(2)?,
                status: row.get(3)?,
                content_preview: message_preview(&content),
                has_descendants: row.get(5)?,
            })
        })?;

        collect_rows(rows)
    }

    fn find_branch_leaf(&self, start_message_id: &str) -> Result<String, AppError> {
        let mut current_id = start_message_id.to_string();
        // Walk forward along the newest non-revision child at each step.
        for _ in 0..100 {
            let child: Option<String> = self
                .connection
                .query_row(
                    "SELECT id FROM messages
                     WHERE parent_message_id = ?1 AND revision_of_message_id IS NULL
                     ORDER BY created_at DESC LIMIT 1",
                    params![current_id],
                    |row| row.get(0),
                )
                .optional()?;
            match child {
                Some(id) => current_id = id,
                None => break,
            }
        }
        Ok(current_id)
    }

    pub fn switch_active_branch(
        &self,
        conversation_id: &str,
        message_id: &str,
    ) -> Result<Vec<Message>, AppError> {
        let conversation = self.get_conversation(conversation_id)?;
        let message = self.get_message(message_id)?;
        if message.conversation_id != conversation_id || message.role != "assistant" {
            return Err(AppError::invalid_input(
                "Only assistant messages in this conversation can be selected as a branch.",
            ));
        }

        // Walk to the deepest descendant so the full branch history is shown.
        let leaf_id = self.find_branch_leaf(message_id)?;
        let leaf = if leaf_id != message_id {
            self.get_message(&leaf_id)?
        } else {
            message.clone()
        };

        let provider_id = leaf
            .provider_id
            .as_deref()
            .or(message.provider_id.as_deref())
            .or(conversation.provider_id.as_deref())
            .unwrap_or(DEFAULT_PROVIDER_ID);
        let model_id = leaf
            .model_id
            .as_deref()
            .or(message.model_id.as_deref())
            .or(conversation.model_id.as_deref())
            .unwrap_or("");

        self.set_conversation_current_message(conversation_id, &leaf_id, provider_id, model_id)?;
        self.get_active_messages(conversation_id)
    }

    pub fn get_message(&self, id: &str) -> Result<Message, AppError> {
        self.connection
            .query_row(
                "SELECT id, conversation_id, parent_message_id, revision_of_message_id, path_index, role,
                    content, status, created_at, updated_at, provider_id, model_id, token_count,
                    error_message, metadata_json
                 FROM messages
                 WHERE id = ?1",
                params![id],
                map_message,
            )
            .optional()?
            .ok_or_else(|| AppError::not_found("Message"))
    }

    pub fn append_message(
        &self,
        conversation_id: &str,
        parent_message_id: Option<&str>,
        revision_of_message_id: Option<&str>,
        role: &str,
        content: &str,
        status: &str,
        provider_id: Option<&str>,
        model_id: Option<&str>,
    ) -> Result<Message, AppError> {
        let timestamp = now();
        let id = Uuid::new_v4().to_string();
        let path_index = self.next_path_index(conversation_id)?;

        self.connection.execute(
            "INSERT INTO messages (
                id, conversation_id, parent_message_id, revision_of_message_id, path_index, role,
                content, status, created_at, updated_at, provider_id, model_id
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9, ?10, ?11)",
            params![
                id,
                conversation_id,
                parent_message_id,
                revision_of_message_id,
                path_index,
                role,
                content,
                status,
                timestamp,
                provider_id,
                model_id,
            ],
        )?;

        self.get_message(&id)
    }

    pub fn append_to_message_content(&self, message_id: &str, delta: &str) -> Result<String, AppError> {
        self.connection.execute(
            "UPDATE messages SET content = content || ?1, updated_at = ?2 WHERE id = ?3",
            params![delta, now(), message_id],
        )?;

        let content = self.connection.query_row(
            "SELECT content FROM messages WHERE id = ?1",
            params![message_id],
            |row| row.get(0),
        )?;

        Ok(content)
    }

    pub fn finish_message(
        &self,
        message_id: &str,
        status: &str,
        error_message: Option<&str>,
        input_tokens: Option<i64>,
        output_tokens: Option<i64>,
    ) -> Result<(), AppError> {
        let token_count = match (input_tokens, output_tokens) {
            (Some(input), Some(output)) => Some(input + output),
            (Some(input), None) => Some(input),
            (None, Some(output)) => Some(output),
            (None, None) => None,
        };

        self.connection.execute(
            "UPDATE messages
             SET status = ?1, error_message = ?2, token_count = ?3, updated_at = ?4
             WHERE id = ?5",
            params![status, error_message, token_count, now(), message_id],
        )?;

        Ok(())
    }

    pub fn set_message_metadata_json(&self, message_id: &str, metadata_json: &str) -> Result<(), AppError> {
        self.connection.execute(
            "UPDATE messages SET metadata_json = ?1, updated_at = ?2 WHERE id = ?3",
            params![metadata_json, now(), message_id],
        )?;
        Ok(())
    }

    pub fn set_conversation_current_message(
        &self,
        conversation_id: &str,
        message_id: &str,
        provider_id: &str,
        model_id: &str,
    ) -> Result<(), AppError> {
        self.connection.execute(
            "UPDATE conversations
             SET current_message_id = ?1, provider_id = ?2, model_id = ?3, updated_at = ?4
             WHERE id = ?5",
            params![message_id, provider_id, model_id, now(), conversation_id],
        )?;
        Ok(())
    }

    pub fn maybe_title_conversation(&self, conversation_id: &str, content: &str) -> Result<(), AppError> {
        let conversation = self.get_conversation(conversation_id)?;
        if conversation.title != "New conversation" {
            return Ok(());
        }

        let title = content
            .split_whitespace()
            .take(8)
            .collect::<Vec<_>>()
            .join(" ");
        let title = if title.is_empty() {
            "New conversation".to_string()
        } else if title.len() > 64 {
            format!("{}...", &title[..61])
        } else {
            title
        };

        self.rename_conversation(conversation_id, &title)?;
        Ok(())
    }

    pub fn list_providers(&self) -> Result<Vec<ProviderConfig>, AppError> {
        let mut statement = self.connection.prepare(
            "SELECT id, name, provider_type, base_url, api_key_ref, default_model_id,
                default_temperature, default_max_tokens, streaming_enabled, is_local, is_enabled,
                created_at, updated_at
             FROM providers
             ORDER BY name ASC",
        )?;
        let rows = statement.query_map([], map_provider)?;
        collect_rows(rows)
    }

    pub fn get_provider(&self, provider_id: &str) -> Result<ProviderConfig, AppError> {
        self.connection
            .query_row(
                "SELECT id, name, provider_type, base_url, api_key_ref, default_model_id,
                    default_temperature, default_max_tokens, streaming_enabled, is_local, is_enabled,
                    created_at, updated_at
                 FROM providers
                 WHERE id = ?1",
                params![provider_id],
                map_provider,
            )
            .optional()?
            .ok_or_else(|| AppError::not_found("Provider"))
    }

    pub fn update_provider(
        &self,
        provider_id: &str,
        base_url: &str,
        default_model_id: Option<&str>,
        temperature: Option<f64>,
        max_tokens: Option<i64>,
        streaming_enabled: bool,
    ) -> Result<ProviderConfig, AppError> {
        if base_url.trim().is_empty() {
            return Err(AppError::invalid_input("Provider base URL cannot be empty."));
        }

        self.connection.execute(
            "UPDATE providers
             SET base_url = ?1, default_model_id = ?2, default_temperature = ?3,
                default_max_tokens = ?4, streaming_enabled = ?5, updated_at = ?6
             WHERE id = ?7",
            params![
                base_url.trim(),
                default_model_id,
                temperature,
                max_tokens,
                streaming_enabled as i64,
                now(),
                provider_id,
            ],
        )?;

        self.get_provider(provider_id)
    }

    pub fn update_provider_base_url(&self, provider_id: &str, base_url: &str) -> Result<(), AppError> {
        self.connection.execute(
            "UPDATE providers SET base_url = ?1, updated_at = ?2 WHERE id = ?3",
            params![base_url, now(), provider_id],
        )?;
        Ok(())
    }

    pub fn list_models(&self, provider_id: &str) -> Result<Vec<ModelInfo>, AppError> {
        let mut statement = self.connection.prepare(
            "SELECT id, provider_id, name, display_name, context_window, supports_streaming,
                supports_tools, supports_vision, supports_embeddings, is_available, last_seen_at,
                metadata_json, created_at, updated_at
             FROM models
             WHERE provider_id = ?1
             ORDER BY name ASC",
        )?;

        let rows = statement.query_map(params![provider_id], map_model)?;
        collect_rows(rows)
    }

    pub fn list_all_models(&self) -> Result<Vec<ModelInfo>, AppError> {
        let mut statement = self.connection.prepare(
            "SELECT id, provider_id, name, display_name, context_window, supports_streaming,
                supports_tools, supports_vision, supports_embeddings, is_available, last_seen_at,
                metadata_json, created_at, updated_at
             FROM models
             ORDER BY provider_id ASC, name ASC",
        )?;

        let rows = statement.query_map([], map_model)?;
        collect_rows(rows)
    }

    pub fn upsert_models(&self, provider_id: &str, models: &[ModelInfo]) -> Result<(), AppError> {
        let timestamp = now();
        self.connection.execute(
            "UPDATE models SET is_available = 0, updated_at = ?1 WHERE provider_id = ?2",
            params![timestamp, provider_id],
        )?;

        for model in models {
            self.connection.execute(
                "INSERT INTO models (
                    id, provider_id, name, display_name, context_window, supports_streaming,
                    supports_tools, supports_vision, supports_embeddings, is_available,
                    last_seen_at, metadata_json, created_at, updated_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
                ON CONFLICT(id) DO UPDATE SET
                    name = excluded.name,
                    display_name = excluded.display_name,
                    context_window = excluded.context_window,
                    supports_streaming = excluded.supports_streaming,
                    supports_tools = excluded.supports_tools,
                    supports_vision = excluded.supports_vision,
                    supports_embeddings = excluded.supports_embeddings,
                    is_available = excluded.is_available,
                    last_seen_at = excluded.last_seen_at,
                    metadata_json = excluded.metadata_json,
                    updated_at = excluded.updated_at",
                params![
                    model.id,
                    model.provider_id,
                    model.name,
                    model.display_name,
                    model.context_window,
                    model.supports_streaming as i64,
                    model.supports_tools as i64,
                    model.supports_vision as i64,
                    model.supports_embeddings as i64,
                    model.is_available as i64,
                    model.last_seen_at,
                    model.metadata_json,
                    model.created_at,
                    model.updated_at,
                ],
            )?;
        }

        if let Some(first_model) = models.first() {
            let provider = self.get_provider(provider_id)?;
            if provider.default_model_id.is_none() {
                self.connection.execute(
                    "UPDATE providers SET default_model_id = ?1, updated_at = ?2 WHERE id = ?3",
                    params![first_model.name, now(), provider_id],
                )?;
            }
        }

        Ok(())
    }

    pub fn get_setting(&self, key: &str) -> Result<Option<String>, AppError> {
        self.connection
            .query_row(
                "SELECT value FROM app_settings WHERE key = ?1",
                params![key],
                |row| row.get(0),
            )
            .optional()
            .map_err(AppError::from)
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<(), AppError> {
        self.connection.execute(
            "INSERT INTO app_settings(key, value, updated_at)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
            params![key, value, now()],
        )?;
        Ok(())
    }

    fn next_path_index(&self, conversation_id: &str) -> Result<i64, AppError> {
        let current: Option<i64> = self
            .connection
            .query_row(
                "SELECT MAX(path_index) FROM messages WHERE conversation_id = ?1",
                params![conversation_id],
                |row| row.get(0),
            )
            .optional()?
            .flatten();

        Ok(current.unwrap_or(0) + 1)
    }
}

pub fn now() -> String {
    Utc::now().to_rfc3339()
}

fn message_preview(content: &str) -> String {
    let normalized = content.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.is_empty() {
        return "Empty response".to_string();
    }

    let mut preview = normalized.chars().take(140).collect::<String>();
    if normalized.chars().count() > 140 {
        preview.push_str("...");
    }
    preview
}

fn collect_rows<T, F>(rows: rusqlite::MappedRows<'_, F>) -> Result<Vec<T>, AppError>
where
    F: FnMut(&rusqlite::Row<'_>) -> rusqlite::Result<T>,
{
    let mut output = Vec::new();
    for row in rows {
        output.push(row?);
    }
    Ok(output)
}

fn map_conversation(row: &rusqlite::Row<'_>) -> rusqlite::Result<Conversation> {
    Ok(Conversation {
        id: row.get(0)?,
        title: row.get(1)?,
        created_at: row.get(2)?,
        updated_at: row.get(3)?,
        provider_id: row.get(4)?,
        model_id: row.get(5)?,
        current_message_id: row.get(6)?,
        system_prompt: row.get(7)?,
        temperature: row.get(8)?,
        max_tokens: row.get(9)?,
        streaming_enabled: row.get::<_, i64>(10)? != 0,
        archived: row.get::<_, i64>(11)? != 0,
    })
}

fn map_message(row: &rusqlite::Row<'_>) -> rusqlite::Result<Message> {
    Ok(Message {
        id: row.get(0)?,
        conversation_id: row.get(1)?,
        parent_message_id: row.get(2)?,
        revision_of_message_id: row.get(3)?,
        path_index: row.get(4)?,
        role: row.get(5)?,
        content: row.get(6)?,
        status: row.get(7)?,
        created_at: row.get(8)?,
        updated_at: row.get(9)?,
        provider_id: row.get(10)?,
        model_id: row.get(11)?,
        token_count: row.get(12)?,
        error_message: row.get(13)?,
        metadata_json: row.get(14)?,
    })
}

fn map_provider(row: &rusqlite::Row<'_>) -> rusqlite::Result<ProviderConfig> {
    Ok(ProviderConfig {
        id: row.get(0)?,
        name: row.get(1)?,
        provider_type: row.get(2)?,
        base_url: row.get(3)?,
        api_key_ref: row.get(4)?,
        default_model_id: row.get(5)?,
        default_temperature: row.get(6)?,
        default_max_tokens: row.get(7)?,
        streaming_enabled: row.get::<_, i64>(8)? != 0,
        is_local: row.get::<_, i64>(9)? != 0,
        is_enabled: row.get::<_, i64>(10)? != 0,
        created_at: row.get(11)?,
        updated_at: row.get(12)?,
    })
}

fn map_model(row: &rusqlite::Row<'_>) -> rusqlite::Result<ModelInfo> {
    Ok(ModelInfo {
        id: row.get(0)?,
        provider_id: row.get(1)?,
        name: row.get(2)?,
        display_name: row.get(3)?,
        context_window: row.get(4)?,
        supports_streaming: row.get::<_, i64>(5)? != 0,
        supports_tools: row.get::<_, i64>(6)? != 0,
        supports_vision: row.get::<_, i64>(7)? != 0,
        supports_embeddings: row.get::<_, i64>(8)? != 0,
        is_available: row.get::<_, i64>(9)? != 0,
        last_seen_at: row.get(10)?,
        metadata_json: row.get(11)?,
        created_at: row.get(12)?,
        updated_at: row.get(13)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn test_db() -> (Database, std::path::PathBuf) {
        let path = std::env::temp_dir().join(format!("ark-test-{}.sqlite3", Uuid::new_v4()));
        let db = Database::open(&path).expect("database opens");
        (db, path)
    }

    #[test]
    fn creates_renames_and_deletes_conversation() {
        let (db, path) = test_db();

        let created = db
            .create_conversation(Some("Initial".to_string()))
            .expect("conversation created");
        assert_eq!(created.title, "Initial");

        let renamed = db
            .rename_conversation(&created.id, "Renamed")
            .expect("conversation renamed");
        assert_eq!(renamed.title, "Renamed");

        let conversations = db.list_conversations().expect("conversations list");
        assert_eq!(conversations.len(), 1);

        db.delete_conversation(&created.id).expect("conversation deleted");
        assert!(db.list_conversations().expect("conversations list").is_empty());

        drop(db);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn loads_active_append_only_branch_path() {
        let (db, path) = test_db();
        let conversation = db
            .create_conversation(Some("Branch".to_string()))
            .expect("conversation created");

        let user = db
            .append_message(
                &conversation.id,
                None,
                None,
                "user",
                "Original question",
                "complete",
                Some(DEFAULT_PROVIDER_ID),
                Some("llama3.2:latest"),
            )
            .expect("user message");
        let first_assistant = db
            .append_message(
                &conversation.id,
                Some(&user.id),
                None,
                "assistant",
                "First answer",
                "complete",
                Some(DEFAULT_PROVIDER_ID),
                Some("llama3.2:latest"),
            )
            .expect("assistant message");

        let regenerated = db
            .append_message(
                &conversation.id,
                Some(&user.id),
                Some(&first_assistant.id),
                "assistant",
                "Second answer",
                "complete",
                Some(DEFAULT_PROVIDER_ID),
                Some("llama3.2:latest"),
            )
            .expect("regenerated assistant message");
        db.set_conversation_current_message(
            &conversation.id,
            &regenerated.id,
            DEFAULT_PROVIDER_ID,
            "llama3.2:latest",
        )
        .expect("current branch updated");

        let active = db
            .get_active_messages(&conversation.id)
            .expect("active messages");
        assert_eq!(active.len(), 2);
        assert_eq!(active[0].content, "Original question");
        assert_eq!(active[1].content, "Second answer");
        assert_eq!(active[1].revision_of_message_id.as_deref(), Some(first_assistant.id.as_str()));

        let all_messages = db
            .get_all_conversation_messages(&conversation.id)
            .expect("all messages");
        assert_eq!(all_messages.len(), 3);

        drop(db);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn lists_and_switches_assistant_branch_alternatives() {
        let (db, path) = test_db();
        let conversation = db
            .create_conversation(Some("Alternatives".to_string()))
            .expect("conversation created");

        let user = db
            .append_message(
                &conversation.id,
                None,
                None,
                "user",
                "Explain local AI.",
                "complete",
                Some(DEFAULT_PROVIDER_ID),
                Some("llama3.2:latest"),
            )
            .expect("user message");
        let first_assistant = db
            .append_message(
                &conversation.id,
                Some(&user.id),
                None,
                "assistant",
                "First answer",
                "complete",
                Some(DEFAULT_PROVIDER_ID),
                Some("llama3.2:latest"),
            )
            .expect("first assistant");
        let second_assistant = db
            .append_message(
                &conversation.id,
                Some(&user.id),
                Some(&first_assistant.id),
                "assistant",
                "Second answer",
                "complete",
                Some(DEFAULT_PROVIDER_ID),
                Some("llama3.2:latest"),
            )
            .expect("second assistant");

        db.set_conversation_current_message(
            &conversation.id,
            &second_assistant.id,
            DEFAULT_PROVIDER_ID,
            "llama3.2:latest",
        )
        .expect("current branch updated");

        let alternatives = db
            .get_assistant_alternatives(&conversation.id, &first_assistant.id)
            .expect("assistant alternatives");
        assert_eq!(alternatives.len(), 2);
        assert!(alternatives
            .iter()
            .any(|alternative| alternative.message_id == second_assistant.id && alternative.is_active));

        let active = db
            .switch_active_branch(&conversation.id, &first_assistant.id)
            .expect("branch switched");
        assert_eq!(active.last().map(|message| message.id.as_str()), Some(first_assistant.id.as_str()));

        drop(db);
        let _ = fs::remove_file(path);
    }
}
