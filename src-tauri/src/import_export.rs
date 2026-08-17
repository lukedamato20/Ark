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

    let mut markdown = conversation_to_markdown(
        &conversation,
        &active_messages,
        provider.as_ref(),
        all_messages.len() > active_messages.len(),
    );
    append_attachments_markdown(db, conversation_id, &mut markdown)?;
    Ok(markdown)
}

fn build_attachment_exports(
    db: &Database,
    conversation_id: &str,
) -> Result<Vec<crate::export::AttachmentExport>, AppError> {
    db.list_conversation_attachments(conversation_id)?
        .into_iter()
        .map(|attachment| {
            let content = db.get_attachment_content(&attachment.id)?;
            Ok(crate::export::AttachmentExport {
                schema_version: crate::export::ATTACHMENT_EXPORT_SCHEMA_VERSION,
                attachment,
                content,
            })
        })
        .collect()
}

fn append_attachments_markdown(
    db: &Database,
    conversation_id: &str,
    markdown: &mut String,
) -> Result<(), AppError> {
    let attachments = build_attachment_exports(db, conversation_id)?;
    if attachments.is_empty() {
        return Ok(());
    }
    markdown.push_str("\n\n## Attachments\n");
    for (index, export) in attachments.iter().enumerate() {
        let attachment = &export.attachment;
        let file_name = attachment.file_name.replace('`', "\\`");
        markdown.push_str(&format!(
            "\n### Attachment {}\n\n- File: `{file_name}`\n- Bytes: {}\n- SHA-256: `{}`\n",
            index + 1,
            attachment.byte_size,
            attachment.sha256
        ));
        if let Some(message_id) = attachment.message_id.as_deref() {
            markdown.push_str(&format!("- Source message: `{message_id}`\n"));
        } else {
            markdown.push_str("- Source message: staged / not sent\n");
        }
        markdown.push_str("\nContent:\n\n");
        for line in export.content.lines() {
            markdown.push_str("    ");
            markdown.push_str(line);
            markdown.push('\n');
        }
    }
    Ok(())
}

fn build_conversation_export(
    db: &Database,
    conversation_id: &str,
) -> Result<ConversationExport, AppError> {
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
    let attachments = build_attachment_exports(db, conversation_id)?;
    Ok(ConversationExport {
        schema_version: CONVERSATION_EXPORT_SCHEMA_VERSION,
        exported_at: now(),
        messages: db.get_all_conversation_messages(conversation_id)?,
        conversation,
        provider,
        attachments,
    })
}

pub fn export_conversation_json(db: &Database, conversation_id: &str) -> Result<String, AppError> {
    let export = build_conversation_export(db, conversation_id)?;
    serde_json::to_string_pretty(&export).map_err(|error| {
        AppError::new(
            "export_error",
            format!("Could not serialize export: {error}"),
        )
    })
}

/// FTR-008: `project_id: None` exports every conversation in the workspace; `Some(id)` scopes to
/// that project. Each entry is still a complete, standalone `ConversationExport` — a workspace
/// bundle's entries remain individually re-importable, and the single-conversation import path
/// is reused unchanged for each one (see `import_workspace_json`) rather than duplicated.
pub fn export_workspace_json(db: &Database, project_id: Option<&str>) -> Result<String, AppError> {
    let conversations = db.list_all_conversations(project_id)?;
    let mut entries = Vec::with_capacity(conversations.len());
    let mut conversation_exports = Vec::with_capacity(conversations.len());
    for conversation in &conversations {
        let export = build_conversation_export(db, &conversation.id)?;
        entries.push(crate::export::WorkspaceExportManifestEntry {
            conversation_id: conversation.id.clone(),
            title: conversation.title.clone(),
            message_count: export.messages.len(),
            attachment_count: export.attachments.len(),
            sha256: crate::export::conversation_content_fingerprint(
                &export.messages,
                &export.attachments,
            ),
        });
        conversation_exports.push(export);
    }

    let bundle = crate::export::WorkspaceExport {
        manifest: crate::export::WorkspaceExportManifest {
            schema_version: crate::export::WORKSPACE_EXPORT_SCHEMA_VERSION,
            exported_at: now(),
            scope: project_id.map_or_else(|| "workspace".to_string(), |id| format!("project:{id}")),
            entity_versions: Some(crate::export::WorkspaceEntityVersions::current()),
            entries,
        },
        conversations: conversation_exports,
    };

    serde_json::to_string_pretty(&bundle).map_err(|error| {
        AppError::new(
            "export_error",
            format!("Could not serialize workspace export: {error}"),
        )
    })
}

/// FTR-008: one concatenated, human-readable document — every conversation in scope rendered
/// via the same `conversation_to_markdown` a single-conversation export already uses, separated
/// by a title heading and a horizontal rule so the file reads sensibly without Ark, matching
/// this task's "Markdown remains readable without Ark" acceptance criterion.
pub fn export_workspace_markdown(
    db: &Database,
    project_id: Option<&str>,
) -> Result<String, AppError> {
    let conversations = db.list_all_conversations(project_id)?;
    let scope_label = project_id.map_or_else(
        || "Entire workspace".to_string(),
        |id| format!("Project {id}"),
    );
    let mut document = format!(
        "# Ark export — {scope_label}\n\nExported {}. {} conversation(s).\n\n---\n\n",
        now(),
        conversations.len()
    );
    for conversation in &conversations {
        let active_messages = db.get_active_messages(&conversation.id)?;
        let all_messages = db.get_all_conversation_messages(&conversation.id)?;
        let provider = conversation
            .provider_id
            .as_deref()
            .and_then(|provider_id| db.get_provider(provider_id).ok());
        document.push_str(&conversation_to_markdown(
            conversation,
            &active_messages,
            provider.as_ref(),
            all_messages.len() > active_messages.len(),
        ));
        append_attachments_markdown(db, &conversation.id, &mut document)?;
        document.push_str("\n\n---\n\n");
    }
    Ok(document)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceImportPreviewEntry {
    pub conversation_id: String,
    pub title: String,
    pub message_count: usize,
    pub attachment_count: usize,
    /// FTR-008: set when a local conversation's message content already hashes identically to
    /// this entry — the only "duplicate" signal actually implemented. The frontend uses this to
    /// default the entry's "include" choice to unchecked (skip); nothing here attempts a
    /// semantic merge of two conversations, which the plan's own acceptance criteria only ask
    /// for "where semantic merge is safe" — for two independently-branched message trees, it
    /// isn't.
    pub duplicate_of_local_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceImportPreview {
    pub scope: String,
    pub entries: Vec<WorkspaceImportPreviewEntry>,
    pub provider_mappings: Vec<ImportProviderMapping>,
}

fn parse_workspace_export(json: &str) -> Result<crate::export::WorkspaceExport, AppError> {
    if json.len() > MAX_IMPORT_JSON_BYTES {
        return Err(AppError::invalid_input(format!(
            "Import file is too large ({:.1} MB). The limit is {} MB.",
            json.len() as f64 / (1024.0 * 1024.0),
            MAX_IMPORT_JSON_BYTES / (1024 * 1024)
        )));
    }
    let export: crate::export::WorkspaceExport = serde_json::from_str(json).map_err(|error| {
        AppError::invalid_input(format!("Invalid workspace export JSON: {error}"))
    })?;
    crate::export::validate_workspace_export(&export)?;
    Ok(export)
}

pub fn preview_workspace_import(
    db: &Database,
    json: &str,
) -> Result<WorkspaceImportPreview, AppError> {
    let export = parse_workspace_export(json)?;

    // FTR-008: local conversations are fingerprinted once up front (not once per manifest
    // entry) so this preview stays linear in the number of local conversations rather than
    // quadratic.
    let mut local_fingerprints: HashMap<String, String> = HashMap::new();
    for conversation in db.list_all_conversations(None)? {
        let messages = db.get_all_conversation_messages(&conversation.id)?;
        let fingerprint = if export.manifest.schema_version >= 2 {
            let attachments = build_attachment_exports(db, &conversation.id)?;
            crate::export::conversation_content_fingerprint(&messages, &attachments)
        } else {
            crate::export::conversation_messages_fingerprint(&messages)
        };
        local_fingerprints.insert(fingerprint, conversation.id);
    }

    let mut source_provider_ids = HashSet::new();
    for conversation_export in &export.conversations {
        source_provider_ids.insert(conversation_export.conversation.provider_id.as_deref());
        for message in &conversation_export.messages {
            source_provider_ids.insert(message.provider_id.as_deref());
        }
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

    let entries = export
        .manifest
        .entries
        .iter()
        .map(|entry| WorkspaceImportPreviewEntry {
            conversation_id: entry.conversation_id.clone(),
            title: entry.title.clone(),
            message_count: entry.message_count,
            attachment_count: entry.attachment_count,
            duplicate_of_local_id: local_fingerprints.get(&entry.sha256).cloned(),
        })
        .collect();

    Ok(WorkspaceImportPreview {
        scope: export.manifest.scope,
        entries,
        provider_mappings,
    })
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceImportResult {
    pub imported_count: usize,
    pub skipped_count: usize,
}

/// FTR-008: imports every conversation in the bundle whose ID is in `include_conversation_ids`,
/// skipping the rest — the real, implemented half of "skip/duplicate... where safe" (see
/// `WorkspaceImportPreviewEntry`'s doc comment on why full merge isn't attempted). Each included
/// conversation goes through the exact same `import_conversation_json_with_control` path a
/// single-conversation import uses (re-serialized back to JSON per entry, rather than
/// duplicating that function's logic against an already-parsed value) — so a batch import commits
/// one conversation at a time, not as one all-or-nothing transaction across the whole bundle;
/// a failure partway through leaves everything imported so far intact rather than rolling back
/// conversations that already succeeded.
pub fn import_workspace_json(
    db: &Database,
    json: &str,
    include_conversation_ids: &HashSet<String>,
) -> Result<WorkspaceImportResult, AppError> {
    let export = parse_workspace_export(json)?;
    let mut imported_count = 0usize;
    let mut skipped_count = 0usize;
    for conversation_export in &export.conversations {
        if !include_conversation_ids.contains(&conversation_export.conversation.id) {
            skipped_count += 1;
            continue;
        }
        let single_json = serde_json::to_string(conversation_export).map_err(|error| {
            AppError::new(
                "export_error",
                format!("Could not re-serialize conversation for import: {error}"),
            )
        })?;
        import_conversation_json_with_control(db, &single_json, || false, |_, _| Ok(()))?;
        imported_count += 1;
    }
    Ok(WorkspaceImportResult {
        imported_count,
        skipped_count,
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

        for attachment_export in &export.attachments {
            let imported_attachment = db.create_attachment(
                &imported.id,
                &attachment_export.attachment.file_name,
                &attachment_export.content,
            )?;
            if let Some(source_message_id) = attachment_export.attachment.message_id.as_deref() {
                let imported_message_id = id_map.get(source_message_id).ok_or_else(|| {
                    AppError::invalid_input(
                        "Conversation export attachment references a message that was not imported.",
                    )
                })?;
                db.link_attachments_to_message(
                    &imported.id,
                    imported_message_id,
                    std::slice::from_ref(&imported_attachment.id),
                )?;
            }
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
        db.set_message_branch_name(&assistant.id, Some("Detailed"))
            .expect("branch named");
        let attachment = db
            .create_attachment(&conversation.id, "evidence.txt", "portable attachment body")
            .expect("attachment created");
        db.link_attachments_to_message(
            &conversation.id,
            &user.id,
            std::slice::from_ref(&attachment.id),
        )
        .expect("attachment linked");

        let markdown =
            export_conversation_markdown(&db, &conversation.id).expect("markdown export succeeds");
        assert!(markdown.contains("Hello 世界"));
        assert!(markdown.contains("Hi there! 🌍"));
        assert!(markdown.contains("evidence.txt"));
        assert!(markdown.contains("portable attachment body"));

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
        assert_eq!(
            imported_messages[1].branch_name.as_deref(),
            Some("Detailed")
        );
        assert_eq!(
            db.get_conversation(&imported.conversation.id)
                .expect("imported conversation reloads")
                .current_message_id,
            Some(imported_messages[1].id.clone()),
            "the selected branch must survive export/import with its remapped local ID"
        );
        let imported_attachments = db
            .list_conversation_attachments(&imported.conversation.id)
            .expect("imported attachments");
        assert_eq!(imported_attachments.len(), 1);
        assert_eq!(imported_attachments[0].file_name, "evidence.txt");
        assert_eq!(
            imported_attachments[0].message_id.as_deref(),
            Some(imported_messages[0].id.as_str()),
            "linked attachment must follow the remapped message ID"
        );
        assert_eq!(
            db.get_attachment_content(&imported_attachments[0].id)
                .expect("imported attachment content"),
            "portable attachment body"
        );

        drop(db);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn workspace_v2_round_trip_hashes_attachments_and_tolerates_unknown_fields() {
        let (source_db, source_path) = test_db();
        let conversation = source_db
            .create_conversation(Some("Workspace portable".to_string()))
            .expect("conversation created");
        let message = source_db
            .append_message(
                &conversation.id,
                None,
                None,
                "user",
                "Read the evidence.",
                "complete",
                Some(DEFAULT_PROVIDER_ID),
                Some("model"),
            )
            .expect("message created");
        let attachment = source_db
            .create_attachment(&conversation.id, "evidence.txt", "attachment evidence")
            .expect("attachment created");
        source_db
            .link_attachments_to_message(
                &conversation.id,
                &message.id,
                std::slice::from_ref(&attachment.id),
            )
            .expect("attachment linked");

        let json = export_workspace_json(&source_db, None).expect("workspace export");
        let bundle: crate::export::WorkspaceExport =
            serde_json::from_str(&json).expect("workspace JSON");
        assert_eq!(
            bundle.manifest.schema_version,
            crate::export::WORKSPACE_EXPORT_SCHEMA_VERSION
        );
        assert_eq!(
            bundle.manifest.entity_versions,
            Some(crate::export::WorkspaceEntityVersions::current())
        );
        assert_eq!(bundle.manifest.entries[0].attachment_count, 1);
        assert_eq!(bundle.conversations[0].attachments.len(), 1);

        let preview = preview_workspace_import(&source_db, &json).expect("preview");
        assert_eq!(preview.entries[0].attachment_count, 1);
        assert_eq!(
            preview.entries[0].duplicate_of_local_id.as_deref(),
            Some(conversation.id.as_str())
        );

        let (destination_db, destination_path) = test_db();
        let include = HashSet::from([conversation.id.clone()]);
        let result =
            import_workspace_json(&destination_db, &json, &include).expect("workspace import");
        assert_eq!(result.imported_count, 1);
        let imported = destination_db
            .list_all_conversations(None)
            .expect("destination conversations")
            .into_iter()
            .next()
            .expect("imported conversation");
        let imported_attachments = destination_db
            .list_conversation_attachments(&imported.id)
            .expect("destination attachments");
        assert_eq!(imported_attachments.len(), 1);
        assert_eq!(
            destination_db
                .get_attachment_content(&imported_attachments[0].id)
                .expect("destination content"),
            "attachment evidence"
        );

        let reexport = export_workspace_json(&destination_db, None).expect("destination re-export");
        let reexport: crate::export::WorkspaceExport =
            serde_json::from_str(&reexport).expect("re-export JSON");
        assert_eq!(
            reexport.manifest.entries[0].sha256, bundle.manifest.entries[0].sha256,
            "content hash must survive ID/timestamp remapping"
        );

        let mut additive: serde_json::Value = serde_json::from_str(&json).expect("JSON value");
        additive["futureBundleField"] = serde_json::json!({ "ignored": true });
        additive["manifest"]["futureManifestField"] = serde_json::json!(42);
        additive["manifest"]["entityVersions"]["futureEntity"] = serde_json::json!(1);
        additive["conversations"][0]["futureConversationExportField"] = serde_json::json!(true);
        additive["conversations"][0]["conversation"]["futureConversationField"] =
            serde_json::json!(true);
        additive["conversations"][0]["messages"][0]["futureMessageField"] = serde_json::json!(true);
        additive["conversations"][0]["attachments"][0]["futureAttachmentExportField"] =
            serde_json::json!(true);
        additive["conversations"][0]["attachments"][0]["attachment"]["futureAttachmentField"] =
            serde_json::json!(true);
        preview_workspace_import(
            &destination_db,
            &serde_json::to_string(&additive).expect("additive JSON"),
        )
        .expect("unknown additive fields are tolerated within schema v2");

        let mut tampered: serde_json::Value = serde_json::from_str(&json).expect("JSON value");
        tampered["conversations"][0]["attachments"][0]["content"] =
            serde_json::json!("tampered content");
        let error = preview_workspace_import(
            &destination_db,
            &serde_json::to_string(&tampered).expect("tampered JSON"),
        )
        .expect_err("attachment tampering must fail before import");
        assert_eq!(error.code, "invalid_input");

        drop(source_db);
        drop(destination_db);
        let _ = std::fs::remove_file(source_path);
        let _ = std::fs::remove_file(destination_path);
    }

    #[test]
    fn workspace_v1_without_attachments_or_new_provider_fields_remains_importable() {
        let (source_db, source_path) = test_db();
        let conversation = source_db
            .create_conversation(Some("Legacy workspace".to_string()))
            .expect("conversation created");
        source_db
            .append_message(
                &conversation.id,
                None,
                None,
                "user",
                "Legacy message",
                "complete",
                Some(DEFAULT_PROVIDER_ID),
                Some("model"),
            )
            .expect("message created");
        let current_json = export_workspace_json(&source_db, None).expect("current export");
        let current: crate::export::WorkspaceExport =
            serde_json::from_str(&current_json).expect("current JSON");
        let legacy_hash =
            crate::export::conversation_messages_fingerprint(&current.conversations[0].messages);
        let mut legacy: serde_json::Value =
            serde_json::from_str(&current_json).expect("legacy mutation source");
        legacy["manifest"]["schemaVersion"] = serde_json::json!(1);
        legacy["manifest"]
            .as_object_mut()
            .expect("manifest object")
            .remove("entityVersions");
        legacy["manifest"]["entries"][0]
            .as_object_mut()
            .expect("entry object")
            .remove("attachmentCount");
        legacy["manifest"]["entries"][0]["sha256"] = serde_json::json!(legacy_hash);
        legacy["conversations"][0]["schemaVersion"] = serde_json::json!(1);
        legacy["conversations"][0]
            .as_object_mut()
            .expect("conversation export object")
            .remove("attachments");
        legacy["conversations"][0]["provider"]
            .as_object_mut()
            .expect("provider object")
            .remove("isUserManaged");
        let legacy_json = serde_json::to_string(&legacy).expect("legacy JSON");

        let (destination_db, destination_path) = test_db();
        let preview = preview_workspace_import(&destination_db, &legacy_json)
            .expect("schema-v1 preview remains supported");
        assert_eq!(preview.entries[0].attachment_count, 0);
        let include = HashSet::from([conversation.id]);
        let imported = import_workspace_json(&destination_db, &legacy_json, &include)
            .expect("schema-v1 import remains supported");
        assert_eq!(imported.imported_count, 1);

        drop(source_db);
        drop(destination_db);
        let _ = std::fs::remove_file(source_path);
        let _ = std::fs::remove_file(destination_path);
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
