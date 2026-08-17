//! FTR-003: a persona is a reusable, named instruction identity (e.g. "terse code reviewer") a
//! conversation can be assigned to — the second half of this plan item, deferred out of the
//! original projects-only pass. See `migrations/0010_personas.sql` for the versioned schema this
//! mirrors: `Persona` here is always the *current* version's content joined onto the persona's
//! own mutable metadata (name, archive state), never a raw `personas` row on its own.

use crate::db::{now, Database};
use crate::errors::AppError;
use chrono::DateTime;
use serde::{Deserialize, Serialize};

pub const PERSONA_EXPORT_SCHEMA_VERSION: u32 = 1;
pub const MAX_PERSONA_EXPORT_BYTES: usize = 5 * 1024 * 1024;
const MAX_PERSONA_EXPORT_VERSIONS: usize = 1_000;
const MAX_PERSONA_NAME_CHARS: usize = 200;

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
    /// UX: an Ark-level behavioral preset, versioned alongside `instructions`/the defaults above
    /// (not the mutable `personas` row) — see `Conversation::response_style`'s doc comment.
    /// Changing it creates a new immutable version, same as changing `instructions`.
    pub response_style: Option<String>,
    pub tone: Option<String>,
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
    pub response_style: Option<String>,
    pub tone: Option<String>,
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

/// FTR-003 criterion 2: a self-contained, portable persona artifact. Version IDs remain in the
/// export as provenance, but import deliberately creates new local IDs so an artifact can be
/// imported alongside its source without collisions. Prompt content and revision timestamps are
/// preserved exactly.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PersonaExport {
    pub schema_version: u32,
    pub exported_at: String,
    pub persona: Persona,
    pub versions: Vec<PersonaVersionSummary>,
}

pub fn validate_persona_name(value: &str) -> Result<String, AppError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(AppError::invalid_input("Persona name cannot be empty."));
    }
    if trimmed.chars().count() > MAX_PERSONA_NAME_CHARS {
        return Err(AppError::invalid_input(format!(
            "Persona name must be at most {MAX_PERSONA_NAME_CHARS} characters."
        )));
    }
    Ok(trimmed.to_string())
}

fn validate_timestamp(value: &str, label: &str) -> Result<(), AppError> {
    DateTime::parse_from_rfc3339(value).map_err(|_| {
        AppError::invalid_input(format!(
            "Persona import contains an invalid {label} timestamp."
        ))
    })?;
    Ok(())
}

fn validate_persona_export(export: &PersonaExport) -> Result<(), AppError> {
    if export.schema_version != PERSONA_EXPORT_SCHEMA_VERSION {
        return Err(AppError::invalid_input(format!(
            "Unsupported persona export schema version {}. This Ark build supports version {PERSONA_EXPORT_SCHEMA_VERSION}.",
            export.schema_version
        )));
    }
    validate_timestamp(&export.exported_at, "exported-at")?;
    crate::validation::validate_entity_id(&export.persona.id, "Persona ID")?;
    validate_persona_name(&export.persona.name)?;
    crate::validation::validate_persona_instructions(&export.persona.instructions)?;
    crate::validation::validate_temperature(export.persona.default_temperature)?;
    crate::validation::validate_max_tokens(export.persona.default_max_tokens)?;
    crate::validation::validate_response_style(export.persona.response_style.clone())?;
    crate::validation::validate_tone(export.persona.tone.clone())?;
    validate_timestamp(&export.persona.created_at, "persona created-at")?;
    validate_timestamp(&export.persona.updated_at, "persona updated-at")?;
    if let Some(archived_at) = &export.persona.archived_at {
        validate_timestamp(archived_at, "persona archived-at")?;
    }

    if export.versions.is_empty() {
        return Err(AppError::invalid_input(
            "Persona import must contain at least one prompt version.",
        ));
    }
    if export.versions.len() > MAX_PERSONA_EXPORT_VERSIONS {
        return Err(AppError::invalid_input(format!(
            "Persona import contains too many prompt versions. The limit is {MAX_PERSONA_EXPORT_VERSIONS}."
        )));
    }

    let mut versions = export.versions.iter().collect::<Vec<_>>();
    versions.sort_by_key(|version| version.version_number);
    for (index, version) in versions.iter().enumerate() {
        let expected_number = i64::try_from(index + 1)
            .map_err(|_| AppError::invalid_input("Persona version number is out of range."))?;
        if version.version_number != expected_number {
            return Err(AppError::invalid_input(
                "Persona prompt versions must be unique and contiguous starting at version 1.",
            ));
        }
        crate::validation::validate_entity_id(&version.id, "Persona version ID")?;
        crate::validation::validate_persona_instructions(&version.instructions)?;
        crate::validation::validate_temperature(version.default_temperature)?;
        crate::validation::validate_max_tokens(version.default_max_tokens)?;
        crate::validation::validate_response_style(version.response_style.clone())?;
        crate::validation::validate_tone(version.tone.clone())?;
        validate_timestamp(&version.created_at, "prompt-version created-at")?;
    }

    let current = versions
        .last()
        .expect("a non-empty version list has a final version");
    if export.persona.version_number != current.version_number
        || export.persona.instructions != current.instructions
        || export.persona.default_temperature != current.default_temperature
        || export.persona.default_max_tokens != current.default_max_tokens
        || export.persona.response_style != current.response_style
        || export.persona.tone != current.tone
    {
        return Err(AppError::invalid_input(
            "Persona import's current prompt does not match its latest version.",
        ));
    }
    Ok(())
}

pub fn export_persona_json(db: &Database, id: &str) -> Result<String, AppError> {
    let export = PersonaExport {
        schema_version: PERSONA_EXPORT_SCHEMA_VERSION,
        exported_at: now(),
        persona: db.get_persona(id)?,
        versions: db.list_persona_versions(id)?,
    };
    serde_json::to_string_pretty(&export).map_err(|error| {
        AppError::new(
            "export_error",
            format!("Could not serialize persona export: {error}"),
        )
    })
}

pub fn import_persona_json(db: &Database, json: &str) -> Result<Persona, AppError> {
    if json.len() > MAX_PERSONA_EXPORT_BYTES {
        return Err(AppError::invalid_input(format!(
            "Persona import is too large. The limit is {} MB.",
            MAX_PERSONA_EXPORT_BYTES / (1024 * 1024)
        )));
    }
    let mut export: PersonaExport = serde_json::from_str(json)
        .map_err(|error| AppError::invalid_input(format!("Invalid persona JSON: {error}")))?;
    validate_persona_export(&export)?;
    // The database inserts versions in ascending order and points the persona at the final one.
    // Export order is intentionally presentation-independent (the current UI emits newest first).
    export
        .versions
        .sort_by_key(|version| version.version_number);
    db.import_persona(&export)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_db() -> (Database, std::path::PathBuf) {
        let path = std::env::temp_dir().join(format!(
            "ark-persona-portability-test-{}.sqlite3",
            uuid::Uuid::new_v4()
        ));
        let db = Database::open(&path).expect("database opens");
        (db, path)
    }

    #[test]
    fn persona_export_import_preserves_immutable_history_with_new_local_ids() {
        let (db, path) = test_db();
        let source = db
            .create_persona(
                "Reviewer",
                "Review carefully.",
                Some(0.2),
                Some(2_000),
                Some("technical"),
                Some("direct"),
            )
            .expect("persona created");
        let source = db
            .update_persona(
                &source.id,
                "Reviewer",
                "Review carefully and cite line numbers.",
                Some(0.1),
                Some(3_000),
                Some("concise"),
                Some("professional"),
            )
            .expect("second version created");
        let source = db
            .set_persona_archived(&source.id, true)
            .expect("persona archived");
        let source_versions = db.list_persona_versions(&source.id).expect("versions load");

        let json = export_persona_json(&db, &source.id).expect("persona exports");
        let imported = import_persona_json(&db, &json).expect("persona imports");
        let imported_versions = db
            .list_persona_versions(&imported.id)
            .expect("imported versions load");

        assert_ne!(imported.id, source.id);
        assert_eq!(imported.name, source.name);
        assert_eq!(imported.instructions, source.instructions);
        assert_eq!(imported.version_number, source.version_number);
        assert_eq!(imported.archived_at, source.archived_at);
        assert_eq!(imported.created_at, source.created_at);
        assert_eq!(imported.updated_at, source.updated_at);
        assert_eq!(imported_versions.len(), source_versions.len());
        for (imported_version, source_version) in
            imported_versions.iter().zip(source_versions.iter())
        {
            assert_ne!(imported_version.id, source_version.id);
            assert_eq!(
                imported_version.version_number,
                source_version.version_number
            );
            assert_eq!(imported_version.instructions, source_version.instructions);
            assert_eq!(
                imported_version.default_temperature,
                source_version.default_temperature
            );
            assert_eq!(
                imported_version.default_max_tokens,
                source_version.default_max_tokens
            );
            assert_eq!(
                imported_version.response_style,
                source_version.response_style
            );
            assert_eq!(imported_version.tone, source_version.tone);
            assert_eq!(imported_version.created_at, source_version.created_at);
        }
        drop(db);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn malformed_persona_history_is_rejected_without_writes() {
        let (db, path) = test_db();
        let source = db
            .create_persona("Reviewer", "Review carefully.", None, None, None, None)
            .expect("persona created");
        let json = export_persona_json(&db, &source.id).expect("persona exports");
        let mut export: PersonaExport = serde_json::from_str(&json).expect("export parses");
        export.versions[0].version_number = 2;
        export.persona.version_number = 2;
        let tampered = serde_json::to_string(&export).expect("tampered export serializes");

        let before_count = db.list_personas().expect("personas load").len();
        let error = import_persona_json(&db, &tampered).expect_err("bad history is rejected");
        assert_eq!(error.code, "invalid_input");
        assert_eq!(
            db.list_personas().expect("personas load").len(),
            before_count
        );
        drop(db);
        let _ = std::fs::remove_file(path);
    }
}
