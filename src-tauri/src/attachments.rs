//! CMP-001: a text-file attachment a user attaches to an outgoing message. See
//! `migrations/0011_attachments.sql` for the schema this mirrors and why content lives in a
//! plain column rather than the filesystem.

use serde::{Deserialize, Serialize};

/// Never carries `content` — this is the summary shape returned by list/attach/link, kept small
/// so loading a conversation's attachment list doesn't re-send potentially-large text bodies for
/// every row. Full content is fetched on demand via `Database::get_attachment_content`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Attachment {
    pub id: String,
    pub conversation_id: String,
    /// `None` means staged — uploaded but not yet linked to a sent message. Set once
    /// `send_chat_message` links it inside the same transaction that creates the user message.
    pub message_id: Option<String>,
    pub file_name: String,
    pub byte_size: i64,
    pub sha256: String,
    pub created_at: String,
}
