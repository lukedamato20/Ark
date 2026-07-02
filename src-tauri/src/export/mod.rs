use crate::chat::{Conversation, Message};
use crate::errors::AppError;
use crate::providers::ProviderConfig;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

pub const CONVERSATION_EXPORT_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationExport {
    pub schema_version: u32,
    pub exported_at: String,
    pub conversation: Conversation,
    pub messages: Vec<Message>,
    pub provider: Option<ProviderConfig>,
}

pub fn validate_conversation_export(export: &ConversationExport) -> Result<(), AppError> {
    if export.schema_version != CONVERSATION_EXPORT_SCHEMA_VERSION {
        return Err(AppError::invalid_input("Unsupported conversation export schema version."));
    }

    if export.conversation.id.trim().is_empty() {
        return Err(AppError::invalid_input("Conversation export is missing a conversation ID."));
    }

    if export.conversation.title.trim().is_empty() {
        return Err(AppError::invalid_input("Conversation export is missing a conversation title."));
    }

    let mut seen_ids = HashSet::new();
    for message in &export.messages {
        if message.id.trim().is_empty() {
            return Err(AppError::invalid_input("Conversation export contains a message without an ID."));
        }
        if !seen_ids.insert(message.id.as_str()) {
            return Err(AppError::invalid_input("Conversation export contains duplicate message IDs."));
        }
        if message.conversation_id != export.conversation.id {
            return Err(AppError::invalid_input(
                "Conversation export contains a message from a different conversation.",
            ));
        }
        if !is_valid_role(&message.role) {
            return Err(AppError::invalid_input("Conversation export contains an invalid message role."));
        }
        if !is_valid_status(&message.status) {
            return Err(AppError::invalid_input("Conversation export contains an invalid message status."));
        }
    }

    let all_ids = export
        .messages
        .iter()
        .map(|message| message.id.as_str())
        .collect::<HashSet<_>>();
    let mut importable_ids = HashSet::new();
    for message in &export.messages {
        validate_message_reference(message.parent_message_id.as_deref(), &all_ids, &importable_ids, "parent")?;
        validate_message_reference(
            message.revision_of_message_id.as_deref(),
            &all_ids,
            &importable_ids,
            "revision",
        )?;
        importable_ids.insert(message.id.as_str());
    }

    if let Some(current_message_id) = export.conversation.current_message_id.as_deref() {
        if !all_ids.contains(current_message_id) {
            return Err(AppError::invalid_input(
                "Conversation export current message does not exist in the message list.",
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
    matches!(status, "pending" | "streaming" | "complete" | "failed" | "cancelled")
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
                streaming_enabled: true,
                archived: false,
            },
            messages,
            provider: None,
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
        }
    }

    #[test]
    fn validates_valid_conversation_export() {
        let export = export_with_messages(vec![message("message-1", None), message("message-2", Some("message-1"))]);

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
        let export = export_with_messages(vec![message("message-2", Some("message-1")), message("message-1", None)]);

        let error = validate_conversation_export(&export).expect_err("invalid export");
        assert_eq!(error.code, "invalid_input");
    }
}
