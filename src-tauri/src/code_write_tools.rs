//! CODE-005 (partial): the `edit_file` write-capable Repository tool.
//!
//! This is the first tool in Ark Code that mutates Repository state, and the first consumer of
//! the ADR 0003 approval-hash binding (`code_sessions::compute_call_hash`/`compute_preview_hash`/
//! `compute_precondition_hash`). It follows the ADR's file-operation recovery verifier exactly:
//! a proposal (`preview_edit_file`) records the file's current content hash (`before_hash`) and
//! the hash the file must have after a successful write (`expected_after_hash`); an approved
//! execution (`execute_edit_file`) re-derives all three approval hashes from the *current* state
//! of the repository and refuses outright if any of them no longer match what was approved,
//! before writing anything. The write itself is a same-directory temp-file write plus an atomic
//! rename, and the result is always classified into exactly one of the ADR's four recovery
//! outcomes by re-reading the file after the rename — a diverged file is surfaced, never
//! silently retried or overwritten.
//!
//! `edit_file` reuses CODE-004's `RepositoryContext`, `enforce_tool`, and `relative_display` from
//! `code_tools` rather than duplicating Repository confinement, and is registered in that
//! module's `ark_code_tools()` so it appears in the same tool registry — but, per that module's
//! The agent may create a durable proposal, but execution remains a separate hash-bound command
//! that requires an explicit local-user approval. Model output can therefore never dispatch this
//! write directly.
//!
//! Git-branch-scoped checkpoint/rollback and an allowlisted command-execution tool are the other
//! two write-capable tools CODE-005 calls for; both remain unimplemented (see
//! `implementation-plan.md`'s CODE-005 entry for why).

use crate::code_sessions;
use crate::code_tools::{enforce_tool, relative_display, repository_read_error, RepositoryContext};
use crate::errors::AppError;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use uuid::Uuid;

pub const EDIT_FILE_TOOL_ID: &str = "edit_file";

const MAX_EDIT_FILE_BYTES: u64 = 1024 * 1024;
const MAX_EDIT_BLOCKS: usize = 20;
const MAX_EDIT_BLOCK_CHARS: usize = 20_000;
const DIFF_CONTEXT_LINES: usize = 3;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EditBlock {
    pub search: String,
    pub replace: String,
}

/// The typed shape `call_hash` is computed over. Field order (and therefore the hash) is fixed
/// by this struct definition, not by whatever order a caller happened to send JSON keys in.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EditFileArguments {
    pub path: String,
    pub edits: Vec<EditBlock>,
}

/// The typed shape `precondition_hash` is computed over: which file, and what it must currently
/// contain for this proposal to still apply cleanly.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct EditFilePreconditions<'a> {
    path: &'a str,
    before_hash: &'a str,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditFilePreview {
    pub path: String,
    pub diff: String,
    pub before_hash: String,
    pub expected_after_hash: String,
    pub call_hash: String,
    pub preview_hash: String,
    pub precondition_hash: String,
}

/// What the frontend must echo back unchanged from the `EditFilePreview` it obtained and the user
/// approved. Any divergence from what a fresh proposal against current state would produce is
/// rejected before any write occurs — see the module doc comment.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ApprovedEditFile {
    pub path: String,
    pub edits: Vec<EditBlock>,
    pub call_hash: String,
    pub preview_hash: String,
    pub precondition_hash: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditFileOutcome {
    pub path: String,
    pub before_hash: String,
    pub expected_after_hash: String,
    pub observed_after_hash: String,
    pub outcome: code_sessions::CodeRecoveryOutcome,
}

/// Strictly decodes and canonicalizes a provider proposal before previewing it. The returned
/// JSON is the exact argument representation persisted beside the approval hashes.
pub fn preview_provider_edit_file(
    context: &RepositoryContext,
    arguments: &serde_json::Value,
) -> Result<(String, EditFilePreview), AppError> {
    let arguments: EditFileArguments = serde_json::from_value(arguments.clone()).map_err(|_| {
        AppError::invalid_input("edit_file arguments do not match the declared tool schema.")
    })?;
    let canonical = code_sessions::serialize_json(&arguments)?;
    let preview = preview_edit_file(context, &arguments.path, arguments.edits)?;
    Ok((canonical, preview))
}

pub fn preview_edit_file(
    context: &RepositoryContext,
    relative_path: &str,
    edits: Vec<EditBlock>,
) -> Result<EditFilePreview, AppError> {
    enforce_tool(context, EDIT_FILE_TOOL_ID)?;
    let (_path, relative, content) = read_editable_file(context, relative_path)?;
    let before_hash = code_sessions::sha256_hex(content.as_bytes());
    let applied = apply_edits_with_diff(&content, &edits)?;
    let expected_after_hash = code_sessions::sha256_hex(applied.result.as_bytes());

    let call_hash = code_sessions::compute_call_hash(&EditFileArguments {
        path: relative.clone(),
        edits,
    })?;
    let preview_hash = code_sessions::compute_preview_hash(&applied.diff);
    let precondition_hash = code_sessions::compute_precondition_hash(&EditFilePreconditions {
        path: &relative,
        before_hash: &before_hash,
    })?;

    Ok(EditFilePreview {
        path: relative,
        diff: applied.diff,
        before_hash,
        expected_after_hash,
        call_hash,
        preview_hash,
        precondition_hash,
    })
}

pub fn execute_edit_file(
    context: &RepositoryContext,
    approved: ApprovedEditFile,
) -> Result<EditFileOutcome, AppError> {
    enforce_tool(context, EDIT_FILE_TOOL_ID)?;
    let (path, relative, content) = read_editable_file(context, &approved.path)?;
    let before_hash = code_sessions::sha256_hex(content.as_bytes());

    // Re-derive every approval-bound hash from what is proposed right now and refuse before
    // writing anything if the approval no longer matches: a stale `call_hash`/`preview_hash`
    // means the approved edit itself was tampered with or targets different content than shown;
    // a stale `precondition_hash` means the file changed since the user approved this diff.
    let call_hash = code_sessions::compute_call_hash(&EditFileArguments {
        path: relative.clone(),
        edits: approved.edits.clone(),
    })?;
    if call_hash != approved.call_hash {
        return Err(edit_approval_stale());
    }
    let precondition_hash = code_sessions::compute_precondition_hash(&EditFilePreconditions {
        path: &relative,
        before_hash: &before_hash,
    })?;
    if precondition_hash != approved.precondition_hash {
        return Err(AppError::new(
            "edit_precondition_changed",
            "This file changed since the edit was approved. Request a new preview.",
        ));
    }
    let applied = apply_edits_with_diff(&content, &approved.edits)?;
    let preview_hash = code_sessions::compute_preview_hash(&applied.diff);
    if preview_hash != approved.preview_hash {
        return Err(edit_approval_stale());
    }
    let expected_after_hash = code_sessions::sha256_hex(applied.result.as_bytes());

    atomic_write(&path, applied.result.as_bytes())?;

    let observed_bytes = fs::read(&path).map_err(|_| repository_read_error())?;
    let observed_after_hash = code_sessions::sha256_hex(&observed_bytes);
    let outcome =
        classify_recovery_outcome(&observed_after_hash, &before_hash, &expected_after_hash);

    Ok(EditFileOutcome {
        path: relative,
        before_hash,
        expected_after_hash,
        observed_after_hash,
        outcome,
    })
}

/// Read-only verifier used after an execution error. It never retries or repairs the write.
pub fn verify_edit_file_outcome(
    context: &RepositoryContext,
    relative_path: &str,
    before_hash: &str,
    expected_after_hash: &str,
) -> Result<EditFileOutcome, AppError> {
    enforce_tool(context, EDIT_FILE_TOOL_ID)?;
    let (_path, relative, content) = read_editable_file(context, relative_path)?;
    let observed_after_hash = code_sessions::sha256_hex(content.as_bytes());
    Ok(EditFileOutcome {
        path: relative,
        before_hash: before_hash.to_string(),
        expected_after_hash: expected_after_hash.to_string(),
        outcome: classify_recovery_outcome(&observed_after_hash, before_hash, expected_after_hash),
        observed_after_hash,
    })
}

/// The ADR 0003 file-verifier's three reachable outcomes for an attempted atomic write: the
/// rename landed and content matches what was proposed (`Applied`); the rename never landed and
/// the file is exactly as it was before the attempt (`NotApplied`); or the file matches neither
/// (`Diverged`) — which this function never tries to auto-correct, only classifies. `Unknown` is
/// reserved for callers that cannot re-read the file at all (this function always can, since it's
/// only called after a successful re-read).
fn classify_recovery_outcome(
    observed_hash: &str,
    before_hash: &str,
    expected_after_hash: &str,
) -> code_sessions::CodeRecoveryOutcome {
    if observed_hash == expected_after_hash {
        code_sessions::CodeRecoveryOutcome::Applied
    } else if observed_hash == before_hash {
        code_sessions::CodeRecoveryOutcome::NotApplied
    } else {
        code_sessions::CodeRecoveryOutcome::Diverged
    }
}

fn read_editable_file(
    context: &RepositoryContext,
    relative_path: &str,
) -> Result<(std::path::PathBuf, String, String), AppError> {
    let path = crate::repository::resolve_existing_repository_path(context.root(), relative_path)?;
    let metadata = fs::metadata(&path).map_err(|_| repository_read_error())?;
    if !metadata.is_file() {
        return Err(AppError::invalid_input(
            "The requested Repository path is not a file.",
        ));
    }
    if metadata.len() > MAX_EDIT_FILE_BYTES {
        return Err(AppError::new(
            "repository_file_too_large",
            format!(
                "Repository files edited by Ark Code must be at most {MAX_EDIT_FILE_BYTES} bytes."
            ),
        ));
    }
    let bytes = fs::read(&path).map_err(|_| repository_read_error())?;
    if bytes.contains(&0) {
        return Err(AppError::new(
            "repository_binary_file",
            "Ark Code does not edit binary files.",
        ));
    }
    let content = String::from_utf8(bytes).map_err(|_| {
        AppError::new(
            "repository_non_utf8_file",
            "Ark Code can edit UTF-8 text files only.",
        )
    })?;
    let relative = relative_display(context.root(), &path)?;
    Ok((path, relative, content))
}

struct AppliedEdits {
    result: String,
    diff: String,
}

/// Applies every edit block in order against the progressively-updated content — each block's
/// `search` text is checked for uniqueness (and existence) against the state *after* prior blocks
/// in this same call have already been applied, not against the file's original content. This
/// lets a sequence of edits build on each other (a later block may target text a prior block just
/// introduced) while still failing closed on any ambiguous or missing match.
fn apply_edits_with_diff(content: &str, edits: &[EditBlock]) -> Result<AppliedEdits, AppError> {
    if edits.is_empty() {
        return Err(AppError::invalid_input(
            "edit_file requires at least one edit block.",
        ));
    }
    if edits.len() > MAX_EDIT_BLOCKS {
        return Err(AppError::invalid_input(format!(
            "edit_file accepts at most {MAX_EDIT_BLOCKS} edit blocks per call."
        )));
    }
    let mut current = content.to_string();
    let mut hunks = Vec::with_capacity(edits.len());
    for (index, edit) in edits.iter().enumerate() {
        if edit.search.is_empty() {
            return Err(AppError::invalid_input(
                "Each edit block's search text must be non-empty.",
            ));
        }
        if edit.search.chars().count() > MAX_EDIT_BLOCK_CHARS
            || edit.replace.chars().count() > MAX_EDIT_BLOCK_CHARS
        {
            return Err(AppError::invalid_input(format!(
                "Each edit block's search/replace text must be at most {MAX_EDIT_BLOCK_CHARS} characters."
            )));
        }
        let occurrences = current.matches(edit.search.as_str()).count();
        if occurrences == 0 {
            return Err(AppError::new(
                "edit_search_not_found",
                format!(
                    "Edit block {} search text was not found in the file.",
                    index + 1
                ),
            ));
        }
        if occurrences > 1 {
            return Err(AppError::new(
                "edit_search_ambiguous",
                format!(
                    "Edit block {} search text matches {occurrences} places; it must match exactly one.",
                    index + 1
                ),
            ));
        }
        let byte_offset = current
            .find(edit.search.as_str())
            .expect("uniqueness already confirmed above");
        hunks.push(render_hunk(
            &current,
            byte_offset,
            &edit.search,
            &edit.replace,
        ));
        current = format!(
            "{}{}{}",
            &current[..byte_offset],
            edit.replace,
            &current[byte_offset + edit.search.len()..]
        );
    }
    Ok(AppliedEdits {
        result: current,
        diff: hunks.join("\n"),
    })
}

/// Renders one line-oriented diff hunk around a byte-offset match. This is intentionally not a
/// general LCS/Myers diff over the whole file — `edit_file` already knows exactly which bytes
/// change (the search/replace pair), so the hunk only needs to show bounded context around that
/// known location for a human to review, not rediscover the change itself.
fn render_hunk(content: &str, byte_offset: usize, search: &str, replace: &str) -> String {
    let all_lines: Vec<&str> = content.lines().collect();
    let start_line = content[..byte_offset].matches('\n').count();
    let search_line_span = search.lines().count().max(1);
    let end_line = (start_line + search_line_span).min(all_lines.len());
    let context_start = start_line.saturating_sub(DIFF_CONTEXT_LINES);
    let context_end = (end_line + DIFF_CONTEXT_LINES).min(all_lines.len());

    let mut out = String::new();
    for line in &all_lines[context_start..start_line] {
        out.push_str("  ");
        out.push_str(line);
        out.push('\n');
    }
    for line in search.lines() {
        out.push_str("- ");
        out.push_str(line);
        out.push('\n');
    }
    for line in replace.lines() {
        out.push_str("+ ");
        out.push_str(line);
        out.push('\n');
    }
    for line in &all_lines[end_line..context_end] {
        out.push_str("  ");
        out.push_str(line);
        out.push('\n');
    }
    out
}

/// Writes to a fresh, uniquely-named temp file in the *same directory* as `path`, then
/// atomically renames it over `path`. A crash between the write and the rename leaves the
/// original file untouched (the temp file is simply orphaned); a crash after the rename lands the
/// new content in full — there is no window where `path` contains partial content.
fn atomic_write(path: &Path, contents: &[u8]) -> Result<(), AppError> {
    let parent = path.parent().ok_or_else(edit_write_failed)?;
    let temp_path = parent.join(format!(".ark-edit-{}.tmp", Uuid::new_v4()));
    fs::write(&temp_path, contents).map_err(|_| edit_write_failed())?;
    if fs::rename(&temp_path, path).is_err() {
        let _ = fs::remove_file(&temp_path);
        return Err(edit_write_failed());
    }
    Ok(())
}

fn edit_approval_stale() -> AppError {
    AppError::new(
        "edit_approval_stale",
        "This edit no longer matches what was approved. Request a new preview.",
    )
}

fn edit_write_failed() -> AppError {
    AppError::new(
        "edit_write_failed",
        "Ark Code could not write the approved edit to the Repository.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::projects::Project;
    use uuid::Uuid as TestUuid;

    fn fixture(initial_content: &str) -> (Project, std::path::PathBuf) {
        let root = std::env::temp_dir().join(format!("ark-code-edit-{}", TestUuid::new_v4()));
        fs::create_dir_all(&root).expect("repository created");
        fs::write(root.join("lib.rs"), initial_content).expect("fixture file written");
        let timestamp = "2026-08-17T00:00:00Z".to_string();
        (
            Project {
                id: "project-code-edit".to_string(),
                name: "Code Edit".to_string(),
                repository_path: Some(root.to_string_lossy().into_owned()),
                instructions: None,
                default_provider_id: None,
                default_model_id: None,
                default_temperature: None,
                default_max_tokens: None,
                response_style: None,
                tone: None,
                archived_at: None,
                created_at: timestamp.clone(),
                updated_at: timestamp,
            },
            root,
        )
    }

    #[test]
    fn preview_computes_matching_before_and_expected_after_hashes() {
        let (project, root) = fixture("pub fn answer() -> u32 {\n    42\n}\n");
        let context = RepositoryContext::from_project(&project).expect("context created");
        let preview = preview_edit_file(
            &context,
            "lib.rs",
            vec![EditBlock {
                search: "42".to_string(),
                replace: "43".to_string(),
            }],
        )
        .expect("preview built");
        assert_eq!(
            preview.before_hash,
            code_sessions::sha256_hex(b"pub fn answer() -> u32 {\n    42\n}\n")
        );
        assert_eq!(
            preview.expected_after_hash,
            code_sessions::sha256_hex(b"pub fn answer() -> u32 {\n    43\n}\n")
        );
        assert!(preview.diff.contains("- 42"));
        assert!(preview.diff.contains("+ 43"));
        fs::remove_dir_all(root).expect("fixture removed");
    }

    #[test]
    fn zero_or_multiple_matches_are_rejected_before_any_write() {
        let (project, root) = fixture("alpha beta alpha\n");
        let context = RepositoryContext::from_project(&project).expect("context created");

        let missing = preview_edit_file(
            &context,
            "lib.rs",
            vec![EditBlock {
                search: "gamma".to_string(),
                replace: "delta".to_string(),
            }],
        )
        .expect_err("missing search text rejected");
        assert_eq!(missing.code, "edit_search_not_found");

        let ambiguous = preview_edit_file(
            &context,
            "lib.rs",
            vec![EditBlock {
                search: "alpha".to_string(),
                replace: "delta".to_string(),
            }],
        )
        .expect_err("ambiguous search text rejected");
        assert_eq!(ambiguous.code, "edit_search_ambiguous");

        assert_eq!(
            fs::read_to_string(root.join("lib.rs")).unwrap(),
            "alpha beta alpha\n"
        );
        fs::remove_dir_all(root).expect("fixture removed");
    }

    #[test]
    fn execute_applies_the_edit_atomically_and_reports_applied() {
        let (project, root) = fixture("pub fn answer() -> u32 {\n    42\n}\n");
        let context = RepositoryContext::from_project(&project).expect("context created");
        let edits = vec![EditBlock {
            search: "42".to_string(),
            replace: "43".to_string(),
        }];
        let preview = preview_edit_file(&context, "lib.rs", edits.clone()).expect("preview built");

        let outcome = execute_edit_file(
            &context,
            ApprovedEditFile {
                path: preview.path.clone(),
                edits,
                call_hash: preview.call_hash.clone(),
                preview_hash: preview.preview_hash.clone(),
                precondition_hash: preview.precondition_hash.clone(),
            },
        )
        .expect("execution applied");
        assert_eq!(outcome.outcome, code_sessions::CodeRecoveryOutcome::Applied);
        assert_eq!(outcome.observed_after_hash, outcome.expected_after_hash);
        assert_eq!(
            fs::read_to_string(root.join("lib.rs")).unwrap(),
            "pub fn answer() -> u32 {\n    43\n}\n"
        );
        fs::remove_dir_all(root).expect("fixture removed");
    }

    #[test]
    fn execute_rejects_a_precondition_that_no_longer_matches_the_file() {
        let (project, root) = fixture("pub fn answer() -> u32 {\n    42\n}\n");
        let context = RepositoryContext::from_project(&project).expect("context created");
        let edits = vec![EditBlock {
            search: "42".to_string(),
            replace: "43".to_string(),
        }];
        let preview = preview_edit_file(&context, "lib.rs", edits.clone()).expect("preview built");

        // Someone/something else changes the file between preview and approval.
        fs::write(root.join("lib.rs"), "pub fn answer() -> u32 {\n    99\n}\n")
            .expect("concurrent write");

        let error = execute_edit_file(
            &context,
            ApprovedEditFile {
                path: preview.path.clone(),
                edits,
                call_hash: preview.call_hash.clone(),
                preview_hash: preview.preview_hash.clone(),
                precondition_hash: preview.precondition_hash.clone(),
            },
        )
        .expect_err("stale precondition rejected");
        assert_eq!(error.code, "edit_precondition_changed");
        // The concurrent write is untouched — execution refused before writing anything.
        assert_eq!(
            fs::read_to_string(root.join("lib.rs")).unwrap(),
            "pub fn answer() -> u32 {\n    99\n}\n"
        );
        fs::remove_dir_all(root).expect("fixture removed");
    }

    #[test]
    fn execute_rejects_edits_or_hashes_that_do_not_match_the_approved_preview() {
        let (project, root) = fixture("pub fn answer() -> u32 {\n    42\n}\n");
        let context = RepositoryContext::from_project(&project).expect("context created");
        let edits = vec![EditBlock {
            search: "42".to_string(),
            replace: "43".to_string(),
        }];
        let preview = preview_edit_file(&context, "lib.rs", edits.clone()).expect("preview built");

        // The approved args no longer match what was actually previewed (tampered call_hash).
        let tampered_edits = execute_edit_file(
            &context,
            ApprovedEditFile {
                path: preview.path.clone(),
                edits: vec![EditBlock {
                    search: "42".to_string(),
                    replace: "999".to_string(),
                }],
                call_hash: preview.call_hash.clone(),
                preview_hash: preview.preview_hash.clone(),
                precondition_hash: preview.precondition_hash.clone(),
            },
        )
        .expect_err("tampered edits rejected");
        assert_eq!(tampered_edits.code, "edit_approval_stale");

        let tampered_preview_hash = execute_edit_file(
            &context,
            ApprovedEditFile {
                path: preview.path.clone(),
                edits,
                call_hash: preview.call_hash.clone(),
                preview_hash: "0".repeat(64),
                precondition_hash: preview.precondition_hash.clone(),
            },
        )
        .expect_err("tampered preview hash rejected");
        assert_eq!(tampered_preview_hash.code, "edit_approval_stale");

        assert_eq!(
            fs::read_to_string(root.join("lib.rs")).unwrap(),
            "pub fn answer() -> u32 {\n    42\n}\n"
        );
        fs::remove_dir_all(root).expect("fixture removed");
    }

    #[test]
    fn path_traversal_is_rejected_the_same_way_read_only_tools_reject_it() {
        let (project, root) = fixture("content\n");
        let context = RepositoryContext::from_project(&project).expect("context created");
        let error = preview_edit_file(
            &context,
            "../outside.rs",
            vec![EditBlock {
                search: "content".to_string(),
                replace: "changed".to_string(),
            }],
        )
        .expect_err("traversal rejected");
        assert_eq!(error.code, "invalid_repository_path");
        fs::remove_dir_all(root).expect("fixture removed");
    }

    /// The ADR's three reachable recovery outcomes, tested directly against the pure classifier:
    /// a real process-crash mid-rename cannot be injected from a unit test, but the actual
    /// decision this classifier makes for each of the three possible post-rename states is fully
    /// exercised here, deterministically and without relying on OS-level fault injection.
    #[test]
    fn recovery_outcome_classifies_all_three_reachable_post_write_states() {
        let before = code_sessions::sha256_hex(b"before");
        let after = code_sessions::sha256_hex(b"after");
        let corrupted = code_sessions::sha256_hex(b"corrupted");

        assert_eq!(
            classify_recovery_outcome(&after, &before, &after),
            code_sessions::CodeRecoveryOutcome::Applied,
            "rename landed and content matches what was proposed"
        );
        assert_eq!(
            classify_recovery_outcome(&before, &before, &after),
            code_sessions::CodeRecoveryOutcome::NotApplied,
            "rename never landed; a crash before rename leaves the original content intact"
        );
        assert_eq!(
            classify_recovery_outcome(&corrupted, &before, &after),
            code_sessions::CodeRecoveryOutcome::Diverged,
            "content matches neither before nor expected-after; never auto-corrected"
        );
    }

    #[test]
    fn too_many_or_oversized_edit_blocks_are_rejected() {
        let (project, root) = fixture("content\n");
        let context = RepositoryContext::from_project(&project).expect("context created");

        let too_many: Vec<EditBlock> = (0..MAX_EDIT_BLOCKS + 1)
            .map(|index| EditBlock {
                search: format!("needle-{index}"),
                replace: "x".to_string(),
            })
            .collect();
        let error =
            preview_edit_file(&context, "lib.rs", too_many).expect_err("block count rejected");
        assert_eq!(error.code, "invalid_input");

        let oversized = vec![EditBlock {
            search: "content".to_string(),
            replace: "x".repeat(MAX_EDIT_BLOCK_CHARS + 1),
        }];
        let error =
            preview_edit_file(&context, "lib.rs", oversized).expect_err("oversized block rejected");
        assert_eq!(error.code, "invalid_input");

        let empty_search = vec![EditBlock {
            search: String::new(),
            replace: "x".to_string(),
        }];
        let error =
            preview_edit_file(&context, "lib.rs", empty_search).expect_err("empty search rejected");
        assert_eq!(error.code, "invalid_input");

        fs::remove_dir_all(root).expect("fixture removed");
    }
}
