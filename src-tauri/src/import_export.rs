//! ARC-001: the conversation export/import application workflow, extracted from
//! `commands::mod` as a cohesive service. Every function here takes `&Database` directly
//! rather than a Tauri `State`, so each one can be exercised in a unit test against a real
//! (temp-file or in-memory) SQLite database with zero Tauri runtime involved — the same
//! testability property the `db`/`providers` modules already have. The `#[tauri::command]`
//! wrappers in `commands::mod` are exactly: lock the database, delegate here, return the
//! result — request decoding and response mapping only, no orchestration logic of their own.

use crate::chat::Conversation;
use crate::config::DEFAULT_PROVIDER_ID;
use crate::db::{now, Database};
use crate::errors::AppError;
use crate::export::{
    conversation_to_markdown, is_transient_status, validate_conversation_export,
    ConversationExport, CONVERSATION_EXPORT_SCHEMA_VERSION, MAX_IMPORT_JSON_BYTES,
};
use serde::Serialize;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportConversationResult {
    pub conversation: Conversation,
    /// Count of messages whose status was a transient in-flight state (`pending`/`streaming`)
    /// at export time and were normalized to `interrupted` on import (COR-001/COR-009: an
    /// imported export can never legitimately carry a live generation, since the process that
    /// owned it no longer exists).
    pub normalized_message_count: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportProviderMapping {
    pub source_provider_id: Option<String>,
    pub target_provider_id: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportConversationPreview {
    pub conversation_count: u32,
    pub message_count: usize,
    pub maximum_branch_depth: usize,
    pub normalized_message_count: usize,
    pub conflicts: Vec<String>,
    pub provider_mappings: Vec<ImportProviderMapping>,
    pub estimated_storage_bytes: usize,
}

fn parse_conversation_export(json: &str) -> Result<ConversationExport, AppError> {
    if json.len() > MAX_IMPORT_JSON_BYTES {
        return Err(AppError::invalid_input(format!(
            "Import file is too large ({:.1} MB). The limit is {} MB.",
            json.len() as f64 / (1024.0 * 1024.0),
            MAX_IMPORT_JSON_BYTES / (1024 * 1024)
        )));
    }

    let export: ConversationExport = serde_json::from_str(json)
        .map_err(|error| AppError::invalid_input(format!("Invalid conversation JSON: {error}")))?;
    validate_conversation_export(&export)?;
    Ok(export)
}

fn map_provider(db: &Database, source: Option<&str>) -> Result<ImportProviderMapping, AppError> {
    let Some(source) = source else {
        return Ok(ImportProviderMapping {
            source_provider_id: None,
            target_provider_id: DEFAULT_PROVIDER_ID.to_string(),
            reason: "No source provider was recorded; using Ark's default provider.".to_string(),
        });
    };
    match db.get_provider(source) {
        Ok(_) => Ok(ImportProviderMapping {
            source_provider_id: Some(source.to_string()),
            target_provider_id: source.to_string(),
            reason: "Matched an existing provider by stable ID.".to_string(),
        }),
        Err(error) if error.code == "not_found" => Ok(ImportProviderMapping {
            source_provider_id: Some(source.to_string()),
            target_provider_id: DEFAULT_PROVIDER_ID.to_string(),
            reason: "The source provider is unavailable in this workspace; using Ark's default provider."
                .to_string(),
        }),
        Err(error) => Err(error),
    }
}

fn message_depths(export: &ConversationExport) -> HashMap<&str, usize> {
    let mut depths = HashMap::new();
    for message in &export.messages {
        let depth = message
            .parent_message_id
            .as_deref()
            .and_then(|parent| depths.get(parent).copied())
            .unwrap_or(0usize)
            .saturating_add(1);
        depths.insert(message.id.as_str(), depth);
    }
    depths
}

pub fn preview_conversation_import(
    db: &Database,
    json: &str,
) -> Result<ImportConversationPreview, AppError> {
    let export = parse_conversation_export(json)?;
    let depths = message_depths(&export);
    let mut source_provider_ids = HashSet::new();
    source_provider_ids.insert(export.conversation.provider_id.as_deref());
    for message in &export.messages {
        source_provider_ids.insert(message.provider_id.as_deref());
    }
    let mut provider_mappings = source_provider_ids
        .into_iter()
        .map(|source| map_provider(db, source))
        .collect::<Result<Vec<_>, _>>()?;
    provider_mappings.sort_by(|left, right| {
        left.source_provider_id
            .cmp(&right.source_provider_id)
            .then(left.target_provider_id.cmp(&right.target_provider_id))
    });

    let mut conflicts = Vec::new();
    match db.get_conversation(&export.conversation.id) {
        Ok(_) => conflicts.push(
            "The source conversation ID already exists; Ark will assign a new local ID."
                .to_string(),
        ),
        Err(error) if error.code == "not_found" => {}
        Err(error) => return Err(error),
    }

    Ok(ImportConversationPreview {
        conversation_count: 1,
        message_count: export.messages.len(),
        maximum_branch_depth: depths.values().copied().max().unwrap_or(0),
        normalized_message_count: export
            .messages
            .iter()
            .filter(|message| is_transient_status(&message.status))
            .count(),
        conflicts,
        provider_mappings,
        estimated_storage_bytes: json
            .len()
            .saturating_add(export.messages.len().saturating_mul(512)),
    })
}

pub fn export_conversation_markdown(
    db: &Database,
    conversation_id: &str,
) -> Result<String, AppError> {
    let conversation = db.get_conversation(conversation_id)?;
    let active_messages = db.get_active_messages(conversation_id)?;
    let all_messages = db.get_all_conversation_messages(conversation_id)?;
    let provider = conversation
        .provider_id
        .as_deref()
        .and_then(|provider_id| db.get_provider(provider_id).ok());

    Ok(conversation_to_markdown(
        &conversation,
        &active_messages,
        provider.as_ref(),
        all_messages.len() > active_messages.len(),
    ))
}

pub fn export_conversation_json(db: &Database, conversation_id: &str) -> Result<String, AppError> {
    let conversation = db.get_conversation(conversation_id)?;
    let provider = conversation
        .provider_id
        .as_deref()
        .and_then(|provider_id| db.get_provider(provider_id).ok())
        .map(|mut provider| {
            // SEC-005: even the opaque OS-keychain lookup identifier is device-local and has
            // no useful meaning in a portable conversation export. Dropping it makes the
            // reconnect-on-restore contract explicit and prevents future import code from
            // accidentally treating a foreign device's credential reference as usable.
            provider.api_key_ref = None;
            provider
        });
    let export = ConversationExport {
        schema_version: CONVERSATION_EXPORT_SCHEMA_VERSION,
        exported_at: now(),
        messages: db.get_all_conversation_messages(conversation_id)?,
        conversation,
        provider,
    };

    serde_json::to_string_pretty(&export).map_err(|error| {
        AppError::new(
            "export_error",
            format!("Could not serialize export: {error}"),
        )
    })
}

#[cfg(test)]
pub fn import_conversation_json(
    db: &Database,
    json: &str,
) -> Result<ImportConversationResult, AppError> {
    import_conversation_json_with_control(db, json, || false, |_, _| Ok(()))
}

pub fn import_conversation_json_with_control<C, P>(
    db: &Database,
    json: &str,
    is_cancelled: C,
    mut on_progress: P,
) -> Result<ImportConversationResult, AppError>
where
    C: Fn() -> bool,
    P: FnMut(usize, usize) -> Result<(), AppError>,
{
    let export = parse_conversation_export(json)?;
    if is_cancelled() {
        return Err(AppError::new("import_cancelled", "Import was cancelled."));
    }
    let conversation_mapping = map_provider(db, export.conversation.provider_id.as_deref())?;
    let mut provider_mappings = HashMap::new();
    for message in &export.messages {
        if let Some(source) = message.provider_id.as_deref() {
            if !provider_mappings.contains_key(source) {
                provider_mappings.insert(
                    source.to_string(),
                    map_provider(db, Some(source))?.target_provider_id,
                );
            }
        }
    }

    // COR-004/COR-009: an import can insert an unbounded number of messages; without a
    // transaction, a failure partway through (or the process crashing) would leave a
    // half-imported conversation with no way to tell which messages actually belong to the
    // original export. The whole import commits atomically or not at all.
    let (imported_id, normalized_message_count) = db.transaction(|| {
        let imported =
            db.create_conversation(Some(format!("{} (imported)", export.conversation.title)))?;
        let mut id_map: HashMap<String, String> = HashMap::new();
        let mut imported_current_message_id: Option<String> = None;
        let mut normalized_message_count: i64 = 0;

        for (index, message) in export.messages.iter().enumerate() {
            if is_cancelled() {
                return Err(AppError::new("import_cancelled", "Import was cancelled."));
            }
            let parent = message
                .parent_message_id
                .as_ref()
                .and_then(|id| id_map.get(id).map(String::as_str));
            let revision = message
                .revision_of_message_id
                .as_ref()
                .and_then(|id| id_map.get(id).map(String::as_str));

            let normalized = is_transient_status(&message.status);
            let import_status = if normalized {
                "interrupted"
            } else {
                message.status.as_str()
            };
            if normalized {
                normalized_message_count += 1;
            }

            let new_message = db.append_message(
                &imported.id,
                parent,
                revision,
                &message.role,
                &message.content,
                import_status,
                message
                    .provider_id
                    .as_deref()
                    .and_then(|source| provider_mappings.get(source).map(String::as_str)),
                message.model_id.as_deref(),
            )?;

            if normalized {
                db.finish_message(
                    &new_message.id,
                    "interrupted",
                    Some("Generation was still in progress when this conversation was exported."),
                    None,
                    None,
                )?;
            }

            let mut metadata = match message.metadata_json.as_deref() {
                Some(raw) => match serde_json::from_str::<serde_json::Value>(raw) {
                    Ok(serde_json::Value::Object(object)) => object,
                    Ok(value) => {
                        let mut object = serde_json::Map::new();
                        object.insert("importedSourceMetadata".to_string(), value);
                        object
                    }
                    Err(_) => {
                        let mut object = serde_json::Map::new();
                        object.insert(
                            "importedSourceMetadataRaw".to_string(),
                            serde_json::Value::String(raw.to_string()),
                        );
                        object
                    }
                },
                None => serde_json::Map::new(),
            };
            metadata.insert(
                "importedOriginalMessageId".to_string(),
                serde_json::Value::String(message.id.clone()),
            );
            metadata.insert(
                "importedOriginalConversationId".to_string(),
                serde_json::Value::String(export.conversation.id.clone()),
            );
            let metadata_json = serde_json::Value::Object(metadata).to_string();
            db.apply_imported_message_fields(
                &new_message.id,
                message,
                message
                    .provider_id
                    .as_deref()
                    .and_then(|source| provider_mappings.get(source).map(String::as_str)),
                &metadata_json,
            )?;

            if export.conversation.current_message_id.as_deref() == Some(message.id.as_str()) {
                imported_current_message_id = Some(new_message.id.clone());
            }

            id_map.insert(message.id.clone(), new_message.id);
            on_progress(index + 1, export.messages.len())?;
        }

        if is_cancelled() {
            return Err(AppError::new("import_cancelled", "Import was cancelled."));
        }
        if let Some(current_message_id) = imported_current_message_id {
            db.set_conversation_current_message(
                &imported.id,
                &current_message_id,
                &conversation_mapping.target_provider_id,
                export.conversation.model_id.as_deref().unwrap_or(""),
            )?;
        }
        db.apply_imported_conversation_fields(
            &imported.id,
            &export.conversation,
            &conversation_mapping.target_provider_id,
        )?;

        Ok((imported.id, normalized_message_count))
    })?;

    Ok(ImportConversationResult {
        conversation: db.get_conversation(&imported_id)?,
        normalized_message_count,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::DEFAULT_PROVIDER_ID;
    use std::cell::Cell;
    use std::env;
    use uuid::Uuid;

    fn test_db() -> (Database, std::path::PathBuf) {
        let path =
            env::temp_dir().join(format!("ark-import-export-test-{}.sqlite3", Uuid::new_v4()));
        let db = Database::open(&path).expect("database opens");
        (db, path)
    }

    /// ARC-001 acceptance evidence: this workflow function is called directly with a real
    /// `Database` — no `State<AppState>`, no Tauri app handle, no mock — proving the
    /// "testable without a Tauri window" property the task requires.
    #[test]
    fn export_and_import_round_trip_through_the_extracted_service_functions() {
        let (db, path) = test_db();
        let conversation = db
            .create_conversation(Some("Round trip via service".to_string()))
            .expect("conversation created");
        let user = db
            .append_message(
                &conversation.id,
                None,
                None,
                "user",
                "Hello 世界",
                "complete",
                Some(DEFAULT_PROVIDER_ID),
                Some("llama3.2:latest"),
            )
            .expect("user message");
        let assistant = db
            .append_message(
                &conversation.id,
                Some(&user.id),
                None,
                "assistant",
                "Hi there! 🌍",
                "complete",
                Some(DEFAULT_PROVIDER_ID),
                Some("llama3.2:latest"),
            )
            .expect("assistant message");
        db.set_conversation_current_message(
            &conversation.id,
            &assistant.id,
            DEFAULT_PROVIDER_ID,
            "llama3.2:latest",
        )
        .expect("set current message");

        let markdown =
            export_conversation_markdown(&db, &conversation.id).expect("markdown export succeeds");
        assert!(markdown.contains("Hello 世界"));
        assert!(markdown.contains("Hi there! 🌍"));

        let json = export_conversation_json(&db, &conversation.id).expect("json export succeeds");
        let imported = import_conversation_json(&db, &json).expect("import succeeds");
        assert_eq!(imported.normalized_message_count, 0);
        assert!(imported
            .conversation
            .title
            .contains("Round trip via service"));

        let imported_messages = db
            .get_active_messages(&imported.conversation.id)
            .expect("imported messages");
        assert_eq!(imported_messages.len(), 2);
        assert_eq!(imported_messages[0].content, "Hello 世界");
        assert_eq!(imported_messages[1].content, "Hi there! 🌍");

        drop(db);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn conversation_export_excludes_provider_secret_references_and_values() {
        let (db, path) = test_db();
        let conversation = db
            .create_conversation(Some("Secret-safe export".to_string()))
            .expect("conversation created");
        db.set_provider_api_key_ref(
            DEFAULT_PROVIDER_ID,
            Some("secret:v1:00000000-0000-4000-8000-000000000000"),
        )
        .expect("opaque reference linked");
        let raw_secret = "must-never-enter-an-export";

        let json = export_conversation_json(&db, &conversation.id).expect("export succeeds");
        let parsed: ConversationExport = serde_json::from_str(&json).expect("valid export JSON");

        assert_eq!(
            parsed.provider.and_then(|provider| provider.api_key_ref),
            None
        );
        assert!(!json.contains("secret:v1:"));
        assert!(!json.contains(raw_secret));

        drop(db);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn import_normalizes_transient_statuses_and_reports_the_count() {
        let (db, path) = test_db();
        let conversation = db
            .create_conversation(Some("Source".to_string()))
            .expect("conversation created");
        let user = db
            .append_message(
                &conversation.id,
                None,
                None,
                "user",
                "Question",
                "complete",
                Some(DEFAULT_PROVIDER_ID),
                Some("llama3.2:latest"),
            )
            .expect("user message");
        db.set_conversation_current_message(
            &conversation.id,
            &user.id,
            DEFAULT_PROVIDER_ID,
            "llama3.2:latest",
        )
        .expect("set current message");
        let json = export_conversation_json(&db, &conversation.id).expect("export succeeds");

        // Simulate an export captured mid-generation: hand-edit the JSON's status to a
        // transient one, matching what a real crash-mid-export snapshot would contain.
        let tampered = json.replace("\"status\": \"complete\"", "\"status\": \"streaming\"");
        let result = import_conversation_json(&db, &tampered).expect("import succeeds");
        assert_eq!(result.normalized_message_count, 1);

        let imported_messages = db
            .get_active_messages(&result.conversation.id)
            .expect("imported messages");
        assert_eq!(imported_messages[0].status, "interrupted");

        drop(db);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn import_rejects_an_oversized_payload_before_deserializing() {
        let (db, path) = test_db();
        let oversized = "x".repeat(MAX_IMPORT_JSON_BYTES + 1);
        let error = import_conversation_json(&db, &oversized)
            .expect_err("must reject an oversized payload");
        assert_eq!(error.code, "invalid_input");

        drop(db);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn import_rolls_back_the_whole_conversation_on_a_validation_failure() {
        let (db, path) = test_db();
        // A schema-version mismatch fails validate_conversation_export before the transaction
        // even opens — no conversation should be created at all.
        let history_request = crate::chat::ConversationListRequest {
            limit: Some(100),
            cursor: None,
            query: None,
            archived: Some(false),
            project_id: None,
        };
        let before = db
            .list_conversations_page(&history_request)
            .expect("list before")
            .items
            .len();
        let bogus = "{\"schemaVersion\": 999, \"exportedAt\": \"x\", \"conversation\": {}, \"messages\": []}";
        let error =
            import_conversation_json(&db, bogus).expect_err("must reject unknown schema version");
        assert_eq!(error.code, "invalid_input");
        let after = db
            .list_conversations_page(&history_request)
            .expect("list after")
            .items
            .len();
        assert_eq!(
            before, after,
            "a rejected import must not create a partial conversation"
        );

        drop(db);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn dry_run_reports_counts_depth_conflicts_mapping_normalization_and_storage() {
        let (db, path) = test_db();
        let conversation = db
            .create_conversation(Some("Preview source".to_string()))
            .expect("conversation created");
        let user = db
            .append_message(
                &conversation.id,
                None,
                None,
                "user",
                "Question",
                "complete",
                Some(DEFAULT_PROVIDER_ID),
                Some("model"),
            )
            .expect("user message");
        db.append_message(
            &conversation.id,
            Some(&user.id),
            None,
            "assistant",
            "Partial",
            "streaming",
            Some(DEFAULT_PROVIDER_ID),
            Some("model"),
        )
        .expect("assistant message");
        let json = export_conversation_json(&db, &conversation.id).expect("export");

        let preview = preview_conversation_import(&db, &json).expect("preview");
        assert_eq!(preview.conversation_count, 1);
        assert_eq!(preview.message_count, 2);
        assert_eq!(preview.maximum_branch_depth, 2);
        assert_eq!(preview.normalized_message_count, 1);
        assert_eq!(preview.conflicts.len(), 1);
        assert!(preview.estimated_storage_bytes > json.len());
        assert!(preview
            .provider_mappings
            .iter()
            .any(|mapping| mapping.target_provider_id == DEFAULT_PROVIDER_ID));

        drop(db);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn cancellation_after_progress_rolls_back_every_imported_row() {
        let (db, path) = test_db();
        let source = db
            .create_conversation(Some("Cancelled source".to_string()))
            .expect("conversation created");
        let mut parent = None;
        for index in 0..3 {
            let created = db
                .append_message(
                    &source.id,
                    parent.as_deref(),
                    None,
                    if index % 2 == 0 { "user" } else { "assistant" },
                    "content",
                    "complete",
                    Some(DEFAULT_PROVIDER_ID),
                    Some("model"),
                )
                .expect("source message");
            parent = Some(created.id);
        }
        let json = export_conversation_json(&db, &source.id).expect("export");
        let before = db
            .list_conversations_page(&crate::chat::ConversationListRequest {
                limit: Some(100),
                cursor: None,
                query: None,
                archived: None,
                project_id: None,
            })
            .expect("list before")
            .items
            .len();
        let cancelled = Cell::new(false);
        let error = import_conversation_json_with_control(
            &db,
            &json,
            || cancelled.get(),
            |completed, _| {
                if completed == 1 {
                    cancelled.set(true);
                }
                Ok(())
            },
        )
        .expect_err("cancelled import must fail");
        assert_eq!(error.code, "import_cancelled");
        let after = db
            .list_conversations_page(&crate::chat::ConversationListRequest {
                limit: Some(100),
                cursor: None,
                query: None,
                archived: None,
                project_id: None,
            })
            .expect("list after")
            .items
            .len();
        assert_eq!(before, after, "the transaction must roll back completely");

        drop(db);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn import_preserves_settings_provenance_and_maps_an_unavailable_provider() {
        let (db, path) = test_db();
        let source = db
            .create_conversation(Some("Portable".to_string()))
            .expect("conversation created");
        let message = db
            .append_message(
                &source.id,
                None,
                None,
                "assistant",
                "answer",
                "failed",
                Some(DEFAULT_PROVIDER_ID),
                Some("portable-model"),
            )
            .expect("message");
        db.set_conversation_current_message(
            &source.id,
            &message.id,
            DEFAULT_PROVIDER_ID,
            "portable-model",
        )
        .expect("set current");
        let json = export_conversation_json(&db, &source.id).expect("export");
        let mut value: serde_json::Value = serde_json::from_str(&json).expect("JSON");
        value["conversation"]["providerId"] = serde_json::json!("missing-provider");
        value["conversation"]["systemPrompt"] = serde_json::json!("Portable system prompt");
        value["conversation"]["temperature"] = serde_json::json!(0.4);
        value["conversation"]["maxTokens"] = serde_json::json!(321);
        value["conversation"]["archived"] = serde_json::json!(true);
        value["conversation"]["projectId"] = serde_json::json!("project-from-export");
        value["messages"][0]["providerId"] = serde_json::json!("missing-provider");
        value["messages"][0]["tokenCount"] = serde_json::json!(17);
        value["messages"][0]["errorMessage"] = serde_json::json!("portable failure");
        value["messages"][0]["metadataJson"] = serde_json::json!("{\"custom\":true}");
        value["futureRootField"] = serde_json::json!({ "ignored": true });
        value["conversation"]["futureConversationField"] = serde_json::json!(42);
        value["messages"][0]["futureMessageField"] = serde_json::json!("ignored");
        let portable_json = serde_json::to_string(&value).expect("serialize");

        let imported = import_conversation_json(&db, &portable_json).expect("import");
        assert_eq!(
            imported.conversation.provider_id.as_deref(),
            Some(DEFAULT_PROVIDER_ID)
        );
        assert_eq!(
            imported.conversation.system_prompt.as_deref(),
            Some("Portable system prompt")
        );
        assert_eq!(imported.conversation.temperature, Some(0.4));
        assert_eq!(imported.conversation.max_tokens, Some(321));
        assert!(imported.conversation.archived);
        assert_eq!(
            imported.conversation.project_id.as_deref(),
            Some("project-from-export")
        );
        let imported_message = db
            .get_all_conversation_messages(&imported.conversation.id)
            .expect("messages")
            .into_iter()
            .next()
            .expect("message");
        assert_eq!(
            imported_message.provider_id.as_deref(),
            Some(DEFAULT_PROVIDER_ID)
        );
        assert_eq!(imported_message.token_count, Some(17));
        assert_eq!(
            imported_message.error_message.as_deref(),
            Some("portable failure")
        );
        let metadata: serde_json::Value =
            serde_json::from_str(imported_message.metadata_json.as_deref().expect("metadata"))
                .expect("metadata JSON");
        assert_eq!(metadata["custom"], true);
        assert_eq!(metadata["importedOriginalMessageId"], message.id);
        assert_eq!(metadata["importedOriginalConversationId"], source.id);

        drop(db);
        let _ = std::fs::remove_file(path);
    }
}
