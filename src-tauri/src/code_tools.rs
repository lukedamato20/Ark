//! CODE-004: Ark Code's read-only Repository tool set, plus the registry entry for CODE-005's
//! write-capable tools (implemented in `code_write_tools`, which reuses `RepositoryContext`,
//! `enforce_tool`, and `relative_display` from this module rather than duplicating them).
//!
//! These tools reuse SEC-009's authoritative `ToolDefinition`/`CapabilityScope` model, but live
//! in a separate registry from Ark Chat's tools. Every operation requires a `RepositoryContext`
//! constructed from an existing Project binding, applies `.gitignore`-aware traversal, and
//! returns bounded, explicit results. No function in *this* module writes Repository state.

use crate::errors::AppError;
use crate::projects::Project;
use crate::providers::{ProviderToolCall, ProviderToolDefinition, ProviderToolResult};
use crate::tool_policy::{CapabilityScope, CapabilityTier};
use crate::tools::ToolDefinition;
use ignore::WalkBuilder;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::ffi::OsStr;
use std::fs;
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;
use tokio::io::AsyncReadExt;

pub const LIST_DIRECTORY_TOOL_ID: &str = "list_directory";
pub const READ_FILE_TOOL_ID: &str = "read_file";
pub const SEARCH_TOOL_ID: &str = "search";
pub const GIT_STATUS_TOOL_ID: &str = "git_status";
pub const GIT_DIFF_TOOL_ID: &str = "git_diff";
pub const REPOSITORY_MAP_TOOL_ID: &str = "repository_map";
pub const REQUEST_CLARIFICATION_TOOL_ID: &str = "request_clarification";

const DEFAULT_LIST_ENTRIES: usize = 200;
const MAX_LIST_ENTRIES: usize = 500;
const DEFAULT_READ_LINES: usize = 200;
const MAX_READ_LINES: usize = 400;
const MAX_READ_OUTPUT_BYTES: usize = 128 * 1024;
const MAX_CONTEXT_FILE_BYTES: u64 = 1024 * 1024;
const TEXT_SAMPLE_BYTES: u64 = 8 * 1024;
const DEFAULT_SEARCH_RESULTS: usize = 100;
const MAX_SEARCH_RESULTS: usize = 500;
const MAX_SEARCH_QUERY_CHARS: usize = 256;
const MAX_SEARCH_FILES: usize = 10_000;
const MAX_SEARCH_TOTAL_BYTES: u64 = 32 * 1024 * 1024;
const MAX_SEARCH_LINE_CHARS: usize = 500;
const DEFAULT_MAP_ENTRIES: usize = 1_000;
const MAX_MAP_ENTRIES: usize = 2_000;
const MAX_GIT_OUTPUT_BYTES: usize = 512 * 1024;
const MAX_GIT_ERROR_BYTES: usize = 32 * 1024;
const GIT_TIMEOUT: Duration = Duration::from_secs(10);

/// An active Project binding. Its root stays private so untrusted provider/model arguments can
/// never replace the trusted Project-derived authority after context construction.
#[derive(Debug, Clone)]
pub struct RepositoryContext {
    root: PathBuf,
}

impl RepositoryContext {
    pub fn from_project(project: &Project) -> Result<Self, AppError> {
        let raw_root = project.repository_path.as_deref().ok_or_else(|| {
            AppError::new(
                "repository_binding_required",
                "Bind a Repository to this Project before using Ark Code.",
            )
        })?;
        let root = PathBuf::from(raw_root);
        // Revalidates availability and rejects a root replaced by a symlink since binding.
        let root = crate::repository::resolve_existing_repository_path(&root, ".")?;
        Ok(Self { root })
    }

    /// Reconstructs authority from a run's immutable, database-persisted Repository snapshot.
    /// This is deliberately not public API: only Ark's durable run protocol may supply this
    /// path, never a provider tool argument.
    pub(crate) fn from_run_snapshot(snapshot: &str) -> Result<Self, AppError> {
        let root = PathBuf::from(snapshot);
        let root = crate::repository::resolve_existing_repository_path(&root, ".")?;
        Ok(Self { root })
    }

    pub(crate) fn root(&self) -> &Path {
        &self.root
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RepositoryEntryKind {
    Directory,
    File,
    Symlink,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryEntry {
    pub path: String,
    pub kind: RepositoryEntryKind,
    pub byte_size: Option<u64>,
    /// Whether Ark may automatically place this file in model context. Directories are `false`.
    pub context_eligible: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryDirectoryListing {
    pub path: String,
    pub entries: Vec<RepositoryEntry>,
    pub truncated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryFileRead {
    pub path: String,
    pub start_line: usize,
    pub end_line: usize,
    pub total_lines: usize,
    pub content: String,
    pub sha256: String,
    pub truncated: bool,
    pub next_start_line: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositorySearchMatch {
    pub path: String,
    pub line_number: usize,
    pub line: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositorySearchResult {
    pub matches: Vec<RepositorySearchMatch>,
    pub files_scanned: usize,
    pub bytes_scanned: u64,
    pub skipped_files: usize,
    pub truncated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryMap {
    pub entries: Vec<RepositoryEntry>,
    pub inspected_files: usize,
    pub skipped_files: usize,
    pub truncated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryGitStatus {
    pub clean: bool,
    pub porcelain: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryGitDiff {
    pub working_tree: String,
    pub staged: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeRepositorySupport {
    pub repository_map: RepositoryMap,
    pub git_status: RepositoryGitStatus,
    pub git_diff: RepositoryGitDiff,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ListDirectoryArguments {
    path: String,
    max_entries: Option<usize>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReadFileArguments {
    path: String,
    start_line: Option<usize>,
    max_lines: Option<usize>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SearchArguments {
    query: String,
    path: Option<String>,
    case_sensitive: Option<bool>,
    max_results: Option<usize>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RepositoryMapArguments {
    max_entries: Option<usize>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct EmptyArguments {}

pub fn ark_code_tools() -> Vec<ToolDefinition> {
    let scope = || CapabilityScope {
        tier: CapabilityTier::RepositoryExecution,
        read: true,
        write: false,
        network: false,
        secret: false,
        data: "Files and Git metadata inside the active Project's bound Repository".to_string(),
    };
    [
        (
            LIST_DIRECTORY_TOOL_ID,
            "List directory",
            "List one Repository directory without traversing ignored paths.",
        ),
        (
            READ_FILE_TOOL_ID,
            "Read file",
            "Read a bounded line range from one text file in the Repository.",
        ),
        (
            SEARCH_TOOL_ID,
            "Search",
            "Search bounded, non-ignored Repository text files for a literal string.",
        ),
        (
            GIT_STATUS_TOOL_ID,
            "Git status",
            "Inspect bounded Git working-tree status without running hooks or external diff tools.",
        ),
        (
            GIT_DIFF_TOOL_ID,
            "Git diff",
            "Inspect bounded staged and unstaged Git diffs without external diff tools.",
        ),
        (
            REPOSITORY_MAP_TOOL_ID,
            "Repository map",
            "Build a bounded map of non-ignored directories and context-eligible text files.",
        ),
    ]
    .into_iter()
    .map(|(id, name, description)| ToolDefinition {
        id: id.to_string(),
        name: name.to_string(),
        description: description.to_string(),
        publisher: "Ark (built-in)".to_string(),
        scope: scope(),
    })
    .chain(std::iter::once(ToolDefinition {
        id: crate::code_write_tools::EDIT_FILE_TOOL_ID.to_string(),
        name: "Edit file".to_string(),
        description:
            "Propose and, once approved, apply a search/replace edit to one Repository file."
                .to_string(),
        publisher: "Ark (built-in)".to_string(),
        scope: CapabilityScope {
            tier: CapabilityTier::RepositoryExecution,
            read: true,
            write: true,
            network: false,
            secret: false,
            data: "Files inside the active Project's bound Repository".to_string(),
        },
    }))
    .chain(std::iter::once(ToolDefinition {
        id: REQUEST_CLARIFICATION_TOOL_ID.to_string(),
        name: "Request clarification".to_string(),
        description: "Pause safely and ask the user one concise question in the coding conversation."
            .to_string(),
        publisher: "Ark (built-in)".to_string(),
        scope: CapabilityScope {
            tier: CapabilityTier::RepositoryExecution,
            read: false,
            write: false,
            network: false,
            secret: false,
            data: "Ark Code conversation control only".to_string(),
        },
    }))
    .chain(std::iter::once(ToolDefinition {
        id: crate::code_command_tools::RUN_COMMAND_TOOL_ID.to_string(),
        name: "Run verification command".to_string(),
        description: "Propose one exact, user-configured test/build/lint command and run it only after per-use approval."
            .to_string(),
        publisher: "Ark (built-in)".to_string(),
        scope: CapabilityScope {
            tier: CapabilityTier::RepositoryExecution,
            read: true,
            write: true,
            network: false,
            secret: false,
            data: "One fixed command definition inside Ark Code's isolated Repository"
                .to_string(),
        },
    }))
    .chain(std::iter::once(ToolDefinition {
        id: crate::code_git_tools::ROLLBACK_TOOL_ID.to_string(),
        name: "Git rollback".to_string(),
        description: "Propose and, once approved, restore Ark Code's isolated branch to one of its recorded checkpoints."
            .to_string(),
        publisher: "Ark (built-in)".to_string(),
        scope: CapabilityScope {
            tier: CapabilityTier::RepositoryExecution,
            read: true,
            write: true,
            network: false,
            secret: false,
            data: "Files and commits produced inside Ark Code's isolated session Repository"
                .to_string(),
        },
    }))
    .chain(std::iter::once(ToolDefinition {
        id: crate::code_git_tools::CHECKPOINT_TOOL_ID.to_string(),
        name: "Git checkpoint".to_string(),
        description: "Propose and, once approved, commit all reviewed changes on Ark Code's isolated session branch."
            .to_string(),
        publisher: "Ark (built-in)".to_string(),
        scope: CapabilityScope {
            tier: CapabilityTier::RepositoryExecution,
            read: true,
            write: true,
            network: false,
            secret: false,
            data: "Git state inside Ark Code's isolated session Repository".to_string(),
        },
    }))
    .collect()
}

/// Model-facing schemas stay separate from permission definitions, as established by CODE-001.
///
/// Write-capable schemas may appear here only when the agent loop treats them as proposals. The
/// `edit_file` executor is intentionally not part of `execute_provider_call`; only the separate
/// approval command may dispatch it after validating the persisted preview hashes.
pub fn provider_tool_definitions() -> Vec<ProviderToolDefinition> {
    use serde_json::json;
    vec![
        ProviderToolDefinition {
            name: LIST_DIRECTORY_TOOL_ID.to_string(),
            description: "List direct children of a Repository directory. Paths are relative to the Repository root."
                .to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string", "description": "Relative directory path, or . for the root"},
                    "max_entries": {"type": "integer", "minimum": 1, "maximum": MAX_LIST_ENTRIES}
                },
                "required": ["path"],
                "additionalProperties": false
            }),
        },
        ProviderToolDefinition {
            name: crate::code_write_tools::EDIT_FILE_TOOL_ID.to_string(),
            description: "Propose search/replace edits to one Repository text file. Ark will show the diff to the user and will not write until they explicitly approve it."
                .to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string"},
                    "edits": {
                        "type": "array",
                        "minItems": 1,
                        "maxItems": 20,
                        "items": {
                            "type": "object",
                            "properties": {
                                "search": {"type": "string", "minLength": 1, "maxLength": 20000},
                                "replace": {"type": "string", "maxLength": 20000}
                            },
                            "required": ["search", "replace"],
                            "additionalProperties": false
                        }
                    }
                },
                "required": ["path", "edits"],
                "additionalProperties": false
            }),
        },
        ProviderToolDefinition {
            name: crate::code_git_tools::CHECKPOINT_TOOL_ID.to_string(),
            description: "Propose a Git checkpoint containing all current reviewed Repository changes. Ark will show the exact status and diff and will commit only after explicit approval."
                .to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "message": {"type": "string", "minLength": 1, "maxLength": 200}
                },
                "required": ["message"],
                "additionalProperties": false
            }),
        },
        ProviderToolDefinition {
            name: crate::code_git_tools::ROLLBACK_TOOL_ID.to_string(),
            description: "Propose restoring Ark Code's isolated branch to a previously reported checkpoint ID. Ark verifies ownership and shows every removed change before approval."
                .to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "checkpoint_id": {"type": "string", "minLength": 1, "maxLength": 128}
                },
                "required": ["checkpoint_id"],
                "additionalProperties": false
            }),
        },
        ProviderToolDefinition {
            name: crate::code_command_tools::RUN_COMMAND_TOOL_ID.to_string(),
            description: "Propose an enabled user-configured verification command by its ID. Only exact fixed templates listed in prior context are valid, and Ark requires per-use approval."
                .to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "command_id": {"type": "string", "minLength": 1, "maxLength": 128}
                },
                "required": ["command_id"],
                "additionalProperties": false
            }),
        },
        ProviderToolDefinition {
            name: REQUEST_CLARIFICATION_TOOL_ID.to_string(),
            description: "Pause the run and ask the user one concise clarification question when proceeding would require a material guess."
                .to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "question": {"type": "string", "minLength": 1, "maxLength": 1000}
                },
                "required": ["question"],
                "additionalProperties": false
            }),
        },
        ProviderToolDefinition {
            name: READ_FILE_TOOL_ID.to_string(),
            description: "Read a bounded line range from a Repository text file.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string"},
                    "start_line": {"type": "integer", "minimum": 1},
                    "max_lines": {"type": "integer", "minimum": 1, "maximum": MAX_READ_LINES}
                },
                "required": ["path"],
                "additionalProperties": false
            }),
        },
        ProviderToolDefinition {
            name: SEARCH_TOOL_ID.to_string(),
            description: format!(
                "Search non-ignored Repository text files for a literal string. max_results is optional and must be between 1 and {MAX_SEARCH_RESULTS}; omit it to use {DEFAULT_SEARCH_RESULTS}."
            ),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "query": {"type": "string", "maxLength": MAX_SEARCH_QUERY_CHARS},
                    "path": {"type": "string", "description": "Relative directory path; defaults to ."},
                    "case_sensitive": {"type": "boolean"},
                    "max_results": {"type": "integer", "minimum": 1, "maximum": MAX_SEARCH_RESULTS}
                },
                "required": ["query"],
                "additionalProperties": false
            }),
        },
        empty_schema_tool(GIT_STATUS_TOOL_ID, "Inspect Git working-tree status."),
        empty_schema_tool(GIT_DIFF_TOOL_ID, "Inspect staged and unstaged Git diffs."),
        ProviderToolDefinition {
            name: REPOSITORY_MAP_TOOL_ID.to_string(),
            description: format!(
                "Build a bounded map of context-eligible Repository files. Call with an empty object to use the default; the only accepted field is optional max_entries between 1 and {MAX_MAP_ENTRIES}. The map contains paths and metadata, not file contents."
            ),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "max_entries": {"type": "integer", "minimum": 1, "maximum": MAX_MAP_ENTRIES}
                },
                "additionalProperties": false
            }),
        },
    ]
}

/// Executes one already-selected CODE-001 provider call through the same functions the typed IPC
/// boundary exposes. The future durable agent loop owns when a call may execute; this dispatcher
/// owns strict argument decoding, tool identity, and bounded JSON observation construction.
pub async fn execute_provider_call(
    context: &RepositoryContext,
    call: &ProviderToolCall,
) -> Result<ProviderToolResult, AppError> {
    let content = match call.name.as_str() {
        LIST_DIRECTORY_TOOL_ID => {
            let args: ListDirectoryArguments = decode_arguments(call)?;
            serde_json::to_string(&list_directory(context, &args.path, args.max_entries)?)
        }
        READ_FILE_TOOL_ID => {
            let args: ReadFileArguments = decode_arguments(call)?;
            serde_json::to_string(&read_file(
                context,
                &args.path,
                args.start_line,
                args.max_lines,
            )?)
        }
        SEARCH_TOOL_ID => {
            let args: SearchArguments = decode_arguments(call)?;
            serde_json::to_string(&search(
                context,
                &args.query,
                args.path.as_deref(),
                args.case_sensitive.unwrap_or(false),
                args.max_results,
            )?)
        }
        REPOSITORY_MAP_TOOL_ID => {
            let args: RepositoryMapArguments = decode_arguments(call)?;
            serde_json::to_string(&repository_map(context, args.max_entries)?)
        }
        GIT_STATUS_TOOL_ID => {
            let _: EmptyArguments = decode_arguments(call)?;
            serde_json::to_string(&git_status(context).await?)
        }
        GIT_DIFF_TOOL_ID => {
            let _: EmptyArguments = decode_arguments(call)?;
            serde_json::to_string(&git_diff(context).await?)
        }
        _ => return Err(AppError::not_found("Ark Code tool")),
    }
    .map_err(|_| {
        AppError::new(
            "tool_result_serialization_failed",
            "Ark Code could not serialize the bounded tool observation.",
        )
    })?;
    Ok(ProviderToolResult {
        provider_call_id: call.provider_call_id.clone(),
        name: call.name.clone(),
        content,
    })
}

fn decode_arguments<T: for<'de> Deserialize<'de>>(call: &ProviderToolCall) -> Result<T, AppError> {
    serde_json::from_value(call.arguments.clone()).map_err(|error| {
        AppError::new(
            "invalid_tool_arguments",
            format!(
                "Ark Code tool '{}' received invalid arguments: {error}. Correct the arguments to match the tool schema before retrying.",
                call.name
            ),
        )
    })
}

fn empty_schema_tool(name: &str, description: &str) -> ProviderToolDefinition {
    ProviderToolDefinition {
        name: name.to_string(),
        description: description.to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {},
            "additionalProperties": false
        }),
    }
}

pub(crate) fn enforce_tool(context: &RepositoryContext, tool_id: &str) -> Result<(), AppError> {
    let definition = ark_code_tools()
        .into_iter()
        .find(|tool| tool.id == tool_id)
        .ok_or_else(|| AppError::not_found("Ark Code tool"))?;
    crate::tool_policy::enforce_tier_boundary(
        &definition.scope,
        !context.root.as_os_str().is_empty(),
    )
    .map_err(|message| AppError::new("repository_binding_required", message))
}

pub fn list_directory(
    context: &RepositoryContext,
    relative_path: &str,
    max_entries: Option<usize>,
) -> Result<RepositoryDirectoryListing, AppError> {
    enforce_tool(context, LIST_DIRECTORY_TOOL_ID)?;
    let limit = bounded_limit(max_entries, DEFAULT_LIST_ENTRIES, MAX_LIST_ENTRIES, "entry")?;
    let target =
        crate::repository::resolve_existing_repository_path(context.root(), relative_path)?;
    if !target.is_dir() {
        return Err(AppError::invalid_input(
            "The requested Repository path is not a directory.",
        ));
    }
    ensure_visible(context, &target)?;

    let target_depth = target
        .strip_prefix(context.root())
        .map_err(|_| repository_escape())?
        .components()
        .count();
    let mut entries = Vec::new();
    let mut truncated = false;
    for result in repository_walk(context, &target, Some(target_depth + 1)) {
        let entry = result.map_err(walk_error)?;
        if entry.path() == target || entry.path().parent() != Some(target.as_path()) {
            continue;
        }
        if entries.len() == limit {
            truncated = true;
            break;
        }
        entries.push(repository_entry(context, &entry)?);
    }
    entries.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(RepositoryDirectoryListing {
        path: relative_display(context.root(), &target)?,
        entries,
        truncated,
    })
}

pub fn read_file(
    context: &RepositoryContext,
    relative_path: &str,
    start_line: Option<usize>,
    max_lines: Option<usize>,
) -> Result<RepositoryFileRead, AppError> {
    enforce_tool(context, READ_FILE_TOOL_ID)?;
    let start_line = start_line.unwrap_or(1);
    if start_line == 0 {
        return Err(AppError::invalid_input("Start line must be at least 1."));
    }
    let max_lines = bounded_limit(max_lines, DEFAULT_READ_LINES, MAX_READ_LINES, "line")?;
    let path = crate::repository::resolve_existing_repository_path(context.root(), relative_path)?;
    ensure_visible(context, &path)?;
    let metadata = fs::metadata(&path).map_err(|_| repository_read_error())?;
    if !metadata.is_file() {
        return Err(AppError::invalid_input(
            "The requested Repository path is not a file.",
        ));
    }
    if metadata.len() > MAX_CONTEXT_FILE_BYTES {
        return Err(AppError::new(
            "repository_file_too_large",
            format!(
                "Repository files read into context must be at most {MAX_CONTEXT_FILE_BYTES} bytes."
            ),
        ));
    }
    let bytes = read_file_bounded(&path, MAX_CONTEXT_FILE_BYTES as usize)?;
    if bytes.contains(&0) {
        return Err(AppError::new(
            "repository_binary_file",
            "Ark Code does not place binary files in model context.",
        ));
    }
    let text = String::from_utf8(bytes.clone()).map_err(|_| {
        AppError::new(
            "repository_non_utf8_file",
            "Ark Code can read UTF-8 text files only.",
        )
    })?;
    let lines: Vec<&str> = text.lines().collect();
    if !lines.is_empty() && start_line > lines.len() {
        return Err(AppError::invalid_input(format!(
            "Start line {start_line} is beyond the file's {} lines.",
            lines.len()
        )));
    }
    let start_index = start_line.saturating_sub(1).min(lines.len());
    let requested_end = start_index.saturating_add(max_lines).min(lines.len());
    let mut selected = Vec::new();
    let mut selected_bytes = 0usize;
    for line in &lines[start_index..requested_end] {
        let added = line.len() + usize::from(!selected.is_empty());
        if selected_bytes.saturating_add(added) > MAX_READ_OUTPUT_BYTES {
            if selected.is_empty() {
                return Err(AppError::new(
                    "repository_line_too_large",
                    "One line exceeds Ark Code's bounded file-read output.",
                ));
            }
            break;
        }
        selected.push(*line);
        selected_bytes += added;
    }
    let end_index = start_index + selected.len();
    let truncated = end_index < lines.len();
    Ok(RepositoryFileRead {
        path: relative_display(context.root(), &path)?,
        start_line,
        end_line: if selected.is_empty() { 0 } else { end_index },
        total_lines: lines.len(),
        content: selected.join("\n"),
        sha256: format!("{:x}", Sha256::digest(&bytes)),
        truncated,
        next_start_line: truncated.then_some(end_index + 1),
    })
}

pub fn search(
    context: &RepositoryContext,
    query: &str,
    relative_path: Option<&str>,
    case_sensitive: bool,
    max_results: Option<usize>,
) -> Result<RepositorySearchResult, AppError> {
    enforce_tool(context, SEARCH_TOOL_ID)?;
    let query = query.trim();
    if query.is_empty() || query.chars().count() > MAX_SEARCH_QUERY_CHARS {
        return Err(AppError::invalid_input(format!(
            "Search query must be between 1 and {MAX_SEARCH_QUERY_CHARS} characters."
        )));
    }
    let limit = bounded_limit(
        max_results,
        DEFAULT_SEARCH_RESULTS,
        MAX_SEARCH_RESULTS,
        "result",
    )?;
    let start = crate::repository::resolve_existing_repository_path(
        context.root(),
        relative_path.unwrap_or("."),
    )?;
    if !start.is_dir() {
        return Err(AppError::invalid_input(
            "Search path must be a Repository directory.",
        ));
    }
    ensure_visible(context, &start)?;

    let normalized_query = (!case_sensitive).then(|| query.to_lowercase());
    let mut result = RepositorySearchResult {
        matches: Vec::new(),
        files_scanned: 0,
        bytes_scanned: 0,
        skipped_files: 0,
        truncated: false,
    };
    let mut considered_files = 0usize;
    for walked in repository_walk(context, &start, None) {
        let entry = walked.map_err(walk_error)?;
        let Some(file_type) = entry.file_type() else {
            result.skipped_files += 1;
            continue;
        };
        if !file_type.is_file() {
            continue;
        }
        considered_files += 1;
        if considered_files > MAX_SEARCH_FILES || result.bytes_scanned >= MAX_SEARCH_TOTAL_BYTES {
            result.truncated = true;
            break;
        }
        let metadata = match entry.metadata() {
            Ok(metadata) => metadata,
            Err(_) => {
                result.skipped_files += 1;
                continue;
            }
        };
        if metadata.len() > MAX_CONTEXT_FILE_BYTES {
            result.skipped_files += 1;
            continue;
        }
        if result.bytes_scanned.saturating_add(metadata.len()) > MAX_SEARCH_TOTAL_BYTES {
            result.truncated = true;
            break;
        }
        let relative = relative_display(context.root(), entry.path())?;
        let canonical =
            crate::repository::resolve_existing_repository_path(context.root(), &relative)?;
        let Some(text) = read_candidate_text(&canonical)? else {
            result.skipped_files += 1;
            continue;
        };
        result.files_scanned += 1;
        result.bytes_scanned += metadata.len();
        for (line_index, line) in text.lines().enumerate() {
            let matches = if let Some(normalized_query) = &normalized_query {
                line.to_lowercase().contains(normalized_query)
            } else {
                line.contains(query)
            };
            if !matches {
                continue;
            }
            if result.matches.len() == limit {
                result.truncated = true;
                return Ok(result);
            }
            result.matches.push(RepositorySearchMatch {
                path: relative.clone(),
                line_number: line_index + 1,
                line: bounded_line_preview(line),
            });
        }
    }
    Ok(result)
}

pub fn repository_map(
    context: &RepositoryContext,
    max_entries: Option<usize>,
) -> Result<RepositoryMap, AppError> {
    enforce_tool(context, REPOSITORY_MAP_TOOL_ID)?;
    let limit = bounded_limit(max_entries, DEFAULT_MAP_ENTRIES, MAX_MAP_ENTRIES, "entry")?;
    let mut map = RepositoryMap {
        entries: Vec::new(),
        inspected_files: 0,
        skipped_files: 0,
        truncated: false,
    };
    let mut considered_files = 0usize;
    for walked in repository_walk(context, context.root(), None) {
        let entry = walked.map_err(walk_error)?;
        if entry.path() == context.root() {
            continue;
        }
        if map.entries.len() == limit {
            map.truncated = true;
            break;
        }
        let repository_entry = repository_entry(context, &entry)?;
        match repository_entry.kind {
            RepositoryEntryKind::Directory => map.entries.push(repository_entry),
            RepositoryEntryKind::File if repository_entry.context_eligible => {
                considered_files += 1;
                map.inspected_files += 1;
                map.entries.push(repository_entry);
            }
            RepositoryEntryKind::File => {
                considered_files += 1;
                map.inspected_files += 1;
                map.skipped_files += 1;
            }
            RepositoryEntryKind::Symlink => {
                considered_files += 1;
                map.skipped_files += 1;
            }
        }
        if considered_files >= MAX_SEARCH_FILES {
            map.truncated = true;
            break;
        }
    }
    map.entries
        .sort_by(|left, right| left.path.cmp(&right.path));
    Ok(map)
}

pub async fn git_status(context: &RepositoryContext) -> Result<RepositoryGitStatus, AppError> {
    enforce_tool(context, GIT_STATUS_TOOL_ID)?;
    let porcelain = run_git(
        context,
        &["status", "--porcelain=v1", "--untracked-files=all"],
    )
    .await?;
    Ok(RepositoryGitStatus {
        clean: porcelain.is_empty(),
        porcelain,
    })
}

pub async fn git_diff(context: &RepositoryContext) -> Result<RepositoryGitDiff, AppError> {
    enforce_tool(context, GIT_DIFF_TOOL_ID)?;
    let working_tree = run_git(
        context,
        &[
            "diff",
            "--no-ext-diff",
            "--no-textconv",
            "--unified=3",
            "--",
        ],
    )
    .await?;
    let staged = run_git(
        context,
        &[
            "diff",
            "--cached",
            "--no-ext-diff",
            "--no-textconv",
            "--unified=3",
            "--",
        ],
    )
    .await?;
    Ok(RepositoryGitDiff {
        working_tree,
        staged,
    })
}

fn repository_walk(
    context: &RepositoryContext,
    selected: &Path,
    max_depth: Option<usize>,
) -> ignore::Walk {
    let root = context.root().to_path_buf();
    let selected = selected.to_path_buf();
    let mut builder = WalkBuilder::new(&root);
    builder
        .standard_filters(true)
        .hidden(false)
        .parents(false)
        .require_git(false)
        .follow_links(false)
        .git_global(false)
        .max_depth(max_depth)
        .filter_entry(move |entry| {
            let path = entry.path();
            !contains_git_metadata(&root, path)
                && (path == root || selected.starts_with(path) || path.starts_with(&selected))
        });
    builder.build()
}

fn contains_git_metadata(root: &Path, path: &Path) -> bool {
    path.strip_prefix(root)
        .ok()
        .into_iter()
        .flat_map(Path::components)
        .any(|component| matches!(component, Component::Normal(name) if name == OsStr::new(".git")))
}

fn ensure_visible(context: &RepositoryContext, target: &Path) -> Result<(), AppError> {
    let depth = target
        .strip_prefix(context.root())
        .map_err(|_| repository_escape())?
        .components()
        .count();
    for walked in repository_walk(context, target, Some(depth)) {
        let entry = walked.map_err(walk_error)?;
        if entry.path() == target {
            return Ok(());
        }
    }
    Err(AppError::new(
        "repository_path_ignored",
        "The requested path is excluded by Repository ignore rules or Git metadata boundaries.",
    ))
}

fn repository_entry(
    context: &RepositoryContext,
    entry: &ignore::DirEntry,
) -> Result<RepositoryEntry, AppError> {
    let file_type = entry.file_type().ok_or_else(|| {
        AppError::new(
            "repository_inspection_failed",
            "A Repository entry could not be inspected safely.",
        )
    })?;
    let path = relative_display(context.root(), entry.path())?;
    if file_type.is_symlink() {
        return Ok(RepositoryEntry {
            path,
            kind: RepositoryEntryKind::Symlink,
            byte_size: None,
            context_eligible: false,
        });
    }
    if file_type.is_dir() {
        return Ok(RepositoryEntry {
            path,
            kind: RepositoryEntryKind::Directory,
            byte_size: None,
            context_eligible: false,
        });
    }
    let canonical = crate::repository::resolve_existing_repository_path(context.root(), &path)?;
    let metadata = fs::metadata(&canonical).map_err(|_| repository_read_error())?;
    let context_eligible =
        metadata.len() <= MAX_CONTEXT_FILE_BYTES && sample_is_text(&canonical).unwrap_or(false);
    Ok(RepositoryEntry {
        path,
        kind: RepositoryEntryKind::File,
        byte_size: Some(metadata.len()),
        context_eligible,
    })
}

fn sample_is_text(path: &Path) -> std::io::Result<bool> {
    let file = fs::File::open(path)?;
    let mut sample = Vec::new();
    file.take(TEXT_SAMPLE_BYTES).read_to_end(&mut sample)?;
    Ok(!sample.contains(&0) && std::str::from_utf8(&sample).is_ok())
}

fn read_candidate_text(path: &Path) -> Result<Option<String>, AppError> {
    let bytes = match read_file_bounded(path, MAX_CONTEXT_FILE_BYTES as usize) {
        Ok(bytes) => bytes,
        Err(error) if error.code == "repository_read_failed" => return Ok(None),
        Err(error) => return Err(error),
    };
    if bytes.contains(&0) {
        return Ok(None);
    }
    Ok(String::from_utf8(bytes).ok())
}

fn read_file_bounded(path: &Path, maximum: usize) -> Result<Vec<u8>, AppError> {
    let file = fs::File::open(path).map_err(|_| repository_read_error())?;
    let mut bytes = Vec::new();
    file.take((maximum + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| repository_read_error())?;
    if bytes.len() > maximum {
        return Err(AppError::new(
            "repository_file_too_large",
            "Repository file exceeded Ark Code's bounded read limit.",
        ));
    }
    Ok(bytes)
}

pub(crate) fn relative_display(root: &Path, path: &Path) -> Result<String, AppError> {
    let relative = path.strip_prefix(root).map_err(|_| repository_escape())?;
    if relative.as_os_str().is_empty() {
        return Ok(".".to_string());
    }
    let relative = relative.to_str().ok_or_else(|| {
        AppError::new(
            "repository_non_unicode_path",
            "Ark Code cannot represent a Repository path that is not valid Unicode.",
        )
    })?;
    Ok(relative.replace('\\', "/"))
}

fn bounded_limit(
    value: Option<usize>,
    default: usize,
    maximum: usize,
    label: &str,
) -> Result<usize, AppError> {
    let value = value.unwrap_or(default);
    if value == 0 || value > maximum {
        return Err(AppError::invalid_input(format!(
            "Requested {label} limit must be between 1 and {maximum}."
        )));
    }
    Ok(value)
}

fn bounded_line_preview(line: &str) -> String {
    let mut preview: String = line
        .chars()
        .take(MAX_SEARCH_LINE_CHARS)
        .map(|character| {
            if character.is_control() && character != '\t' {
                '�'
            } else {
                character
            }
        })
        .collect();
    if line.chars().count() > MAX_SEARCH_LINE_CHARS {
        preview.push('…');
    }
    preview
}

async fn run_git(context: &RepositoryContext, args: &[&str]) -> Result<String, AppError> {
    // `git status` and `git diff` do not require a filesystem path argument from the model. The
    // cwd is the already-validated binding, external diff/textconv/fsmonitor execution is
    // disabled, optional locks are disabled, and output/time are bounded.
    let null_device = if cfg!(windows) { "NUL" } else { "/dev/null" };
    let git_directory = validate_git_directory(context)?;
    let mut command = tokio::process::Command::new("git");
    crate::process_window::hide_tokio_process_window(&mut command);
    command
        .arg("--no-pager")
        .arg("--literal-pathspecs")
        .arg("-c")
        .arg("core.fsmonitor=false")
        .arg("-c")
        .arg(format!("core.hooksPath={null_device}"))
        .arg("-c")
        .arg("diff.external=")
        .arg("-c")
        .arg("core.quotePath=true")
        .arg("-C")
        .arg(context.root())
        .args(args)
        .env_clear()
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", null_device)
        .env("GIT_DIR", git_directory)
        .env("GIT_WORK_TREE", context.root())
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("GIT_PAGER", "cat")
        .env("LC_ALL", "C")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    for name in ["PATH", "SystemRoot", "WINDIR", "TEMP", "TMP"] {
        if let Some(value) = std::env::var_os(name) {
            command.env(name, value);
        }
    }
    let mut child = command.spawn().map_err(|_| {
        AppError::new(
            "git_unavailable",
            "Git is not installed or could not be started safely.",
        )
    })?;
    let stdout = child.stdout.take().ok_or_else(git_error)?;
    let stderr = child.stderr.take().ok_or_else(git_error)?;
    let stdout_task = tokio::spawn(read_process_pipe(stdout, MAX_GIT_OUTPUT_BYTES));
    let stderr_task = tokio::spawn(read_process_pipe(stderr, MAX_GIT_ERROR_BYTES));
    let status = match tokio::time::timeout(GIT_TIMEOUT, child.wait()).await {
        Ok(status) => status.map_err(|_| git_error())?,
        Err(_) => {
            let _ = child.kill().await;
            let _ = child.wait().await;
            return Err(AppError::new(
                "git_timeout",
                "Git inspection exceeded Ark Code's 10-second limit.",
            ));
        }
    };
    let (stdout, stdout_overflow) = stdout_task.await.map_err(|_| git_error())??;
    let (_stderr, stderr_overflow) = stderr_task.await.map_err(|_| git_error())??;
    if stdout_overflow || stderr_overflow {
        return Err(AppError::new(
            "git_output_too_large",
            "Git inspection output exceeded Ark Code's bounded result limit.",
        ));
    }
    if !status.success() {
        return Err(git_error());
    }
    String::from_utf8(stdout).map_err(|_| {
        AppError::new(
            "git_non_utf8_output",
            "Git returned output Ark Code could not represent safely.",
        )
    })
}

fn validate_git_directory(context: &RepositoryContext) -> Result<PathBuf, AppError> {
    let git_directory = context.root().join(".git");
    let metadata = fs::symlink_metadata(&git_directory).map_err(|_| {
        AppError::new(
            "git_repository_required",
            "The bound Repository is not an initialized Git repository.",
        )
    })?;
    // Linked worktrees commonly store `.git` as a pointer to metadata outside the worktree.
    // Supporting that would violate CODE-004's strict Repository-root boundary, so V1 fails
    // closed rather than allowing Git to discover or follow external metadata.
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(AppError::new(
            "git_metadata_outside_repository",
            "Git metadata must be a real directory inside the bound Repository.",
        ));
    }
    let canonical = crate::validation::canonicalize_for_use(&git_directory, "Git metadata")?;
    if !canonical.starts_with(context.root()) {
        return Err(AppError::new(
            "git_metadata_outside_repository",
            "Git metadata resolves outside the bound Repository.",
        ));
    }
    Ok(canonical)
}

async fn read_process_pipe<R: tokio::io::AsyncRead + Unpin>(
    reader: R,
    maximum: usize,
) -> Result<(Vec<u8>, bool), AppError> {
    let mut bytes = Vec::new();
    reader
        .take((maximum + 1) as u64)
        .read_to_end(&mut bytes)
        .await
        .map_err(|_| git_error())?;
    let overflow = bytes.len() > maximum;
    bytes.truncate(maximum);
    Ok((bytes, overflow))
}

fn walk_error(_: ignore::Error) -> AppError {
    AppError::new(
        "repository_walk_failed",
        "Ark Code could not enumerate the Repository safely.",
    )
}

pub(crate) fn repository_read_error() -> AppError {
    AppError::new(
        "repository_read_failed",
        "Ark Code could not read the requested Repository content safely.",
    )
}

fn repository_escape() -> AppError {
    AppError::new(
        "repository_path_escape",
        "The requested path resolves outside the bound Repository.",
    )
}

fn git_error() -> AppError {
    AppError::new(
        "git_inspection_failed",
        "Git could not inspect the bound Repository. Verify that it is a valid Git worktree.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn fixture() -> (Project, PathBuf) {
        let root = std::env::temp_dir().join(format!("ark-code-tools-{}", Uuid::new_v4()));
        fs::create_dir_all(root.join("src")).expect("source directory created");
        fs::write(root.join(".gitignore"), "*.log\nignored/\n").expect("ignore file written");
        fs::write(
            root.join("src/lib.rs"),
            "pub fn answer() -> u32 {\n    42\n}\n",
        )
        .expect("source written");
        fs::write(root.join("debug.log"), "needle in ignored file\n")
            .expect("ignored file written");
        fs::write(root.join("binary.bin"), [0, 1, 2, 3]).expect("binary written");
        fs::create_dir_all(root.join("ignored")).expect("ignored directory created");
        fs::write(root.join("ignored/secret.txt"), "needle\n").expect("ignored text written");
        let timestamp = "2026-08-17T00:00:00Z".to_string();
        (
            Project {
                id: "project-code".to_string(),
                name: "Code".to_string(),
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
    fn registry_is_repository_scoped_and_side_effects_are_proposals() {
        let tools = ark_code_tools();
        assert_eq!(tools.len(), 11);
        assert!(tools
            .iter()
            .all(|tool| tool.scope.tier == CapabilityTier::RepositoryExecution));
        let read_only: Vec<_> = tools
            .iter()
            .filter(|tool| tool.scope.read && !tool.scope.write)
            .collect();
        assert_eq!(read_only.len(), 6);
        assert!(read_only.iter().all(|tool| tool.scope.read
            && !tool.scope.write
            && !tool.scope.network
            && !tool.scope.secret));
        let edit_file = tools
            .iter()
            .find(|tool| tool.id == crate::code_write_tools::EDIT_FILE_TOOL_ID)
            .expect("edit_file is registered");
        assert!(edit_file.scope.write);
        for tool_id in [
            crate::code_git_tools::CHECKPOINT_TOOL_ID,
            crate::code_git_tools::ROLLBACK_TOOL_ID,
            crate::code_command_tools::RUN_COMMAND_TOOL_ID,
        ] {
            assert!(tools
                .iter()
                .find(|tool| tool.id == tool_id)
                .is_some_and(|tool| tool.scope.write));
        }

        let provider_tools = provider_tool_definitions();
        assert_eq!(provider_tools.len(), tools.len());
        assert!(provider_tools
            .iter()
            .any(|tool| tool.name == crate::code_write_tools::EDIT_FILE_TOOL_ID));
        assert!(provider_tools
            .iter()
            .all(|tool| tool.input_schema["additionalProperties"] == false));
    }

    #[test]
    fn repository_context_requires_a_real_project_binding() {
        let (mut project, root) = fixture();
        project.repository_path = None;
        let error = RepositoryContext::from_project(&project).expect_err("binding required");
        assert_eq!(error.code, "repository_binding_required");
        fs::remove_dir_all(root).expect("fixture removed");
    }

    #[test]
    fn listing_mapping_reading_and_search_are_bounded_and_ignore_aware() {
        let (project, root) = fixture();
        let context = RepositoryContext::from_project(&project).expect("context created");

        let listing = list_directory(&context, ".", None).expect("root listed");
        assert!(listing.entries.iter().any(|entry| entry.path == "src"));
        assert!(!listing
            .entries
            .iter()
            .any(|entry| entry.path == "debug.log"));
        assert!(!listing.entries.iter().any(|entry| entry.path == "ignored"));
        let binary = listing
            .entries
            .iter()
            .find(|entry| entry.path == "binary.bin")
            .expect("binary is visible but classified");
        assert!(!binary.context_eligible);

        let read = read_file(&context, "src/lib.rs", Some(2), Some(1)).expect("source read");
        assert_eq!(read.content.trim(), "42");
        assert_eq!(read.start_line, 2);
        assert!(read.truncated);
        assert_eq!(read.next_start_line, Some(3));
        assert_eq!(read.sha256.len(), 64);

        let search = search(&context, "42", None, true, None).expect("repository searched");
        assert_eq!(search.matches.len(), 1);
        assert_eq!(search.matches[0].path, "src/lib.rs");

        let ignored =
            read_file(&context, "debug.log", None, None).expect_err("ignored read denied");
        assert_eq!(ignored.code, "repository_path_ignored");
        let traversal =
            read_file(&context, "../outside", None, None).expect_err("traversal denied");
        assert_eq!(traversal.code, "invalid_repository_path");

        let map = repository_map(&context, None).expect("map built");
        assert!(map.entries.iter().any(|entry| entry.path == "src/lib.rs"));
        assert!(!map.entries.iter().any(|entry| entry.path == "binary.bin"));
        assert!(!map.entries.iter().any(|entry| entry.path == "debug.log"));

        fs::remove_dir_all(root).expect("fixture removed");
    }

    #[test]
    fn explicit_limits_fail_instead_of_silently_expanding_work() {
        let (project, root) = fixture();
        let context = RepositoryContext::from_project(&project).expect("context created");
        assert_eq!(
            list_directory(&context, ".", Some(MAX_LIST_ENTRIES + 1))
                .expect_err("oversized list rejected")
                .code,
            "invalid_input"
        );
        assert_eq!(
            search(
                &context,
                &"x".repeat(MAX_SEARCH_QUERY_CHARS + 1),
                None,
                true,
                None,
            )
            .expect_err("oversized query rejected")
            .code,
            "invalid_input"
        );
        fs::remove_dir_all(root).expect("fixture removed");
    }

    #[tokio::test]
    async fn provider_dispatcher_uses_strict_arguments_and_returns_a_typed_observation() {
        let (project, root) = fixture();
        let context = RepositoryContext::from_project(&project).expect("context created");
        let call = ProviderToolCall {
            provider_call_id: Some("provider-call-1".to_string()),
            name: READ_FILE_TOOL_ID.to_string(),
            arguments: serde_json::json!({"path": "src/lib.rs", "max_lines": 1}),
        };
        let result = execute_provider_call(&context, &call)
            .await
            .expect("call dispatched");
        assert_eq!(result.provider_call_id, call.provider_call_id);
        assert_eq!(result.name, READ_FILE_TOOL_ID);
        let observation: serde_json::Value =
            serde_json::from_str(&result.content).expect("observation is JSON");
        assert_eq!(observation["path"], "src/lib.rs");
        assert_eq!(observation["truncated"], true);

        let invalid = ProviderToolCall {
            provider_call_id: None,
            name: READ_FILE_TOOL_ID.to_string(),
            arguments: serde_json::json!({"path": "src/lib.rs", "unexpected": true}),
        };
        let error = execute_provider_call(&context, &invalid)
            .await
            .expect_err("unknown argument rejected");
        assert_eq!(error.code, "invalid_tool_arguments");
        assert!(error.message.contains("unknown field `unexpected`"));
        assert!(error.message.contains("match the tool schema"));

        fs::remove_dir_all(root).expect("fixture removed");
    }

    #[tokio::test]
    async fn every_model_supplied_repository_path_is_confined() {
        let (project, root) = fixture();
        let context = RepositoryContext::from_project(&project).expect("context created");
        let calls = [
            (
                LIST_DIRECTORY_TOOL_ID,
                serde_json::json!({"path": "../outside"}),
            ),
            (READ_FILE_TOOL_ID, serde_json::json!({"path": "../outside"})),
            (
                SEARCH_TOOL_ID,
                serde_json::json!({"query": "secret", "path": "../outside"}),
            ),
        ];

        for (name, arguments) in calls {
            let error = execute_provider_call(
                &context,
                &ProviderToolCall {
                    provider_call_id: None,
                    name: name.to_string(),
                    arguments,
                },
            )
            .await
            .expect_err("path traversal must fail");
            assert_eq!(error.code, "invalid_repository_path", "tool {name}");
        }

        fs::remove_dir_all(root).expect("fixture removed");
    }

    #[tokio::test]
    async fn git_status_and_diff_use_the_bound_repository() {
        if std::process::Command::new("git")
            .arg("--version")
            .output()
            .is_err()
        {
            return;
        }
        let (project, root) = fixture();
        let context = RepositoryContext::from_project(&project).expect("context created");
        let error = git_status(&context)
            .await
            .expect_err("Git does not discover a parent repository");
        assert_eq!(error.code, "git_repository_required");
        fs::write(root.join(".git"), "gitdir: ../outside\n").expect("external Git pointer written");
        let error = git_status(&context)
            .await
            .expect_err("external Git metadata rejected");
        assert_eq!(error.code, "git_metadata_outside_repository");
        fs::remove_file(root.join(".git")).expect("Git pointer removed");
        let run = |args: &[&str]| {
            let status = std::process::Command::new("git")
                .arg("-C")
                .arg(&root)
                .args(args)
                .status()
                .expect("git starts");
            assert!(status.success());
        };
        run(&["init", "--quiet"]);
        fs::write(root.join("tracked.txt"), "staged\n").expect("tracked file written");
        run(&["add", "tracked.txt"]);
        fs::write(root.join("tracked.txt"), "working\n").expect("working change written");

        let status = git_status(&context).await.expect("status read");
        assert!(!status.clean);
        assert!(status.porcelain.contains("tracked.txt"));
        let diff = git_diff(&context).await.expect("diff read");
        assert!(diff.staged.contains("+staged"));
        assert!(diff.working_tree.contains("+working"));

        fs::write(root.join("large.txt"), "x\n".repeat(MAX_GIT_OUTPUT_BYTES))
            .expect("large diff file written");
        run(&["add", "large.txt"]);
        let error = git_diff(&context)
            .await
            .expect_err("oversized Git output rejected");
        assert_eq!(error.code, "git_output_too_large");

        fs::remove_dir_all(root).expect("fixture removed");
    }
}
