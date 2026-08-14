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
    /// ARC-006: reserved for a future per-conversation custom-system-prompt feature (Phase 5
    /// `FTR`); deliberately unimplemented today — no command writes a non-null value here. Kept
    /// rather than removed because the feature is a clearly intended, near-term addition and the
    /// column's shape won't need to change when it lands.
    pub system_prompt: Option<String>,
    pub temperature: Option<f64>,
    pub max_tokens: Option<i64>,
    pub archived: bool,
    /// ARC-007: nullable until FTR-003 introduces project entities and mutations. Exposed now
    /// so the paginated history contract can already filter by project without later changing
    /// its response shape.
    pub project_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationListRequest {
    pub limit: Option<u32>,
    pub cursor: Option<String>,
    pub query: Option<String>,
    /// `Some(false)` lists active conversations, `Some(true)` lists archived conversations,
    /// and `None` includes both.
    pub archived: Option<bool>,
    pub project_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationPage {
    pub items: Vec<Conversation>,
    pub next_cursor: Option<String>,
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

/// ARC-002: the current `StreamEvent` schema version. Bump this — and document the change in
/// `docs/protocol-versioning.md` — whenever a field is added, removed, or given new meaning in
/// a way an older frontend build could not safely ignore. A frontend build that only knows an
/// older version drops any event carrying a version it doesn't recognize (see
/// `KNOWN_STREAM_EVENT_SCHEMA_VERSION` in `src/lib/ArkClient.ts`) rather than misinterpreting it.
pub const STREAM_EVENT_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamEvent {
    pub conversation_id: String,
    pub message_id: String,
    pub delta: Option<String>,
    pub content: Option<String>,
    pub status: String,
    pub error: Option<String>,
    /// COR-002 (partial): a per-message, monotonically increasing sequence number starting
    /// at 1 for the first `chat:stream-delta` event. Lets the frontend detect a missed or
    /// out-of-order delta (revision != last seen + 1) and fall back to an authoritative
    /// refetch instead of silently corrupting client-accumulated content — see guiding
    /// principle "prefer events as invalidation/delta notifications with a monotonically
    /// increasing revision." `None` on non-delta events (start/complete/error/cancelled/
    /// interrupted), which always carry authoritative full content instead.
    pub revision: Option<i64>,
    /// ARC-002: identifies which version of this event's shape the sender used. See
    /// `STREAM_EVENT_SCHEMA_VERSION`.
    pub schema_version: u32,
}
