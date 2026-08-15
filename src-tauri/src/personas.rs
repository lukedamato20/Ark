//! FTR-003: a persona is a reusable, named instruction identity (e.g. "terse code reviewer") a
//! conversation can be assigned to — the second half of this plan item, deferred out of the
//! original projects-only pass. See `migrations/0010_personas.sql` for the versioned schema this
//! mirrors: `Persona` here is always the *current* version's content joined onto the persona's
//! own mutable metadata (name, archive state), never a raw `personas` row on its own.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Persona {
    pub id: String,
    pub name: String,
    /// The current version's instructions — unlike a project's `instructions`, never `None`: a
    /// persona's entire purpose is its prompt content, so one is required at creation.
    pub instructions: String,
    pub default_temperature: Option<f64>,
    pub default_max_tokens: Option<i64>,
    /// Which `persona_versions` version this is — visible so "documented and visible" (FTR-003
    /// criterion 1) extends to versioning too: a user can tell a persona has been revised.
    pub version_number: i64,
    /// `None` means active, matching `Project.archived_at`'s convention.
    pub archived_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// One entry in a persona's version history — `Database::list_persona_versions`. Deliberately a
/// separate, smaller type from `Persona` rather than reusing it: a version has no name or archive
/// state of its own (those live on the persona, not the version).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PersonaVersionSummary {
    pub id: String,
    pub version_number: i64,
    pub instructions: String,
    pub default_temperature: Option<f64>,
    pub default_max_tokens: Option<i64>,
    pub created_at: String,
}

/// The result of `Database::preview_persona_deletion` — mirrors `ProjectDeletionPreview` exactly:
/// deleting a persona never deletes the conversations assigned to it, only unassigns them, but
/// the count is still surfaced so the user isn't surprised.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PersonaDeletionPreview {
    pub persona: Persona,
    pub conversation_count: i64,
}
