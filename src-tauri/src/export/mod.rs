use crate::attachments::Attachment;
use crate::chat::{Conversation, Message};
use crate::errors::AppError;
use crate::providers::ProviderConfig;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};

/// FTR-008: a deterministic content fingerprint for a conversation's messages — role, content,
/// and path position only, deliberately excluding IDs/timestamps/provider so the same
/// conversation exported twice (or exported, imported into a fresh workspace with new local
/// IDs, then exported again) still hashes identically. Used both to populate
/// `WorkspaceExportManifestEntry.sha256` and, on import, to detect a conversation already
/// present locally.
pub fn conversation_messages_fingerprint(messages: &[Message]) -> String {
    let mut hasher = Sha256::new();
    for message in messages {
        hasher.update(message.role.as_bytes());
        hasher.update([0u8]);
        hasher.update(message.content.as_bytes());
        hasher.update([0u8]);
        hasher.update(message.path_index.to_le_bytes());
        hasher.update([0u8]);
    }
    format!("{:x}", hasher.finalize())
}

/// FTR-008 schema-v2 fingerprint: message content plus stable attachment content identity.
/// Attachment IDs, timestamps, and linked message IDs are deliberately excluded because import
/// remaps them. Ordering is the database's stable attachment creation order.
pub fn conversation_content_fingerprint(
    messages: &[Message],
    attachments: &[AttachmentExport],
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(conversation_messages_fingerprint(messages).as_bytes());
    for export in attachments {
        hasher.update([0u8]);
        hasher.update(export.attachment.file_name.as_bytes());
        hasher.update([0u8]);
        hasher.update(export.attachment.sha256.as_bytes());
        hasher.update([0u8]);
        if let Some(message_id) = export.attachment.message_id.as_deref() {
            if let Some(message) = messages.iter().find(|message| message.id == message_id) {
                hasher.update(b"linked");
                hasher.update(message.path_index.to_le_bytes());
                hasher.update(message.role.as_bytes());
                hasher.update([0u8]);
                hasher.update(message.content.as_bytes());
            } else {
                // Validation rejects this case; keep the hash deterministic before validation.
                hasher.update(b"invalid-link");
            }
        } else {
            hasher.update(b"staged");
        }
    }
    format!("{:x}", hasher.finalize())
}

pub const CONVERSATION_EXPORT_SCHEMA_VERSION: u32 = 2;
pub const MIN_CONVERSATION_EXPORT_SCHEMA_VERSION: u32 = 1;
pub const ATTACHMENT_EXPORT_SCHEMA_VERSION: u32 = 1;

/// COR-009 bounded-import ceilings. Conservative and visible on purpose — raising them is a
/// deliberate decision, not a side effect of someone hitting the limit.
pub const MAX_IMPORT_JSON_BYTES: usize = 50 * 1024 * 1024;
pub const MAX_IMPORT_MESSAGES: usize = 20_000;
pub const MAX_MESSAGE_CONTENT_CHARS: usize = 2_000_000;
pub const MAX_IMPORT_BRANCH_DEPTH: usize = 2_048;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationExport {
    pub schema_version: u32,
    pub exported_at: String,
    pub conversation: Conversation,
    pub messages: Vec<Message>,
    pub provider: Option<ProviderConfig>,
    /// Added in schema v2. `default` preserves import compatibility with v1 bundles.
    #[serde(default)]
    pub attachments: Vec<AttachmentExport>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttachmentExport {
    pub schema_version: u32,
    pub attachment: Attachment,
    pub content: String,
}

pub fn validate_conversation_export(export: &ConversationExport) -> Result<(), AppError> {
    if !(MIN_CONVERSATION_EXPORT_SCHEMA_VERSION..=CONVERSATION_EXPORT_SCHEMA_VERSION)
        .contains(&export.schema_version)
    {
        return Err(AppError::invalid_input(
            "Unsupported conversation export schema version.",
        ));
    }

    if export.conversation.id.trim().is_empty() {
        return Err(AppError::invalid_input(
            "Conversation export is missing a conversation ID.",
        ));
    }

    if export.conversation.title.trim().is_empty() {
        return Err(AppError::invalid_input(
            "Conversation export is missing a conversation title.",
        ));
    }

    if export.messages.len() > MAX_IMPORT_MESSAGES {
        return Err(AppError::invalid_input(format!(
            "Conversation export contains {} messages, which exceeds the {} message import limit.",
            export.messages.len(),
            MAX_IMPORT_MESSAGES
        )));
    }

    let mut seen_ids = HashSet::new();
    for message in &export.messages {
        if message.id.trim().is_empty() {
            return Err(AppError::invalid_input(
                "Conversation export contains a message without an ID.",
            ));
        }
        if !seen_ids.insert(message.id.as_str()) {
            return Err(AppError::invalid_input(
                "Conversation export contains duplicate message IDs.",
            ));
        }
        if message.conversation_id != export.conversation.id {
            return Err(AppError::invalid_input(
                "Conversation export contains a message from a different conversation.",
            ));
        }
        if !is_valid_role(&message.role) {
            return Err(AppError::invalid_input(
                "Conversation export contains an invalid message role.",
            ));
        }
        if !is_valid_status(&message.status) {
            return Err(AppError::invalid_input(
                "Conversation export contains an invalid message status.",
            ));
        }
        if message.content.chars().count() > MAX_MESSAGE_CONTENT_CHARS {
            return Err(AppError::invalid_input(format!(
                "A message in this conversation export exceeds the {MAX_MESSAGE_CONTENT_CHARS}-character import limit."
            )));
        }
    }

    let all_ids = export
        .messages
        .iter()
        .map(|message| message.id.as_str())
        .collect::<HashSet<_>>();
    let mut importable_ids = HashSet::new();
    let mut depths = HashMap::new();
    for message in &export.messages {
        validate_message_reference(
            message.parent_message_id.as_deref(),
            &all_ids,
            &importable_ids,
            "parent",
        )?;
        validate_message_reference(
            message.revision_of_message_id.as_deref(),
            &all_ids,
            &importable_ids,
            "revision",
        )?;
        let depth = message
            .parent_message_id
            .as_deref()
            .and_then(|parent| depths.get(parent).copied())
            .unwrap_or(0usize)
            .saturating_add(1);
        if depth > MAX_IMPORT_BRANCH_DEPTH {
            return Err(AppError::invalid_input(format!(
                "Conversation export exceeds the maximum branch depth of {MAX_IMPORT_BRANCH_DEPTH}."
            )));
        }
        depths.insert(message.id.as_str(), depth);
        importable_ids.insert(message.id.as_str());
    }

    if let Some(current_message_id) = export.conversation.current_message_id.as_deref() {
        if !all_ids.contains(current_message_id) {
            return Err(AppError::invalid_input(
                "Conversation export current message does not exist in the message list.",
            ));
        }
    }

    const MAX_IMPORT_ATTACHMENTS: usize = 10_000;
    if export.attachments.len() > MAX_IMPORT_ATTACHMENTS {
        return Err(AppError::invalid_input(format!(
            "Conversation export contains {} attachments, which exceeds the {MAX_IMPORT_ATTACHMENTS} attachment import limit.",
            export.attachments.len()
        )));
    }
    let mut attachment_ids = HashSet::new();
    for attachment_export in &export.attachments {
        if attachment_export.schema_version != ATTACHMENT_EXPORT_SCHEMA_VERSION {
            return Err(AppError::invalid_input(
                "Unsupported attachment export schema version.",
            ));
        }
        let attachment = &attachment_export.attachment;
        if attachment.id.trim().is_empty() || !attachment_ids.insert(attachment.id.as_str()) {
            return Err(AppError::invalid_input(
                "Conversation export contains a missing or duplicate attachment ID.",
            ));
        }
        if attachment.conversation_id != export.conversation.id {
            return Err(AppError::invalid_input(
                "Conversation export contains an attachment from a different conversation.",
            ));
        }
        if attachment
            .message_id
            .as_deref()
            .is_some_and(|message_id| !all_ids.contains(message_id))
        {
            return Err(AppError::invalid_input(
                "Conversation export attachment references a message outside the conversation.",
            ));
        }
        let (validated_name, validated_content) = crate::validation::validate_attachment(
            &attachment.file_name,
            &attachment_export.content,
        )?;
        let digest = format!("{:x}", Sha256::digest(validated_content.as_bytes()));
        if validated_name != attachment.file_name
            || attachment.byte_size != validated_content.len() as i64
            || attachment.sha256 != digest
        {
            return Err(AppError::invalid_input(
                "Conversation export attachment metadata does not match its content.",
            ));
        }
    }

    Ok(())
}

pub const WORKSPACE_EXPORT_SCHEMA_VERSION: u32 = 2;
pub const MIN_WORKSPACE_EXPORT_SCHEMA_VERSION: u32 = 1;

/// FTR-008: one entry per included conversation. `sha256` is computed over the conversation's
/// serialized `messages` only (not the wrapping `ConversationExport`, whose `conversation`/
/// `provider` fields the destination workspace may legitimately rewrite on import — a new local
/// ID, a remapped provider) — so the hash stays meaningful for duplicate detection across two
/// workspaces that both imported the same original export, not just byte-identical files.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceExportManifestEntry {
    pub conversation_id: String,
    pub title: String,
    pub message_count: usize,
    #[serde(default)]
    pub attachment_count: usize,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceEntityVersions {
    pub conversation: u32,
    pub message: u32,
    pub provider: u32,
    pub attachment: u32,
}

impl WorkspaceEntityVersions {
    pub const fn current() -> Self {
        Self {
            conversation: CONVERSATION_EXPORT_SCHEMA_VERSION,
            message: 1,
            provider: 1,
            attachment: ATTACHMENT_EXPORT_SCHEMA_VERSION,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceExportManifest {
    pub schema_version: u32,
    pub exported_at: String,
    /// `"workspace"` or `"project:<project id>"` — recorded so a reader (human or Ark itself on
    /// re-import) can tell what this bundle was scoped to without re-deriving it from the entry
    /// list.
    pub scope: String,
    /// Added in workspace schema v2. The manifest records the version of every included entity
    /// family instead of leaving those versions implicit in Rust types.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entity_versions: Option<WorkspaceEntityVersions>,
    pub entries: Vec<WorkspaceExportManifestEntry>,
}

/// FTR-008: a batch export — every conversation in scope, each still shaped as the existing
/// single-conversation `ConversationExport` (so a workspace bundle's entries remain valid
/// standalone conversation exports too), plus a manifest recording what's included and a
/// content hash per conversation.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceExport {
    pub manifest: WorkspaceExportManifest,
    pub conversations: Vec<ConversationExport>,
}

/// FTR-008 bounded-import ceiling for a batch — deliberately separate from
/// `MAX_IMPORT_MESSAGES` (which bounds one conversation), conservative and visible on purpose.
pub const MAX_IMPORT_CONVERSATIONS: usize = 5_000;

pub fn validate_workspace_export(export: &WorkspaceExport) -> Result<(), AppError> {
    if !(MIN_WORKSPACE_EXPORT_SCHEMA_VERSION..=WORKSPACE_EXPORT_SCHEMA_VERSION)
        .contains(&export.manifest.schema_version)
    {
        return Err(AppError::invalid_input(
            "Unsupported workspace export schema version.",
        ));
    }
    if export.conversations.len() > MAX_IMPORT_CONVERSATIONS {
        return Err(AppError::invalid_input(format!(
            "Workspace export contains {} conversations, which exceeds the {} conversation import limit.",
            export.conversations.len(),
            MAX_IMPORT_CONVERSATIONS
        )));
    }
    if export.manifest.entries.len() != export.conversations.len() {
        return Err(AppError::invalid_input(
            "Workspace export manifest does not match its conversation entries.",
        ));
    }
    if export.manifest.schema_version >= 2
        && export.manifest.entity_versions.as_ref() != Some(&WorkspaceEntityVersions::current())
    {
        return Err(AppError::invalid_input(
            "Workspace export entity versions are missing or unsupported.",
        ));
    }
    let manifest_ids: HashSet<&str> = export
        .manifest
        .entries
        .iter()
        .map(|entry| entry.conversation_id.as_str())
        .collect();
    if manifest_ids.len() != export.manifest.entries.len() {
        return Err(AppError::invalid_input(
            "Workspace export manifest contains duplicate conversation IDs.",
        ));
    }
    for conversation_export in &export.conversations {
        let Some(entry) = export
            .manifest
            .entries
            .iter()
            .find(|entry| entry.conversation_id == conversation_export.conversation.id)
        else {
            return Err(AppError::invalid_input(
                "Workspace export contains a conversation not listed in its manifest.",
            ));
        };
        if export.manifest.schema_version >= 2
            && conversation_export.schema_version != CONVERSATION_EXPORT_SCHEMA_VERSION
        {
            return Err(AppError::invalid_input(
                "Workspace export conversation entity version does not match its manifest.",
            ));
        }
        validate_conversation_export(conversation_export)?;
        let expected_hash = if export.manifest.schema_version >= 2 {
            conversation_content_fingerprint(
                &conversation_export.messages,
                &conversation_export.attachments,
            )
        } else {
            conversation_messages_fingerprint(&conversation_export.messages)
        };
        if entry.title != conversation_export.conversation.title
            || entry.message_count != conversation_export.messages.len()
            || (export.manifest.schema_version >= 2
                && entry.attachment_count != conversation_export.attachments.len())
            || entry.sha256 != expected_hash
        {
            return Err(AppError::invalid_input(
                "Workspace export manifest counts or hashes do not match its content.",
            ));
        }
    }
    Ok(())
}

pub fn conversation_to_markdown(
    conversation: &Conversation,
    messages: &[Message],
    provider: Option<&ProviderConfig>,
    has_branches: bool,
) -> String {
    let provider_name = provider
        .map(|value| value.name.as_str())
        .unwrap_or("Unknown provider");
    let model = conversation
        .model_id
        .as_deref()
        .unwrap_or("No model selected");

    let mut markdown = String::new();
    markdown.push_str(&format!("# {}\n\n", conversation.title));
    markdown.push_str(&format!("- Created: {}\n", conversation.created_at));
    markdown.push_str(&format!("- Provider: {provider_name}\n"));
    markdown.push_str(&format!("- Model: {model}\n"));
    if has_branches {
        markdown.push_str("- Note: Alternate branches exist. This Markdown export contains the active branch only.\n");
    }
    markdown.push('\n');

    for message in messages {
        markdown.push_str(&format!("## {}\n\n", role_label(&message.role)));
        if message.status != "complete" {
            markdown.push_str(&format!("_Status: {}_\n\n", message.status));
        }
        markdown.push_str(message.content.trim());
        markdown.push_str("\n\n");
    }

    markdown
}

fn validate_message_reference(
    reference: Option<&str>,
    all_ids: &HashSet<&str>,
    importable_ids: &HashSet<&str>,
    reference_name: &str,
) -> Result<(), AppError> {
    let Some(reference) = reference else {
        return Ok(());
    };

    if !all_ids.contains(reference) {
        return Err(AppError::invalid_input(format!(
            "Conversation export contains a missing {reference_name} message reference.",
        )));
    }

    if !importable_ids.contains(reference) {
        return Err(AppError::invalid_input(format!(
            "Conversation export contains a {reference_name} reference before the referenced message is importable.",
        )));
    }

    Ok(())
}

fn is_valid_role(role: &str) -> bool {
    matches!(role, "system" | "user" | "assistant" | "tool")
}

fn is_valid_status(status: &str) -> bool {
    matches!(
        status,
        "pending" | "streaming" | "complete" | "failed" | "cancelled" | "interrupted"
    )
}

/// True for durable statuses that only make sense while a generation is actively running.
/// An imported export can never legitimately contain these — the process that owned the
/// generation is gone — so COR-001 requires normalizing them to `interrupted` on import.
pub fn is_transient_status(status: &str) -> bool {
    matches!(status, "pending" | "streaming")
}

fn role_label(role: &str) -> &str {
    match role {
        "user" => "User",
        "assistant" => "Assistant",
        "system" => "System",
        "tool" => "Tool",
        _ => "Message",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn export_with_messages(messages: Vec<Message>) -> ConversationExport {
        ConversationExport {
            schema_version: CONVERSATION_EXPORT_SCHEMA_VERSION,
            exported_at: "2026-07-02T00:00:00Z".to_string(),
            conversation: Conversation {
                id: "conversation-1".to_string(),
                title: "Imported".to_string(),
                created_at: "2026-07-02T00:00:00Z".to_string(),
                updated_at: "2026-07-02T00:00:00Z".to_string(),
                provider_id: None,
                model_id: None,
                current_message_id: messages.last().map(|message| message.id.clone()),
                system_prompt: None,
                temperature: None,
                max_tokens: None,
                archived: false,
                project_id: None,
                pinned_at: None,
                persona_id: None,
                response_style: None,
                tone: None,
            },
            messages,
            provider: None,
            attachments: Vec::new(),
        }
    }

    fn message(id: &str, parent_message_id: Option<&str>) -> Message {
        Message {
            id: id.to_string(),
            conversation_id: "conversation-1".to_string(),
            parent_message_id: parent_message_id.map(str::to_string),
            revision_of_message_id: None,
            path_index: 1,
            role: "user".to_string(),
            content: "Hello".to_string(),
            status: "complete".to_string(),
            created_at: "2026-07-02T00:00:00Z".to_string(),
            updated_at: "2026-07-02T00:00:00Z".to_string(),
            provider_id: None,
            model_id: None,
            token_count: None,
            error_message: None,
            metadata_json: None,
            branch_name: None,
        }
    }

    #[test]
    fn validates_valid_conversation_export() {
        let export = export_with_messages(vec![
            message("message-1", None),
            message("message-2", Some("message-1")),
        ]);

        validate_conversation_export(&export).expect("valid export");
    }

    #[test]
    fn rejects_missing_parent_reference() {
        let export = export_with_messages(vec![message("message-1", Some("missing"))]);

        let error = validate_conversation_export(&export).expect_err("invalid export");
        assert_eq!(error.code, "invalid_input");
    }

    #[test]
    fn rejects_forward_parent_reference() {
        let export = export_with_messages(vec![
            message("message-2", Some("message-1")),
            message("message-1", None),
        ]);

        let error = validate_conversation_export(&export).expect_err("invalid export");
        assert_eq!(error.code, "invalid_input");
    }

    #[test]
    fn rejects_message_count_over_the_import_ceiling() {
        let mut messages = Vec::with_capacity(MAX_IMPORT_MESSAGES + 1);
        let mut parent: Option<String> = None;
        for i in 0..=MAX_IMPORT_MESSAGES {
            let id = format!("message-{i}");
            messages.push(message(&id, parent.as_deref()));
            parent = Some(id);
        }
        let export = export_with_messages(messages);

        let error = validate_conversation_export(&export)
            .expect_err("must reject over-limit message count");
        assert_eq!(error.code, "invalid_input");
        assert!(error.message.contains(&MAX_IMPORT_MESSAGES.to_string()));
    }

    #[test]
    fn accepts_message_count_exactly_at_the_import_ceiling() {
        let mut messages = Vec::with_capacity(MAX_IMPORT_MESSAGES);
        for i in 0..MAX_IMPORT_MESSAGES {
            let id = format!("message-{i}");
            // Keep this fixture shallow so it isolates the count ceiling from the separately
            // tested branch-depth ceiling.
            messages.push(message(&id, None));
        }
        let export = export_with_messages(messages);

        validate_conversation_export(&export)
            .expect("exactly-at-limit message count must be accepted");
    }

    #[test]
    fn rejects_a_single_message_over_the_content_length_ceiling() {
        let mut oversized = message("message-1", None);
        oversized.content = "a".repeat(MAX_MESSAGE_CONTENT_CHARS + 1);
        let export = export_with_messages(vec![oversized]);

        let error = validate_conversation_export(&export)
            .expect_err("must reject over-limit message content");
        assert_eq!(error.code, "invalid_input");
    }

    #[test]
    fn accepts_message_content_exactly_at_the_length_ceiling() {
        let mut at_limit = message("message-1", None);
        at_limit.content = "a".repeat(MAX_MESSAGE_CONTENT_CHARS);
        let export = export_with_messages(vec![at_limit]);

        validate_conversation_export(&export)
            .expect("exactly-at-limit content length must be accepted");
    }

    /// COR-009: a full serialize → deserialize round trip through the actual export JSON
    /// format, covering the specific content classes the plan calls out — Unicode (emoji,
    /// CJK, RTL, combining marks), an append-only branch/revision structure, and per-message
    /// provenance (provider/model/token metadata) — proving none of it is corrupted or
    /// dropped by the JSON boundary, and that the round-tripped structure still validates.
    #[test]
    fn conversation_export_survives_a_full_json_round_trip_with_unicode_and_branches() {
        let mut root = message("msg-1", None);
        root.role = "user".to_string();
        root.content = "セキュリティ監査 🔒 — how do I keep this café's wifi safe? Ω≠Σ".to_string();

        let mut first_reply = message("msg-2", Some("msg-1"));
        first_reply.role = "assistant".to_string();
        first_reply.content = "אבטחה חשובה מאוד. 这是第一个回答。 🚀".to_string(); // RTL Hebrew + CJK + emoji
        first_reply.provider_id = Some("ollama".to_string());
        first_reply.model_id = Some("llama3.2:latest".to_string());
        first_reply.token_count = Some(42);
        first_reply.metadata_json = Some("{\"custom\":\"value with a \\\"quote\\\"\"}".to_string());

        // A regenerated branch of the same assistant turn — append-only, references msg-2 via
        // revision_of_message_id rather than replacing it.
        let mut second_reply = message("msg-3", Some("msg-1"));
        second_reply.role = "assistant".to_string();
        second_reply.revision_of_message_id = Some("msg-2".to_string());
        second_reply.content =
            "e\u{0301}e\u{0301}e\u{0301} — combining marks, and a null-adjacent edge case: \u{0}"
                .to_string();
        second_reply.status = "interrupted".to_string();
        second_reply.error_message =
            Some("Generation was interrupted before Ark could finish.".to_string());

        let original = export_with_messages(vec![root, first_reply, second_reply]);

        let json = serde_json::to_string(&original).expect("export serializes to JSON");
        let round_tripped: ConversationExport =
            serde_json::from_str(&json).expect("export deserializes from JSON");

        validate_conversation_export(&round_tripped)
            .expect("round-tripped export must still validate");

        assert_eq!(round_tripped.messages.len(), original.messages.len());
        for (original_message, round_tripped_message) in
            original.messages.iter().zip(round_tripped.messages.iter())
        {
            assert_eq!(round_tripped_message.id, original_message.id);
            assert_eq!(
                round_tripped_message.content, original_message.content,
                "message content (including Unicode) must survive the round trip exactly"
            );
            assert_eq!(
                round_tripped_message.parent_message_id,
                original_message.parent_message_id
            );
            assert_eq!(
                round_tripped_message.revision_of_message_id,
                original_message.revision_of_message_id
            );
            assert_eq!(round_tripped_message.status, original_message.status);
            assert_eq!(
                round_tripped_message.provider_id,
                original_message.provider_id
            );
            assert_eq!(round_tripped_message.model_id, original_message.model_id);
            assert_eq!(
                round_tripped_message.token_count,
                original_message.token_count
            );
            assert_eq!(
                round_tripped_message.error_message,
                original_message.error_message
            );
            assert_eq!(
                round_tripped_message.metadata_json,
                original_message.metadata_json
            );
        }
    }

    #[test]
    fn conversation_export_round_trip_preserves_conversation_level_settings() {
        let mut export = export_with_messages(vec![message("msg-1", None)]);
        export.conversation.system_prompt =
            Some("You are a helpful assistant. 你好世界 🌍".to_string());
        export.conversation.temperature = Some(0.42);
        export.conversation.max_tokens = Some(4096);
        export.conversation.archived = true;
        export.conversation.provider_id = Some("ollama".to_string());
        export.conversation.model_id = Some("llama3.2:latest".to_string());

        let json = serde_json::to_string(&export).expect("serializes");
        let round_tripped: ConversationExport = serde_json::from_str(&json).expect("deserializes");

        assert_eq!(
            round_tripped.conversation.system_prompt,
            export.conversation.system_prompt
        );
        assert_eq!(
            round_tripped.conversation.temperature,
            export.conversation.temperature
        );
        assert_eq!(
            round_tripped.conversation.max_tokens,
            export.conversation.max_tokens
        );
        assert_eq!(
            round_tripped.conversation.archived,
            export.conversation.archived
        );
        assert_eq!(
            round_tripped.conversation.provider_id,
            export.conversation.provider_id
        );
        assert_eq!(
            round_tripped.conversation.model_id,
            export.conversation.model_id
        );
    }

    #[test]
    fn rejects_an_export_with_an_unknown_future_schema_version() {
        let mut export = export_with_messages(vec![message("msg-1", None)]);
        export.schema_version = CONVERSATION_EXPORT_SCHEMA_VERSION + 1;

        let error = validate_conversation_export(&export)
            .expect_err("a newer schema version must be rejected");
        assert_eq!(error.code, "invalid_input");
    }

    #[test]
    fn rejects_malformed_json_gracefully() {
        let result: Result<ConversationExport, _> =
            serde_json::from_str("{ this is not valid json");
        assert!(
            result.is_err(),
            "malformed JSON must fail deserialization, not panic"
        );

        let result: Result<ConversationExport, _> =
            serde_json::from_str("{\"schemaVersion\": \"not a number\"}");
        assert!(
            result.is_err(),
            "type-mismatched fields must fail deserialization cleanly"
        );
    }

    #[test]
    fn branch_depth_limit_accepts_the_boundary_and_rejects_one_over() {
        let make_chain = |count: usize| {
            let mut messages = Vec::with_capacity(count);
            for index in 0..count {
                let id = format!("message-{index}");
                let parent = (index > 0).then(|| format!("message-{}", index - 1));
                messages.push(message(&id, parent.as_deref()));
            }
            messages
        };

        validate_conversation_export(&export_with_messages(make_chain(MAX_IMPORT_BRANCH_DEPTH)))
            .expect("exact branch-depth boundary is valid");
        let error = validate_conversation_export(&export_with_messages(make_chain(
            MAX_IMPORT_BRANCH_DEPTH + 1,
        )))
        .expect_err("one over the branch-depth boundary must fail");
        assert_eq!(error.code, "invalid_input");
        assert!(error.message.contains("branch depth"));
    }
}
