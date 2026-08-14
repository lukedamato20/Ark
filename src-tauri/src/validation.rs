//! COR-008: centralized native input validation for values that cross the Rust trust
//! boundary from the frontend before they reach a provider request, a filesystem operation,
//! or a persisted config. URL/destination validation lives in [`crate::security`] since it is
//! itself a Rust trust-boundary concern tied to SEC-001; this module covers numeric generation
//! parameters, opaque entity IDs, and filesystem paths. Every validator returns a stable [`AppError`] with code
//! `"invalid_input"` and a safe, user-facing message — no technical internals leak.

use crate::errors::AppError;
use std::path::Path;

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

/// Validates a workspace root path before it is persisted/probed. A NUL byte is rejected
/// because it silently truncates the effective path in several OS filesystem APIs (a path
/// with a NUL is not a valid Rust `&str`-derived filesystem path assumption on any supported
/// platform), which could make the path Ark *validates* differ from the path Ark actually
/// *uses*.
pub fn validate_workspace_path(raw_path: &str) -> Result<&str, AppError> {
    let trimmed = raw_path.trim();
    if trimmed.is_empty() {
        return Err(AppError::invalid_input("Workspace path cannot be empty."));
    }
    if trimmed.contains('\0') {
        return Err(AppError::invalid_input(
            "Workspace path must not contain a null byte.",
        ));
    }
    if !Path::new(trimmed).is_absolute() {
        return Err(AppError::invalid_input("Workspace path must be absolute."));
    }
    reject_ambiguous_path(Path::new(trimmed), "Workspace path")?;

    Ok(trimmed)
}

/// Validates a built-in-runtime model file path before it is passed to `llama-server` as a
/// CLI argument. Existence/type/extension checks turn "the process failed to start for an
/// opaque reason 30 seconds later" into an immediate, specific error.
pub fn validate_model_path(raw_path: &str) -> Result<&str, AppError> {
    let trimmed = raw_path.trim();
    if trimmed.is_empty() {
        return Err(AppError::invalid_input("Model file path cannot be empty."));
    }
    if trimmed.contains('\0') {
        return Err(AppError::invalid_input(
            "Model file path must not contain a null byte.",
        ));
    }

    let path = Path::new(trimmed);
    reject_ambiguous_path(path, "Model file path")?;
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

    if !path.exists() {
        return Err(AppError::invalid_input(
            "Model file was not found at the given path.",
        ));
    }
    if !path.is_file() {
        return Err(AppError::invalid_input(
            "Model file path must point to a file, not a directory.",
        ));
    }

    Ok(trimmed)
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
        #[cfg(windows)]
        assert!(validate_workspace_path("C:\\Users\\me\\..\\other").is_err());
        #[cfg(not(windows))]
        assert!(validate_workspace_path("/home/me/../other").is_err());
    }

    #[test]
    fn workspace_path_accepts_absolute_paths_and_trims_whitespace() {
        #[cfg(windows)]
        let accepted = validate_workspace_path("  C:\\Users\\me\\ArkWorkspace  ").unwrap();
        #[cfg(windows)]
        assert_eq!(accepted, "C:\\Users\\me\\ArkWorkspace");

        #[cfg(not(windows))]
        let accepted = validate_workspace_path("  /home/me/ark-workspace  ").unwrap();
        #[cfg(not(windows))]
        assert_eq!(accepted, "/home/me/ark-workspace");
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
        let error = validate_model_path("/definitely/does/not/exist/model.GGUF").unwrap_err();
        assert!(
            error.message.contains("not found"),
            "must fail on existence, not extension, for a valid extension"
        );
    }

    #[test]
    fn model_path_rejects_a_nonexistent_file() {
        let error = validate_model_path("/definitely/does/not/exist/model.gguf").unwrap_err();
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
        assert!(error.message.contains("not a directory"));

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
        assert_eq!(accepted, path_str);

        let _ = std::fs::remove_file(&path);
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
}
