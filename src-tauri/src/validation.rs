//! COR-008: centralized native input validation for values that cross the Rust trust
//! boundary from the frontend before they reach a provider request, a filesystem operation,
//! or a persisted config. URL/destination validation lives in [`crate::security`] since it is
//! itself a Rust trust-boundary concern tied to SEC-001; this module covers numeric generation
//! parameters, opaque entity IDs, and filesystem paths. Validators return stable, safe
//! [`AppError`] values without leaking technical internals; malformed values use `invalid_input`,
//! while a required existing file uses `file_not_found` so its domain command can map that state.

use crate::errors::AppError;
use std::path::{Path, PathBuf};

pub const MAX_ENTITY_ID_BYTES: usize = 128;

/// IDs are intentionally opaque rather than forced to UUID syntax because supported imports may
/// preserve IDs from earlier versions. The trust boundary still rejects blank, oversized, or
/// control-bearing values before they reach queries or logs.
pub fn validate_entity_id<'a>(value: &'a str, label: &str) -> Result<&'a str, AppError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(AppError::invalid_input(format!("{label} cannot be empty.")));
    }
    if trimmed.len() > MAX_ENTITY_ID_BYTES {
        return Err(AppError::invalid_input(format!(
            "{label} must be at most {MAX_ENTITY_ID_BYTES} bytes."
        )));
    }
    if trimmed.chars().any(char::is_control) {
        return Err(AppError::invalid_input(format!(
            "{label} must not contain control characters."
        )));
    }
    Ok(trimmed)
}

fn reject_ambiguous_path(path: &Path, label: &str) -> Result<(), AppError> {
    if path.components().any(|component| {
        matches!(
            component,
            std::path::Component::ParentDir | std::path::Component::CurDir
        )
    }) {
        return Err(AppError::invalid_input(format!(
            "{label} must not contain '.' or '..' path segments."
        )));
    }
    Ok(())
}

fn validate_absolute_path(raw_path: &str, label: &str) -> Result<PathBuf, AppError> {
    let trimmed = raw_path.trim();
    if trimmed.is_empty() {
        return Err(AppError::invalid_input(format!("{label} cannot be empty.")));
    }
    if trimmed.contains('\0') {
        return Err(AppError::invalid_input(format!(
            "{label} must not contain a null byte."
        )));
    }

    let path = PathBuf::from(trimmed);
    if !path.is_absolute() {
        return Err(AppError::invalid_input(format!(
            "{label} must be absolute."
        )));
    }
    reject_ambiguous_path(&path, label)?;
    Ok(path)
}

fn normalize_canonical_path(canonical: PathBuf) -> PathBuf {
    #[cfg(windows)]
    {
        let display = canonical.to_string_lossy();
        if let Some(rest) = display.strip_prefix(r"\\?\UNC\") {
            return PathBuf::from(format!(r"\\{rest}"));
        }
        if let Some(rest) = display.strip_prefix(r"\\?\") {
            let bytes = rest.as_bytes();
            if bytes.len() >= 3
                && bytes[0].is_ascii_alphabetic()
                && bytes[1] == b':'
                && (bytes[2] == b'\\' || bytes[2] == b'/')
            {
                return PathBuf::from(rest);
            }
        }
    }
    canonical
}

pub(crate) fn canonicalize_for_use(path: &Path, label: &str) -> Result<PathBuf, AppError> {
    let canonical = std::fs::canonicalize(path).map_err(|_| {
        AppError::invalid_input(format!("{label} could not be canonicalized safely."))
    })?;
    // `std::fs::canonicalize` uses Windows verbatim (`\\?\`) paths. They are correct for
    // Win32 file APIs but are not accepted by SQLite's `file:` URI parser and are needlessly
    // exposed in UI. Preserve identity while converting ordinary drive/UNC forms back to their
    // interoperable spelling.
    Ok(normalize_canonical_path(canonical))
}

/// Compares filesystem identity when paths exist, with a lexical fallback that still normalizes
/// Windows verbatim (`\\?\`) spelling. The fallback matters after a managed file has been
/// removed but its matching provenance record still needs to be cleared.
pub(crate) fn paths_refer_to_same_location(left: &Path, right: &Path) -> bool {
    let normalized = |path: &Path| {
        canonicalize_for_use(path, "Path identity")
            .unwrap_or_else(|_| normalize_canonical_path(path.to_path_buf()))
    };
    normalized(left) == normalized(right)
}

/// SEC-007 target policy: reject a symlink at the user-selected leaf, but resolve any symlinked
/// ancestor into one canonical location. The latter is required for normal platform aliases such
/// as macOS `/var` -> `/private/var`; rejecting every linked ancestor would make legitimate save
/// locations unusable. Missing path suffixes are appended only after the closest existing
/// ancestor has been canonicalized and confirmed to be a directory.
fn canonicalize_target_path(
    path: &Path,
    label: &str,
    reject_leaf_symlink: bool,
) -> Result<PathBuf, AppError> {
    let mut existing_ancestor = None;
    for candidate in path.ancestors() {
        match std::fs::symlink_metadata(candidate) {
            Ok(metadata) => {
                if reject_leaf_symlink && candidate == path && metadata.file_type().is_symlink() {
                    return Err(AppError::invalid_input(format!(
                        "{label} must not be a symbolic link."
                    )));
                }
                existing_ancestor = Some((candidate, metadata));
                break;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => {
                return Err(AppError::invalid_input(format!(
                    "{label} could not be inspected safely."
                )));
            }
        }
    }
    let (existing_ancestor, _) = existing_ancestor.ok_or_else(|| {
        AppError::invalid_input(format!(
            "{label} is not rooted in an accessible filesystem location."
        ))
    })?;
    let canonical_ancestor = canonicalize_for_use(existing_ancestor, label)?;
    if existing_ancestor != path {
        let canonical_metadata = std::fs::metadata(&canonical_ancestor).map_err(|_| {
            AppError::invalid_input(format!("{label} could not be inspected safely."))
        })?;
        if !canonical_metadata.is_dir() {
            return Err(AppError::invalid_input(format!(
                "{label} has a parent path that is not a directory."
            )));
        }
    }
    let missing_suffix = path.strip_prefix(existing_ancestor).map_err(|_| {
        AppError::invalid_input(format!("{label} could not be canonicalized safely."))
    })?;
    Ok(canonical_ancestor.join(missing_suffix))
}

/// Validates a user-selected directory that may be created by the operation. Existing targets
/// must already be real directories; missing targets are accepted only when their closest
/// existing ancestor resolves to a real directory. The returned path is canonicalized.
pub fn validate_directory_target_path(raw_path: &str, label: &str) -> Result<PathBuf, AppError> {
    let path = validate_absolute_path(raw_path, label)?;
    let canonical_path = canonicalize_target_path(&path, label, false)?;
    if let Ok(metadata) = std::fs::symlink_metadata(&canonical_path) {
        if !metadata.is_dir() {
            return Err(AppError::invalid_input(format!(
                "{label} must point to a directory."
            )));
        }
    }
    Ok(canonical_path)
}

/// Validates a user-selected directory that must already exist. Unlike a workspace target, a
/// Repository is never created implicitly from a typo; the canonical directory the user selected
/// is what gets persisted and later used for containment checks.
pub fn validate_existing_directory_path(raw_path: &str, label: &str) -> Result<PathBuf, AppError> {
    let path = validate_absolute_path(raw_path, label)?;
    let metadata = std::fs::symlink_metadata(&path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            AppError::new("directory_not_found", format!("{label} was not found."))
        } else {
            AppError::invalid_input(format!("{label} could not be inspected safely."))
        }
    })?;
    if !metadata.is_dir() {
        return Err(AppError::invalid_input(format!(
            "{label} must point to a directory."
        )));
    }
    canonicalize_for_use(&path, label)
}

/// COR-007's collision-safe write probe, generalized for both the storage Workspace and an Ark
/// Code Repository. Callers supply a UUID-based name and their own error namespace; `create_new`
/// makes an existing user file untouchable even under an adversarial collision.
pub(crate) fn probe_writable_directory(
    root: &Path,
    probe_name: &str,
    label: &str,
    error_prefix: &str,
) -> Result<(), AppError> {
    let probe = root.join(probe_name);
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&probe)
        .map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                return AppError::new(
                    format!("{error_prefix}_missing"),
                    format!("The {label} folder disappeared while Ark was checking it."),
                );
            }
            let classified = AppError::from(error);
            let code = match classified.code.as_str() {
                "io_error" => format!("{error_prefix}_error"),
                "workspace_read_only" => format!("{error_prefix}_read_only"),
                _ => classified.code.clone(),
            };
            AppError::new(
                code,
                format!("The {label} folder is not writable: {}", classified.message),
            )
        })?;
    if let Err(error) = crate::file_permissions::harden_file(&probe) {
        let cleanup_error = std::fs::remove_file(&probe).err();
        let cleanup_detail = cleanup_error
            .map(|cleanup| format!(" Probe cleanup also failed: {cleanup}"))
            .unwrap_or_default();
        return Err(AppError::new(
            format!("{error_prefix}_error"),
            format!(
                "Ark could not secure its temporary {label} write probe: {}.{cleanup_detail}",
                error.message
            ),
        ));
    }
    std::fs::remove_file(&probe).map_err(|error| {
        AppError::new(
            format!("{error_prefix}_cleanup_failed"),
            format!(
                "The {label} folder is writable, but Ark could not remove its probe '{}': {error}",
                probe.display()
            ),
        )
    })?;
    Ok(())
}

/// Validates and canonicalizes a file that must already exist. Returning the canonical path makes
/// the checked object authoritative for subsequent reads instead of reverting to the unchecked
/// spelling supplied across IPC.
pub fn validate_existing_file_path(raw_path: &str, label: &str) -> Result<PathBuf, AppError> {
    let path = validate_absolute_path(raw_path, label)?;
    let metadata = std::fs::symlink_metadata(&path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            AppError::new("file_not_found", format!("{label} was not found."))
        } else {
            AppError::invalid_input(format!("{label} could not be inspected safely."))
        }
    })?;
    if metadata.file_type().is_symlink() {
        return Err(AppError::invalid_input(format!(
            "{label} must not be a symbolic link."
        )));
    }
    if !metadata.is_file() {
        return Err(AppError::invalid_input(format!(
            "{label} must point to a regular file."
        )));
    }
    canonicalize_for_use(&path, label)
}

/// Validates a user-selected output file. The file may not exist yet, but its existing ancestor
/// chain is canonicalized; an existing target must be a regular file and not a symlink.
pub fn validate_output_file_path(raw_path: &str, label: &str) -> Result<PathBuf, AppError> {
    let path = validate_absolute_path(raw_path, label)?;
    let canonical_path = canonicalize_target_path(&path, label, true)?;
    if let Ok(metadata) = std::fs::symlink_metadata(&path) {
        if !metadata.is_file() {
            return Err(AppError::invalid_input(format!(
                "{label} must point to a regular file."
            )));
        }
    }
    Ok(canonical_path)
}

/// Practical range shared by Ollama and OpenAI-compatible APIs; values outside this are
/// either meaningless (negative) or produce degenerate output without any real benefit.
pub const MIN_TEMPERATURE: f64 = 0.0;
pub const MAX_TEMPERATURE: f64 = 2.0;

/// A generation must produce at least one token; the upper bound is a generous ceiling —
/// individual providers/models further constrain this by their own context window.
pub const MIN_MAX_TOKENS: i64 = 1;
pub const MAX_MAX_TOKENS: i64 = 1_000_000;

/// Validates an optional sampling temperature. `None` (provider default) always passes.
pub fn validate_temperature(value: Option<f64>) -> Result<Option<f64>, AppError> {
    let Some(value) = value else {
        return Ok(None);
    };

    if !value.is_finite() {
        return Err(AppError::invalid_input(
            "Temperature must be a finite number (not NaN or infinite).",
        ));
    }
    if !(MIN_TEMPERATURE..=MAX_TEMPERATURE).contains(&value) {
        return Err(AppError::invalid_input(format!(
            "Temperature must be between {MIN_TEMPERATURE} and {MAX_TEMPERATURE}."
        )));
    }

    Ok(Some(value))
}

/// Validates an optional max-tokens limit. `None` (provider default) always passes.
pub fn validate_max_tokens(value: Option<i64>) -> Result<Option<i64>, AppError> {
    let Some(value) = value else {
        return Ok(None);
    };

    if !(MIN_MAX_TOKENS..=MAX_MAX_TOKENS).contains(&value) {
        return Err(AppError::invalid_input(format!(
            "Max tokens must be between {MIN_MAX_TOKENS} and {MAX_MAX_TOKENS}."
        )));
    }

    Ok(Some(value))
}

/// FTR-004: a generous bound, not a tuned limit — large enough for genuinely long instructions,
/// small enough to keep a single conversation setting from becoming an unbounded-size liability
/// (every provider request re-sends it in full on every message).
pub const MAX_SYSTEM_PROMPT_CHARS: usize = 32_000;

/// Validates a per-conversation system prompt override. `None` or a blank/whitespace-only
/// string both mean "no override, inherit the provider default" — trimmed and normalized to
/// `None` rather than persisting an empty string, so `Option::is_some()` is always a reliable
/// "the user actually set one" check everywhere this field is read.
pub fn validate_system_prompt(value: Option<String>) -> Result<Option<String>, AppError> {
    let Some(value) = value else {
        return Ok(None);
    };
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    if trimmed.chars().count() > MAX_SYSTEM_PROMPT_CHARS {
        return Err(AppError::invalid_input(format!(
            "System prompt must be at most {MAX_SYSTEM_PROMPT_CHARS} characters."
        )));
    }
    Ok(Some(trimmed.to_string()))
}

/// UX: the fixed allow-list of Ark-level "response style" presets — must stay in sync with
/// `generation.rs`'s `response_style_instruction` match arms (that function's own tests assert
/// every value here maps to a real instruction). Not every low-level provider parameter Ark could
/// expose — a deliberately small, human-readable set of behavioral presets.
pub const RESPONSE_STYLE_VALUES: &[&str] = &[
    "balanced",
    "concise",
    "detailed",
    "explanatory",
    "technical",
    "creative",
];

/// UX: mirrors `RESPONSE_STYLE_VALUES` for tone — see `generation.rs`'s `tone_instruction`.
pub const TONE_VALUES: &[&str] = &["neutral", "professional", "friendly", "direct", "casual"];

/// Validates a response-style override. `None` or blank means "no override" (same normalization
/// as `validate_system_prompt`); anything present must be one of `RESPONSE_STYLE_VALUES` — this
/// is a closed preset, not free text, so an unrecognized value is a real input error, not
/// something to silently pass through.
pub fn validate_response_style(value: Option<String>) -> Result<Option<String>, AppError> {
    let Some(value) = value else {
        return Ok(None);
    };
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    if !RESPONSE_STYLE_VALUES.contains(&trimmed) {
        return Err(AppError::invalid_input(format!(
            "Response style must be one of: {}.",
            RESPONSE_STYLE_VALUES.join(", ")
        )));
    }
    Ok(Some(trimmed.to_string()))
}

/// Validates a tone override — mirrors `validate_response_style` exactly, against `TONE_VALUES`.
pub fn validate_tone(value: Option<String>) -> Result<Option<String>, AppError> {
    let Some(value) = value else {
        return Ok(None);
    };
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    if !TONE_VALUES.contains(&trimmed) {
        return Err(AppError::invalid_input(format!(
            "Tone must be one of: {}.",
            TONE_VALUES.join(", ")
        )));
    }
    Ok(Some(trimmed.to_string()))
}

/// FTR-003: validates a persona's instructions. Unlike `validate_system_prompt`, `None`/blank is
/// rejected rather than normalized away — a persona's entire purpose is its prompt content, so
/// (unlike a conversation's optional override) an empty one is a user error, not a valid "no
/// override" state. Reuses `MAX_SYSTEM_PROMPT_CHARS` since this is the same kind of content.
pub fn validate_persona_instructions(value: &str) -> Result<String, AppError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(AppError::invalid_input(
            "Persona instructions cannot be empty.",
        ));
    }
    if trimmed.chars().count() > MAX_SYSTEM_PROMPT_CHARS {
        return Err(AppError::invalid_input(format!(
            "Persona instructions must be at most {MAX_SYSTEM_PROMPT_CHARS} characters."
        )));
    }
    Ok(trimmed.to_string())
}

/// CMP-001: a generous bound, not a tuned limit — text attachments are meant to be genuinely
/// read as context, not truncated mid-thought, but an unbounded value would let one attachment
/// exhaust memory or blow the token budget of every subsequent request in the conversation.
pub const MAX_ATTACHMENT_BYTES: usize = 2 * 1024 * 1024;
pub const MAX_ATTACHMENT_FILE_NAME_CHARS: usize = 255;

/// CMP-001: validates a text attachment before it's stored. `content` arrives as whatever text
/// the frontend read from the picked/dropped/pasted file (`File.text()` or the clipboard's
/// plain-text data) — this is the actual "content sniffing does not trust extension alone" check
/// (acceptance criterion 1): a `.txt`-named file whose bytes don't actually decode as plausible
/// text is rejected here regardless of what its name claims, via the NUL-byte heuristic below
/// (a strong, cheap signal that the browser's UTF-8 decode produced garbage from binary input —
/// genuine text content essentially never contains one).
pub fn validate_attachment(file_name: &str, content: &str) -> Result<(String, String), AppError> {
    let trimmed_name = file_name.trim();
    if trimmed_name.is_empty() {
        return Err(AppError::invalid_input(
            "Attachment file name cannot be empty.",
        ));
    }
    if trimmed_name.chars().count() > MAX_ATTACHMENT_FILE_NAME_CHARS {
        return Err(AppError::invalid_input(format!(
            "Attachment file name must be at most {MAX_ATTACHMENT_FILE_NAME_CHARS} characters."
        )));
    }
    if trimmed_name.chars().any(char::is_control) {
        return Err(AppError::invalid_input(
            "Attachment file name must not contain control characters.",
        ));
    }

    if content.is_empty() {
        return Err(AppError::invalid_input(
            "Attachment content cannot be empty.",
        ));
    }
    if content.len() > MAX_ATTACHMENT_BYTES {
        return Err(AppError::invalid_input(format!(
            "Attachment content must be at most {} MB.",
            MAX_ATTACHMENT_BYTES / (1024 * 1024)
        )));
    }
    if content.contains('\0') {
        return Err(AppError::invalid_input(
            "This file does not look like text — Ark only accepts plain-text attachments in this build.",
        ));
    }

    Ok((trimmed_name.to_string(), content.to_string()))
}

/// CMP-003: a conversation note is a short scratch memo, not a document store — generous enough
/// for real notes, small enough that the built-in "notes" tool can't become an unbounded-size
/// liability the way an attachment-scale limit would be wrong for.
pub const MAX_NOTE_CONTENT_CHARS: usize = 8_000;

/// Validates a conversation note's content before it is stored via the "notes" tool.
pub fn validate_note_content(value: &str) -> Result<String, AppError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(AppError::invalid_input("Note content cannot be empty."));
    }
    if trimmed.chars().count() > MAX_NOTE_CONTENT_CHARS {
        return Err(AppError::invalid_input(format!(
            "Note content must be at most {MAX_NOTE_CONTENT_CHARS} characters."
        )));
    }
    Ok(trimmed.to_string())
}

/// CMP-003: bounds for an explicit, user-chosen capability grant TTL (the Settings-panel "grant
/// this tool access" path). ADR 0002 requires narrow, time-boxed grants only — an hour is a
/// generous ceiling for a chat-safe, no-network, no-secret tool, not an "effectively unlimited"
/// escape hatch.
pub const MIN_GRANT_TTL_MINUTES: i64 = 1;
pub const MAX_GRANT_TTL_MINUTES: i64 = 60;

/// CMP-004: a web search query is short by nature — far tighter than a note's 8,000-character
/// ceiling. Bounds what's sent to Brave and, by extension, what shows up in the preview/audit
/// paths.
pub const MAX_SEARCH_QUERY_CHARS: usize = 400;

/// Validates a web search query before it's previewed/sent to Brave Search.
pub fn validate_search_query(value: &str) -> Result<String, AppError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(AppError::invalid_input("Search query cannot be empty."));
    }
    if trimmed.chars().count() > MAX_SEARCH_QUERY_CHARS {
        return Err(AppError::invalid_input(format!(
            "Search query must be at most {MAX_SEARCH_QUERY_CHARS} characters."
        )));
    }
    Ok(trimmed.to_string())
}

/// Validates a capability grant TTL in minutes.
pub fn validate_grant_ttl_minutes(value: i64) -> Result<i64, AppError> {
    if !(MIN_GRANT_TTL_MINUTES..=MAX_GRANT_TTL_MINUTES).contains(&value) {
        return Err(AppError::invalid_input(format!(
            "Grant duration must be between {MIN_GRANT_TTL_MINUTES} and {MAX_GRANT_TTL_MINUTES} minutes."
        )));
    }
    Ok(value)
}

/// FTR-005: a branch label is a short, glanceable name shown next to a "Response N" ordinal in
/// the alternatives switcher — not free-form prose, so the bound is much tighter than
/// `MAX_SYSTEM_PROMPT_CHARS`.
pub const MAX_BRANCH_NAME_CHARS: usize = 80;

/// Validates a branch (message revision) label. `None` or blank/whitespace-only both mean "no
/// label, fall back to the default ordinal presentation" — trimmed and normalized to `None`
/// rather than persisting an empty string, matching `validate_system_prompt`'s convention.
pub fn validate_branch_name(value: Option<String>) -> Result<Option<String>, AppError> {
    let Some(value) = value else {
        return Ok(None);
    };
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    if trimmed.chars().count() > MAX_BRANCH_NAME_CHARS {
        return Err(AppError::invalid_input(format!(
            "Branch name must be at most {MAX_BRANCH_NAME_CHARS} characters."
        )));
    }
    Ok(Some(trimmed.to_string()))
}

/// Validates a workspace root path before it is persisted/probed. A NUL byte is rejected
/// because it silently truncates the effective path in several OS filesystem APIs (a path
/// with a NUL is not a valid Rust `&str`-derived filesystem path assumption on any supported
/// platform), which could make the path Ark *validates* differ from the path Ark actually
/// *uses*.
pub fn validate_workspace_path(raw_path: &str) -> Result<PathBuf, AppError> {
    validate_directory_target_path(raw_path, "Workspace path")
}

/// Validates a built-in-runtime model file path before it is passed to `llama-server` as a
/// CLI argument. Existence/type/extension checks turn "the process failed to start for an
/// opaque reason 30 seconds later" into an immediate, specific error.
pub fn validate_model_path(raw_path: &str) -> Result<PathBuf, AppError> {
    let path = validate_absolute_path(raw_path, "Model file path")?;
    let has_gguf_extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.eq_ignore_ascii_case("gguf"))
        .unwrap_or(false);
    if !has_gguf_extension {
        return Err(AppError::invalid_input(
            "Model file must have a .gguf extension (GGUF format required).",
        ));
    }

    validate_existing_file_path(
        path.to_str().ok_or_else(|| {
            AppError::invalid_input("Model file path must contain valid Unicode.")
        })?,
        "Model file path",
    )
    .map_err(|error| {
        if error.code == "file_not_found" {
            AppError::invalid_input(error.message)
        } else {
            error
        }
    })
}

/// The GGUF format's fixed 4-byte magic number at file offset 0.
const GGUF_MAGIC: [u8; 4] = *b"GGUF";

/// A minimal valid GGUF header (magic + uint32 version + uint64 tensor count + uint64 metadata
/// KV count) is already 24 bytes; anything smaller cannot possibly be a real GGUF file. This
/// catches empty, truncated, or placeholder files immediately rather than handing them to
/// `llama-server`, which would fail 30+ seconds later with an opaque process-exit code.
const MIN_GGUF_BYTES: u64 = 32;

/// A generous absolute ceiling, not a real-world model-size expectation or a hardware-fit
/// prediction — that nuanced "does this fit this machine's RAM/VRAM/context budget" assessment
/// is PERF-004's job ("Preflight estimates model + context memory and free disk/RAM with a
/// confidence label"). This check exists only to refuse an adversarial or corrupted size (a
/// sparse file, a filesystem quirk, a deliberately malformed path) before it reaches the launch
/// path, without second-guessing legitimate large local models loaded via mmap.
const MAX_GGUF_BYTES: u64 = 1_000 * 1024 * 1024 * 1024;

/// SEC-007: deeper content validation beyond `validate_model_path`'s cheap path-shape checks —
/// run once, immediately before the file is handed to `llama-server` as a launch argument.
/// Deliberately a separate function so the cheap check (used wherever a path is merely being
/// accepted or displayed) never pays the cost of opening and reading the file.
pub fn validate_gguf_file(path: &Path) -> Result<(), AppError> {
    // `symlink_metadata` does not follow symlinks — the point. A `.gguf`-named symlink pointing
    // at an arbitrary file (or one whose target changes between this check and the actual
    // read/launch — a classic TOCTOU pattern) is rejected outright rather than transparently
    // followed.
    let link_metadata = std::fs::symlink_metadata(path).map_err(|error| {
        AppError::invalid_input(format!("Could not read the model file: {error}"))
    })?;
    if link_metadata.file_type().is_symlink() {
        return Err(AppError::invalid_input(
            "Model file must be a regular file, not a symlink.",
        ));
    }
    if !link_metadata.is_file() {
        return Err(AppError::invalid_input(
            "Model file must be a regular file, not a directory, device, or pipe.",
        ));
    }

    let size = link_metadata.len();
    if size < MIN_GGUF_BYTES {
        return Err(AppError::invalid_input(format!(
            "Model file is too small ({size} bytes) to be a valid GGUF file."
        )));
    }
    if size > MAX_GGUF_BYTES {
        return Err(AppError::invalid_input(
            "Model file size is implausible for a GGUF model file.",
        ));
    }

    let mut file = std::fs::File::open(path).map_err(|error| {
        AppError::invalid_input(format!("Could not open the model file: {error}"))
    })?;
    let mut magic = [0u8; 4];
    std::io::Read::read_exact(&mut file, &mut magic).map_err(|error| {
        AppError::invalid_input(format!("Could not read the model file header: {error}"))
    })?;
    if magic != GGUF_MAGIC {
        return Err(AppError::invalid_input(
            "Model file does not start with the GGUF format signature.",
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opaque_ids_accept_import_compatible_values_and_reject_unsafe_boundaries() {
        assert_eq!(
            validate_entity_id(" legacy-message_42 ", "Message ID").unwrap(),
            "legacy-message_42"
        );
        for invalid in ["", "   ", "line\nbreak", "nul\0byte"] {
            let error = validate_entity_id(invalid, "Message ID").unwrap_err();
            assert_eq!(error.code, "invalid_input");
            if !invalid.is_empty() {
                assert!(!error.message.contains(invalid));
            }
        }
        assert!(validate_entity_id(&"x".repeat(MAX_ENTITY_ID_BYTES), "Message ID").is_ok());
        assert!(validate_entity_id(&"x".repeat(MAX_ENTITY_ID_BYTES + 1), "Message ID").is_err());
    }

    #[test]
    fn temperature_none_always_passes() {
        assert_eq!(validate_temperature(None).unwrap(), None);
    }

    #[test]
    fn temperature_accepts_the_full_valid_range_inclusive() {
        assert_eq!(validate_temperature(Some(0.0)).unwrap(), Some(0.0));
        assert_eq!(validate_temperature(Some(0.7)).unwrap(), Some(0.7));
        assert_eq!(validate_temperature(Some(2.0)).unwrap(), Some(2.0));
    }

    #[test]
    fn temperature_rejects_out_of_range_values() {
        assert!(validate_temperature(Some(-0.01)).is_err());
        assert!(validate_temperature(Some(2.01)).is_err());
        assert!(validate_temperature(Some(-100.0)).is_err());
        assert!(validate_temperature(Some(1000.0)).is_err());
    }

    #[test]
    fn temperature_rejects_nan_and_infinity() {
        let error = validate_temperature(Some(f64::NAN)).unwrap_err();
        assert_eq!(error.code, "invalid_input");
        assert!(validate_temperature(Some(f64::INFINITY)).is_err());
        assert!(validate_temperature(Some(f64::NEG_INFINITY)).is_err());
    }

    #[test]
    fn max_tokens_none_always_passes() {
        assert_eq!(validate_max_tokens(None).unwrap(), None);
    }

    #[test]
    fn max_tokens_accepts_the_full_valid_range_inclusive() {
        assert_eq!(validate_max_tokens(Some(1)).unwrap(), Some(1));
        assert_eq!(validate_max_tokens(Some(2048)).unwrap(), Some(2048));
        assert_eq!(
            validate_max_tokens(Some(1_000_000)).unwrap(),
            Some(1_000_000)
        );
    }

    #[test]
    fn max_tokens_rejects_zero_negative_and_overflow_adjacent_values() {
        assert!(validate_max_tokens(Some(0)).is_err());
        assert!(validate_max_tokens(Some(-1)).is_err());
        assert!(validate_max_tokens(Some(-9_223_372_036_854_775_808)).is_err()); // i64::MIN
        assert!(validate_max_tokens(Some(1_000_001)).is_err());
        assert!(validate_max_tokens(Some(9_223_372_036_854_775_807)).is_err()); // i64::MAX
    }

    #[test]
    fn workspace_path_rejects_empty_and_whitespace_only() {
        assert!(validate_workspace_path("").is_err());
        assert!(validate_workspace_path("   ").is_err());
    }

    #[test]
    fn workspace_path_rejects_null_bytes() {
        let error = validate_workspace_path("C:\\Users\\me\\Ark\0evil").unwrap_err();
        assert_eq!(error.code, "invalid_input");
    }

    #[test]
    fn workspace_path_rejects_relative_paths() {
        assert!(validate_workspace_path("relative/path").is_err());
        assert!(validate_workspace_path("./relative").is_err());
        assert!(validate_workspace_path("../parent").is_err());
    }

    #[test]
    fn workspace_path_rejects_ambiguous_traversal_segments() {
        let ambiguous = std::env::temp_dir()
            .join("ark-child")
            .join("..")
            .join("other");
        assert!(validate_workspace_path(ambiguous.to_str().expect("utf8 path")).is_err());
    }

    #[test]
    fn workspace_path_accepts_absolute_paths_and_trims_whitespace() {
        let path =
            std::env::temp_dir().join(format!("ark-workspace-target-{}", uuid::Uuid::new_v4()));
        let accepted = validate_workspace_path(&format!("  {}  ", path.display())).unwrap();
        assert_eq!(
            accepted,
            canonicalize_for_use(&std::env::temp_dir(), "Temp directory")
                .expect("canonical temp directory")
                .join(path.file_name().expect("target name"))
        );
    }

    #[test]
    fn model_path_rejects_empty_null_byte_and_wrong_extension() {
        assert!(validate_model_path("").is_err());
        assert!(validate_model_path("model.gguf\0evil").is_err());
        assert!(
            validate_model_path("model.bin").is_err(),
            "must require the .gguf extension"
        );
        assert!(validate_model_path("model.GGUF.exe").is_err());
    }

    #[test]
    fn model_path_accepts_case_insensitive_gguf_extension() {
        // Extension check passes for both cases; existence is checked after, so a
        // non-existent path with the right extension fails on the existence check
        // specifically, not the extension check — proving the two checks are independent.
        let path =
            std::env::temp_dir().join(format!("ark-missing-model-{}.GGUF", uuid::Uuid::new_v4()));
        let error = validate_model_path(path.to_str().expect("utf8 path")).unwrap_err();
        assert!(
            error.message.contains("not found"),
            "must fail on existence, not extension, for a valid extension"
        );
    }

    #[test]
    fn model_path_rejects_a_nonexistent_file() {
        let path =
            std::env::temp_dir().join(format!("ark-missing-model-{}.gguf", uuid::Uuid::new_v4()));
        let error = validate_model_path(path.to_str().expect("utf8 path")).unwrap_err();
        assert_eq!(error.code, "invalid_input");
        assert!(error.message.contains("not found"));
    }

    #[test]
    fn model_path_rejects_a_directory() {
        let dir =
            std::env::temp_dir().join(format!("ark-validation-test-dir-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        // Directories don't have a meaningful "extension" in the .gguf sense, but to isolate
        // exactly the is_file() check, give it one.
        let fake_dir_with_extension = dir.join("looks-like-a-model.gguf");
        std::fs::create_dir_all(&fake_dir_with_extension).expect("create nested temp dir");

        let path_str = fake_dir_with_extension.to_str().expect("valid utf8 path");
        let error = validate_model_path(path_str).unwrap_err();
        assert!(error.message.contains("regular file"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn model_path_accepts_a_real_gguf_file() {
        let path =
            std::env::temp_dir().join(format!("ark-validation-test-{}.gguf", uuid::Uuid::new_v4()));
        std::fs::write(&path, b"not a real gguf file, just needs to exist")
            .expect("write temp file");

        let path_str = path.to_str().expect("valid utf8 path");
        let accepted = validate_model_path(path_str).expect("valid existing .gguf file must pass");
        assert_eq!(
            accepted,
            canonicalize_for_use(&path, "Model path").expect("canonical fixture path")
        );

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn shared_file_path_policy_type_checks_inputs_and_outputs() {
        let root = std::env::temp_dir().join(format!("ark-path-policy-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).expect("create fixture root");
        let input = root.join("input.sqlite3");
        std::fs::write(&input, b"fixture").expect("write input fixture");

        assert_eq!(
            validate_existing_file_path(input.to_str().expect("utf8 path"), "Input file")
                .expect("regular input passes"),
            canonicalize_for_use(&input, "Input file").expect("canonical input")
        );
        assert!(
            validate_existing_file_path(root.to_str().expect("utf8 path"), "Input file").is_err()
        );

        let output = root.join("nested").join("output.txt");
        assert!(
            validate_output_file_path(output.to_str().expect("utf8 path"), "Output file").is_ok()
        );
        assert!(
            validate_output_file_path(root.to_str().expect("utf8 path"), "Output file").is_err()
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[cfg(windows)]
    #[test]
    fn canonical_paths_use_windows_spelling_accepted_by_sqlite_file_uris() {
        let root =
            std::env::temp_dir().join(format!("ark-canonical-windows-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).expect("create fixture root");

        let canonical =
            validate_directory_target_path(root.to_str().expect("utf8 path"), "Workspace path")
                .expect("canonical path");

        assert!(!canonical.to_string_lossy().starts_with(r"\\?\"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[cfg(windows)]
    #[test]
    fn path_identity_treats_verbatim_and_normal_drive_spellings_as_equal() {
        assert!(paths_refer_to_same_location(
            Path::new(r"\\?\C:\Ark\models\managed.gguf"),
            Path::new(r"C:\Ark\models\managed.gguf"),
        ));
        assert!(!paths_refer_to_same_location(
            Path::new(r"\\?\C:\Ark\models\other.gguf"),
            Path::new(r"C:\Ark\models\managed.gguf"),
        ));
    }

    #[cfg(unix)]
    #[test]
    fn shared_file_path_policy_rejects_symlinked_leaves_and_canonicalizes_ancestors() {
        use std::os::unix::fs::symlink;

        let root =
            std::env::temp_dir().join(format!("ark-path-policy-link-{}", uuid::Uuid::new_v4()));
        let real = root.join("real");
        std::fs::create_dir_all(&real).expect("create real directory");
        let real_file = real.join("model.gguf");
        std::fs::write(&real_file, b"GGUF fixture").expect("write real file");

        let file_link = root.join("model.gguf");
        symlink(&real_file, &file_link).expect("create file symlink");
        let file_error =
            validate_existing_file_path(file_link.to_str().expect("utf8 path"), "Model file")
                .expect_err("symlinked file must be rejected");
        assert!(file_error.message.contains("symbolic link"));

        let directory_link = root.join("linked-directory");
        symlink(&real, &directory_link).expect("create directory symlink");
        let target = directory_link.join("new-workspace");
        let canonical_target =
            validate_directory_target_path(target.to_str().expect("utf8 path"), "Workspace path")
                .expect("a linked ancestor is resolved to its canonical location");
        assert_eq!(
            canonical_target,
            canonicalize_for_use(&real, "Real directory")
                .expect("canonical real directory")
                .join("new-workspace")
        );

        let _ = std::fs::remove_dir_all(root);
    }

    fn gguf_temp_path(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "ark-gguf-test-{name}-{}.gguf",
            uuid::Uuid::new_v4()
        ))
    }

    #[test]
    fn validate_gguf_file_accepts_a_plausible_header() {
        let path = gguf_temp_path("valid");
        let mut content = b"GGUF".to_vec();
        content.extend_from_slice(&[0u8; 60]); // padding past the minimum-size floor
        std::fs::write(&path, content).expect("write fixture");

        validate_gguf_file(&path).expect("a file starting with the GGUF magic must pass");

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn validate_gguf_file_rejects_wrong_magic_bytes() {
        let path = gguf_temp_path("wrong-magic");
        let mut content = b"PK\x03\x04".to_vec(); // a real zip file's magic, not GGUF's
        content.extend_from_slice(&[0u8; 60]);
        std::fs::write(&path, content).expect("write fixture");

        let error = validate_gguf_file(&path).unwrap_err();
        assert!(error.message.contains("GGUF format signature"));

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn validate_gguf_file_rejects_a_file_too_small_to_contain_a_header() {
        let path = gguf_temp_path("truncated");
        std::fs::write(&path, b"GGUF").expect("write fixture"); // magic only, no header body

        let error = validate_gguf_file(&path).unwrap_err();
        assert!(error.message.contains("too small"));

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn validate_gguf_file_rejects_an_empty_file() {
        let path = gguf_temp_path("empty");
        std::fs::write(&path, b"").expect("write fixture");

        let error = validate_gguf_file(&path).unwrap_err();
        assert!(error.message.contains("too small"));

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn validate_gguf_file_rejects_a_missing_file() {
        let path = std::env::temp_dir().join(format!(
            "ark-gguf-test-missing-{}.gguf",
            uuid::Uuid::new_v4()
        ));
        assert!(validate_gguf_file(&path).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn validate_gguf_file_rejects_a_symlink_even_with_a_plausible_target() {
        use std::os::unix::fs::symlink;

        let target = gguf_temp_path("symlink-target");
        let mut content = b"GGUF".to_vec();
        content.extend_from_slice(&[0u8; 60]);
        std::fs::write(&target, content).expect("write real target file");

        let link = gguf_temp_path("symlink");
        symlink(&target, &link).expect("create symlink");

        let error = validate_gguf_file(&link).unwrap_err();
        assert!(error.message.contains("symlink"));

        let _ = std::fs::remove_file(&target);
        let _ = std::fs::remove_file(&link);
    }

    #[test]
    fn system_prompt_none_always_passes() {
        assert_eq!(validate_system_prompt(None).unwrap(), None);
    }

    #[test]
    fn system_prompt_normalizes_blank_and_whitespace_only_input_to_none() {
        assert_eq!(validate_system_prompt(Some(String::new())).unwrap(), None);
        assert_eq!(
            validate_system_prompt(Some("   \n\t  ".to_string())).unwrap(),
            None
        );
    }

    #[test]
    fn system_prompt_trims_surrounding_whitespace() {
        assert_eq!(
            validate_system_prompt(Some("  Be concise.  ".to_string())).unwrap(),
            Some("Be concise.".to_string())
        );
    }

    #[test]
    fn system_prompt_accepts_up_to_the_character_limit() {
        let value = "a".repeat(MAX_SYSTEM_PROMPT_CHARS);
        assert_eq!(
            validate_system_prompt(Some(value.clone())).unwrap(),
            Some(value)
        );
    }

    #[test]
    fn system_prompt_rejects_over_the_character_limit() {
        let value = "a".repeat(MAX_SYSTEM_PROMPT_CHARS + 1);
        let error = validate_system_prompt(Some(value)).unwrap_err();
        assert_eq!(error.code, "invalid_input");
    }

    #[test]
    fn response_style_none_and_blank_both_normalize_to_none() {
        assert_eq!(validate_response_style(None).unwrap(), None);
        assert_eq!(
            validate_response_style(Some("   ".to_string())).unwrap(),
            None
        );
    }

    #[test]
    fn response_style_accepts_every_allowed_value() {
        for value in RESPONSE_STYLE_VALUES {
            assert_eq!(
                validate_response_style(Some((*value).to_string())).unwrap(),
                Some((*value).to_string())
            );
        }
    }

    #[test]
    fn response_style_rejects_a_value_outside_the_allow_list() {
        let error = validate_response_style(Some("aggressive".to_string())).unwrap_err();
        assert_eq!(error.code, "invalid_input");
    }

    #[test]
    fn tone_none_and_blank_both_normalize_to_none() {
        assert_eq!(validate_tone(None).unwrap(), None);
        assert_eq!(validate_tone(Some("  ".to_string())).unwrap(), None);
    }

    #[test]
    fn tone_accepts_every_allowed_value() {
        for value in TONE_VALUES {
            assert_eq!(
                validate_tone(Some((*value).to_string())).unwrap(),
                Some((*value).to_string())
            );
        }
    }

    #[test]
    fn tone_rejects_a_value_outside_the_allow_list() {
        let error = validate_tone(Some("sarcastic".to_string())).unwrap_err();
        assert_eq!(error.code, "invalid_input");
    }

    #[test]
    fn attachment_accepts_a_normal_text_file_and_trims_the_name() {
        let (name, content) = validate_attachment("  notes.txt  ", "hello world").unwrap();
        assert_eq!(name, "notes.txt");
        assert_eq!(content, "hello world");
    }

    #[test]
    fn attachment_rejects_a_blank_or_control_bearing_file_name() {
        assert_eq!(
            validate_attachment("   ", "content").unwrap_err().code,
            "invalid_input"
        );
        assert_eq!(
            validate_attachment("notes\u{0007}.txt", "content")
                .unwrap_err()
                .code,
            "invalid_input"
        );
    }

    #[test]
    fn attachment_rejects_empty_content() {
        assert_eq!(
            validate_attachment("notes.txt", "").unwrap_err().code,
            "invalid_input"
        );
    }

    #[test]
    fn attachment_rejects_content_over_the_byte_limit() {
        let oversized = "a".repeat(MAX_ATTACHMENT_BYTES + 1);
        assert_eq!(
            validate_attachment("notes.txt", &oversized)
                .unwrap_err()
                .code,
            "invalid_input"
        );
    }

    #[test]
    fn attachment_accepts_content_at_exactly_the_byte_limit() {
        let exact = "a".repeat(MAX_ATTACHMENT_BYTES);
        assert!(validate_attachment("notes.txt", &exact).is_ok());
    }

    #[test]
    fn attachment_rejects_content_containing_a_nul_byte_as_not_really_text() {
        let error = validate_attachment("notes.txt", "hello\0world").unwrap_err();
        assert_eq!(error.code, "invalid_input");
    }

    #[test]
    fn note_content_trims_and_rejects_blank() {
        assert_eq!(validate_note_content("  hello  ").unwrap(), "hello");
        assert_eq!(
            validate_note_content("   ").unwrap_err().code,
            "invalid_input"
        );
    }

    #[test]
    fn note_content_rejects_over_the_character_limit() {
        let oversized = "a".repeat(MAX_NOTE_CONTENT_CHARS + 1);
        assert_eq!(
            validate_note_content(&oversized).unwrap_err().code,
            "invalid_input"
        );
    }

    #[test]
    fn note_content_accepts_content_at_exactly_the_character_limit() {
        let exact = "a".repeat(MAX_NOTE_CONTENT_CHARS);
        assert!(validate_note_content(&exact).is_ok());
    }

    #[test]
    fn search_query_trims_and_rejects_blank() {
        assert_eq!(
            validate_search_query("  latest rust release  ").unwrap(),
            "latest rust release"
        );
        assert_eq!(
            validate_search_query("   ").unwrap_err().code,
            "invalid_input"
        );
    }

    #[test]
    fn search_query_rejects_over_the_character_limit() {
        let oversized = "a".repeat(MAX_SEARCH_QUERY_CHARS + 1);
        assert_eq!(
            validate_search_query(&oversized).unwrap_err().code,
            "invalid_input"
        );
    }

    #[test]
    fn search_query_accepts_content_at_exactly_the_character_limit() {
        let exact = "a".repeat(MAX_SEARCH_QUERY_CHARS);
        assert!(validate_search_query(&exact).is_ok());
    }

    #[test]
    fn grant_ttl_accepts_the_full_valid_range_inclusive() {
        assert_eq!(
            validate_grant_ttl_minutes(MIN_GRANT_TTL_MINUTES).unwrap(),
            1
        );
        assert_eq!(
            validate_grant_ttl_minutes(MAX_GRANT_TTL_MINUTES).unwrap(),
            60
        );
    }

    #[test]
    fn grant_ttl_rejects_out_of_range_values() {
        assert!(validate_grant_ttl_minutes(0).is_err());
        assert!(validate_grant_ttl_minutes(-1).is_err());
        assert!(validate_grant_ttl_minutes(MAX_GRANT_TTL_MINUTES + 1).is_err());
    }
}
