use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Message {
    pub id: String,
    pub conversation_id: String,
    pub parent_message_id: Option<String>,
    pub revision_of_message_id: Option<String>,
    pub path_index: i64,
    pub role: String,
    pub content: String,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
    pub provider_id: Option<String>,
    pub model_id: Option<String>,
    pub token_count: Option<i64>,
    pub error_message: Option<String>,
    pub metadata_json: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BranchAlternative {
    pub message_id: String,
    pub revision_of_message_id: Option<String>,
    pub created_at: String,
    pub status: String,
    pub content_preview: String,
    pub is_active: bool,
    pub has_descendants: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Conversation {
    pub id: String,
    pub title: String,
    pub created_at: String,
    pub updated_at: String,
    pub provider_id: Option<String>,
    pub model_id: Option<String>,
    pub current_message_id: Option<String>,
    pub system_prompt: Option<String>,
    pub temperature: Option<f64>,
    pub max_tokens: Option<i64>,
    pub streaming_enabled: bool,
    pub archived: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendChatRequest {
    pub conversation_id: String,
    pub content: String,
    pub provider_id: String,
    pub model: String,
    pub temperature: Option<f64>,
    pub max_tokens: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendChatResult {
    pub conversation_id: String,
    pub user_message_id: String,
    pub assistant_message_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamEvent {
    pub conversation_id: String,
    pub message_id: String,
    pub delta: Option<String>,
    pub content: Option<String>,
    pub status: String,
    pub error: Option<String>,
}
