//! FTR-003: a project groups conversations under a shared name, instructions, and default
//! provider/model/settings — the first of this plan item's two halves (personas/prompt library
//! are deferred). See `migrations/0008_projects.sql` for the schema this mirrors.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: String,
    pub name: String,
    /// `None` means no project-level instructions are injected. See `generation.rs`'s
    /// `resolve_system_prompt` for how this composes with a conversation's own override.
    pub instructions: Option<String>,
    pub default_provider_id: Option<String>,
    pub default_model_id: Option<String>,
    pub default_temperature: Option<f64>,
    pub default_max_tokens: Option<i64>,
    /// `None` means active. An ISO timestamp rather than a bare boolean, matching the
    /// `pinned_at`/`archived_at` convention already established for conversations.
    pub archived_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// Grouped to keep `Database::update_project`'s argument count within clippy's
/// `too_many_arguments` threshold, matching the existing `UpdateProviderChanges` convention.
pub struct UpdateProjectChanges<'a> {
    pub name: &'a str,
    pub instructions: Option<&'a str>,
    pub default_provider_id: Option<&'a str>,
    pub default_model_id: Option<&'a str>,
    pub default_temperature: Option<f64>,
    pub default_max_tokens: Option<i64>,
}

/// The result of `Database::preview_project_deletion`: what deleting this project would affect,
/// shown to the user before they confirm. Deleting a project never deletes its conversations —
/// only unassigns them (`project_id` -> `NULL`) — but the count still needs to be surfaced so
/// "safely" in this task's acceptance criteria means "the user isn't surprised," not just
/// "nothing is destroyed."
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectDeletionPreview {
    pub project: Project,
    pub conversation_count: i64,
}
