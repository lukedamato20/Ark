use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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
    /// FTR-005: `None` means unnamed — see migration `0009_message_branch_names.sql`'s doc
    /// comment for why this lives on the message itself rather than a separate branch entity.
    pub branch_name: Option<String>,
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
    /// FTR-005: `None` means unnamed — the frontend falls back to an ordinal "Response N" label.
    pub branch_name: Option<String>,
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
    /// FTR-004: `None` means "no conversation-level override, inherit the effective provider/
    /// project default." Set via `update_conversation_settings` and applied by `generation.rs`.
    pub system_prompt: Option<String>,
    pub temperature: Option<f64>,
    pub max_tokens: Option<i64>,
    pub archived: bool,
    /// FTR-003: `None` means unassigned. Set via `Database::set_conversation_project`, which
    /// validates the referenced project exists first — see `projects.rs`'s module doc.
    pub project_id: Option<String>,
    /// FTR-002: `None` means unpinned. A timestamp rather than a bare boolean so pin order
    /// among multiple pinned conversations is deterministic (most-recently-pinned first) —
    /// see migration `0007_conversation_pinning.sql`'s doc comment.
    pub pinned_at: Option<String>,
    /// FTR-003: `None` means unassigned. Independent of `project_id` — a project groups
    /// conversations by subject, a persona defines how the assistant behaves; both can be set
    /// on the same conversation. Set via `Database::set_conversation_persona`, which validates
    /// the referenced persona exists first — see `personas.rs`'s module doc.
    pub persona_id: Option<String>,
    /// UX: `None` means "no conversation-level override, inherit persona/project." Ark-level
    /// behavioral preset (see `generation.rs`'s `response_style_instruction`) — not a real
    /// provider parameter, unlike `temperature`. Validated against a fixed allow-list by
    /// `validation::validate_response_style`.
    pub response_style: Option<String>,
    /// UX: mirrors `response_style` — see `validation::validate_tone`.
    pub tone: Option<String>,
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
    /// FTR-002: conversation id -> a short plain-text excerpt of the matching title/message
    /// content, present only for the conversations in `items` that a search query actually
    /// matched (empty when `ConversationListRequest.query` is unset). A map keyed by id rather
    /// than a field on `Conversation` itself, since a snippet only has meaning in the context
    /// of a search result — `Conversation` is also returned from non-search paths (`get_conversation`,
    /// single-conversation reads) where "which query matched this" doesn't apply.
    pub search_snippets: HashMap<String, String>,
}

/// PERF-003: the bounded response `get_conversation_messages` returns — see
/// `Database::get_active_messages_page`'s own doc comment for what "bounded" means and why it
/// exists alongside (not instead of) the unbounded `get_active_messages` every other caller
/// still uses.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationMessagePage {
    pub messages: Vec<Message>,
    /// `true` when the oldest message in `messages` still has a parent that wasn't loaded —
    /// the frontend's signal to show a "Load earlier messages" affordance.
    pub has_more_older: bool,
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
    /// CMP-001: ids of staged attachments (`Attachment.message_id` still `None`) to link to the
    /// new user message and disclose to the provider. `None`/empty means no attachments — the
    /// vast majority of sends, so this stays optional rather than requiring every existing
    /// caller to pass an empty array.
    #[serde(default)]
    pub attachment_ids: Vec<String>,
    /// CMP-004: already-fetched web search results (the frontend calls `search_web` — its own
    /// preview/approval-gated command — before `sendChatMessage`, since a network call cannot
    /// happen inside this request's DB transaction). `None` means search was not used for this
    /// send.
    #[serde(default)]
    pub web_search: Option<WebSearchInput>,
}

/// CMP-004: what the frontend already fetched via `search_web`, threaded through as plain data —
/// `generation.rs` composes it into the outgoing provider message and provenance record; it
/// performs no network I/O of its own.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebSearchInput {
    pub query: String,
    pub citations: Vec<crate::web_search::SearchCitation>,
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
