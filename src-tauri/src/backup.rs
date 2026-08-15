//! FTR-001: verified backup, restore, and workspace-copy migration.
//!
//! Three independent operations, each reusing the same safety pattern already established by
//! `data_protection.rs`'s SEC-006 protection-mode changes and `db::backup_before_migrations`:
//! checkpoint the live connection, snapshot via SQLite's Online Backup API (not a raw file copy,
//! which only produces a complete result if a checkpoint happens to fully drain the WAL at that
//! exact moment), then independently open and integrity-check the result before trusting it.
//!
//! - `create_backup`: snapshots the current workspace database plus a hash-manifested sidecar
//!   file, into a directory the caller chooses. Never touches the live workspace.
//! - `preview_restore`/`restore_backup`: read-only inspection, then a restore that always lands
//!   in a *new* workspace directory (the acceptance criteria's own stated safe default) — never
//!   overwrites or reopens the live connections in place. The user then uses the existing
//!   `set_workspace` flow (already `requires_restart: true`) to switch to it when ready.
//! - `copy_workspace_data`: used by workspace-change "copy" mode to seed a new workspace
//!   location with the current data before `set_workspace_root` repoints to it. Deliberately does
//!   *not* implement a "move" that deletes the original: the live app still holds that file open
//!   for the remainder of this session (a workspace change is not applied until restart), and
//!   deleting a database file while it's still open by the process is not reliably safe or
//!   consistent across Windows/macOS/Linux. "Copy, then delete the old location yourself once
//!   you've confirmed the new one works" is the documented alternative.

use crate::data_protection;
use crate::db;
use crate::errors::AppError;
use crate::AppState;
use rusqlite::{Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

const MANIFEST_SUFFIX: &str = ".ark-backup-manifest.json";
const DATABASE_FILE_NAME: &str = "ark.sqlite3";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupManifest {
    pub app_version: String,
    pub created_at: String,
    pub database_sha256: String,
    pub database_size_bytes: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupResult {
    pub backup_path: String,
    pub manifest: BackupManifest,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RestorePreview {
    /// `None` for a backup file that predates this manifest format, or any plain `.sqlite3` file
    /// the user points Ark at directly — the rest of the preview still works without it.
    pub manifest: Option<BackupManifest>,
    pub detected_schema_version: i64,
    pub schema_supported: bool,
    pub conversation_count: i64,
    pub message_count: i64,
}

fn manifest_path(backup_path: &Path) -> PathBuf {
    let mut file_name = backup_path.file_name().unwrap_or_default().to_os_string();
    file_name.push(MANIFEST_SUFFIX);
    backup_path.with_file_name(file_name)
}

fn sha256_file(path: &Path) -> Result<(u64, String), AppError> {
    let mut file = fs::File::open(path).map_err(|error| {
        AppError::new(
            "backup_hash_failed",
            format!("Could not read {} to hash it: {error}.", path.display()),
        )
    })?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; 1024 * 1024];
    let mut total_bytes = 0_u64;
    loop {
        let count = file.read(&mut buffer).map_err(|error| {
            AppError::new(
                "backup_hash_failed",
                format!("Could not hash {}: {error}.", path.display()),
            )
        })?;
        if count == 0 {
            break;
        }
        total_bytes += count as u64;
        hasher.update(&buffer[..count]);
    }
    Ok((total_bytes, format!("{:x}", hasher.finalize())))
}

/// Snapshots the current workspace database into `destination_dir` (created if missing), plus a
/// hash-manifested `.ark-backup-manifest.json` sidecar. `destination_dir` must not already
/// contain an `ark.sqlite3` — this never overwrites an existing backup.
pub fn create_backup(state: &AppState, destination_dir: String) -> Result<BackupResult, AppError> {
    let destination_dir = PathBuf::from(crate::validation::validate_workspace_path(
        &destination_dir,
    )?);
    let backup_path = destination_dir.join(DATABASE_FILE_NAME);

    // Exclusivity only — this never disconnects or swaps the live connections (unlike SEC-006's
    // protection-mode changes, which begin_maintenance was originally built for), it just reads
    // a live snapshot through them.
    let _maintenance = data_protection::begin_maintenance(state)?;

    fs::create_dir_all(&destination_dir)?;
    crate::file_permissions::harden_directory(&destination_dir)?;

    {
        let writer = state
            .db
            .lock()
            .map_err(|_| AppError::new("state_error", "Could not access local database."))?;
        writer.create_verified_backup(&backup_path)?;
    }

    let (database_size_bytes, database_sha256) = sha256_file(&backup_path)?;
    let manifest = BackupManifest {
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        created_at: db::now(),
        database_sha256,
        database_size_bytes,
    };
    let manifest_json = serde_json::to_vec_pretty(&manifest).map_err(|error| {
        AppError::new(
            "backup_failed",
            format!("Backup succeeded but its manifest could not be written: {error}."),
        )
    })?;
    fs::write(manifest_path(&backup_path), manifest_json)?;
    crate::file_permissions::harden_file(&manifest_path(&backup_path))?;

    Ok(BackupResult {
        backup_path: backup_path.display().to_string(),
        manifest,
    })
}

/// Read-only: opens `backup_path`, checks its integrity, and reports what it contains, without
/// modifying anything — the live workspace is never touched by this function.
pub fn preview_restore(state: &AppState, backup_path: String) -> Result<RestorePreview, AppError> {
    let backup_path = PathBuf::from(crate::validation::validate_workspace_path(&backup_path)?);
    if !backup_path.is_file() {
        return Err(AppError::new(
            "backup_not_found",
            format!("{} is not a file.", backup_path.display()),
        ));
    }

    let key = restore_key_for(state)?;
    let connection = Connection::open_with_flags(&backup_path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|error| {
            AppError::new(
                "backup_open_failed",
                format!(
                    "Could not open {} as an Ark backup: {error}.",
                    backup_path.display()
                ),
            )
        })?;
    db::apply_encryption_key(&connection, key.as_deref().map(String::as_str))?;

    let integrity: String = connection
        .pragma_query_value(None, "integrity_check", |row| row.get(0))
        .map_err(AppError::from)?;
    if integrity.to_lowercase() != "ok" {
        return Err(AppError::new(
            "backup_verification_failed",
            format!(
                "{} failed its integrity check ({integrity}).",
                backup_path.display()
            ),
        ));
    }

    let detected_schema_version: i64 = connection
        .query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);
    let conversation_count: i64 = connection
        .query_row("SELECT COUNT(*) FROM conversations", [], |row| row.get(0))
        .map_err(AppError::from)?;
    let message_count: i64 = connection
        .query_row("SELECT COUNT(*) FROM messages", [], |row| row.get(0))
        .map_err(AppError::from)?;

    let manifest = fs::read(manifest_path(&backup_path))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<BackupManifest>(&bytes).ok());

    Ok(RestorePreview {
        manifest,
        detected_schema_version,
        schema_supported: detected_schema_version <= db::latest_schema_version(),
        conversation_count,
        message_count,
    })
}

/// Restores `backup_path` into a brand-new workspace directory at `target_root` — never the live
/// one. `target_root` must not already contain an `ark.sqlite3`. The live app is completely
/// unaffected; use the existing `set_workspace` flow afterward to actually switch to it.
pub fn restore_backup(
    state: &AppState,
    backup_path: String,
    target_root: String,
) -> Result<(), AppError> {
    let backup_path = PathBuf::from(crate::validation::validate_workspace_path(&backup_path)?);
    let target_root = PathBuf::from(crate::validation::validate_workspace_path(&target_root)?);
    let target_database_path = target_root.join(DATABASE_FILE_NAME);
    if target_database_path.exists() {
        return Err(AppError::new(
            "restore_destination_exists",
            format!(
                "{} already contains a workspace database. Choose an empty destination.",
                target_root.display()
            ),
        ));
    }

    // Validates integrity before anything is written to the destination — restore must not
    // "partially succeed" into a target that then looks populated but is actually broken.
    preview_restore(state, backup_path.display().to_string())?;

    fs::create_dir_all(&target_root)?;
    crate::file_permissions::harden_directory(&target_root)?;
    fs::copy(&backup_path, &target_database_path)?;
    crate::file_permissions::harden_file(&target_database_path)?;

    // Independently re-verifies the copy landed correctly, rather than trusting `fs::copy`'s
    // success return value alone — the same "don't trust it until you've reopened and checked
    // it" discipline `create_verified_backup`/`backup_before_migrations` already apply.
    let key = restore_key_for(state)?;
    let verify =
        Connection::open_with_flags(&target_database_path, OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(|error| {
                AppError::new(
                    "restore_verification_failed",
                    format!(
                        "Restored to {} but could not open it to verify it: {error}.",
                        target_database_path.display()
                    ),
                )
            })?;
    db::apply_encryption_key(&verify, key.as_deref().map(String::as_str))?;
    let integrity: String = verify
        .pragma_query_value(None, "integrity_check", |row| row.get(0))
        .map_err(AppError::from)?;
    if integrity.to_lowercase() != "ok" {
        drop(verify);
        let _ = fs::remove_dir_all(&target_root);
        return Err(AppError::new(
            "restore_verification_failed",
            format!(
                "The restored copy at {} failed its integrity check ({integrity}) and was removed.",
                target_database_path.display()
            ),
        ));
    }

    Ok(())
}

/// The live workspace's own encryption key, if any — a restored/verified backup is assumed to
/// use the same protection mode as the workspace it came from. Restoring an encrypted backup
/// into a workspace under different protection is out of scope for this pass.
fn restore_key_for(state: &AppState) -> Result<Option<zeroize::Zeroizing<String>>, AppError> {
    let path = data_protection::current_database_path(state)?;
    data_protection::key_for_database_open(&path)
}

/// Used by `workspace::set_workspace_root`'s `copy_data` flag to seed a new location with the
/// current workspace's data before repointing to it. See this module's own doc comment for why
/// there is no "move" variant that deletes the original.
pub fn copy_workspace_data(state: &AppState, target_root: &Path) -> Result<(), AppError> {
    let target_database_path = target_root.join(DATABASE_FILE_NAME);
    if target_database_path.exists() {
        return Err(AppError::new(
            "restore_destination_exists",
            format!(
                "{} already contains a workspace database.",
                target_root.display()
            ),
        ));
    }
    // Self-sufficient rather than relying on the caller (`set_workspace_root` already calls
    // `prepare_workspace_root` first, which happens to create this directory too) — a function
    // should not depend on an undocumented precondition only one caller happens to satisfy.
    fs::create_dir_all(target_root)?;
    crate::file_permissions::harden_directory(target_root)?;

    let _maintenance = data_protection::begin_maintenance(state)?;
    {
        let writer = state
            .db
            .lock()
            .map_err(|_| AppError::new("state_error", "Could not access local database."))?;
        writer.create_verified_backup(&target_database_path)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;
    use crate::sidecar::SidecarState;
    use crate::AppState;
    use std::collections::HashMap;
    use std::sync::atomic::AtomicBool;
    use std::sync::{Arc, Mutex};

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("ark-backup-{name}-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    fn test_list_request() -> crate::chat::ConversationListRequest {
        crate::chat::ConversationListRequest {
            limit: None,
            cursor: None,
            query: None,
            archived: None,
            project_id: None,
        }
    }

    /// Same pattern as `data_protection::tests::test_state`/`generation::tests::test_state`: a
    /// real `AppState` against a real temp-file database, not a mock, so `begin_maintenance`/
    /// `lock_db`/checkpoint all exercise their actual behavior.
    fn test_state(database_path: &Path) -> AppState {
        let db = Database::open(database_path).expect("writer opens");
        let read_db = Database::open_read_replica(database_path).expect("read replica opens");
        AppState {
            db: Mutex::new(db),
            workspace: Mutex::new(crate::workspace::WorkspaceInfo {
                root_path: database_path
                    .parent()
                    .expect("parent")
                    .display()
                    .to_string(),
                database_path: database_path.display().to_string(),
                default_root_path: database_path
                    .parent()
                    .expect("parent")
                    .display()
                    .to_string(),
                config_path: database_path.with_extension("json").display().to_string(),
                is_portable: false,
                requires_restart: false,
            }),
            read_db: Mutex::new(read_db),
            workspace_open_error: Mutex::new(None),
            active_streams: Mutex::new(HashMap::new()),
            pending_streams: Mutex::new(HashMap::new()),
            active_imports: Mutex::new(HashMap::new()),
            active_ollama_pulls: Mutex::new(HashMap::new()),
            storage_maintenance: AtomicBool::new(false),
            sidecar: Arc::new(Mutex::new(SidecarState::new())),
            observability_log: Arc::new(Mutex::new(crate::observability::DiagnosticsLog::new())),
        }
    }

    fn seeded_state(name: &str) -> (AppState, PathBuf) {
        let workspace_dir = temp_dir(name);
        let database_path = workspace_dir.join("ark.sqlite3");
        let state = test_state(&database_path);
        {
            let db = state.db.lock().expect("lock db");
            db.create_conversation(Some("seed conversation".to_string()))
                .expect("seed a conversation so backups have real data to preserve");
        }
        (state, workspace_dir)
    }

    #[test]
    fn create_backup_produces_a_verified_manifested_copy_with_a_matching_hash() {
        let (state, _workspace_dir) = seeded_state("create-ok");
        let destination = temp_dir("create-ok-dest");

        let result =
            create_backup(&state, destination.display().to_string()).expect("backup succeeds");

        let (actual_size, actual_hash) =
            sha256_file(Path::new(&result.backup_path)).expect("hash the backup");
        assert_eq!(result.manifest.database_size_bytes, actual_size);
        assert_eq!(result.manifest.database_sha256, actual_hash);
        assert!(!result.manifest.app_version.is_empty());

        // The backup is independently a real, readable Ark database with the seeded data.
        let backup_db = Database::open(Path::new(&result.backup_path)).expect("backup opens");
        assert_eq!(
            backup_db
                .list_conversations_page(&test_list_request())
                .unwrap()
                .items
                .len(),
            1
        );
    }

    #[test]
    fn create_backup_refuses_to_overwrite_an_existing_destination_and_leaves_it_untouched() {
        let (state, _workspace_dir) = seeded_state("create-exists");
        let destination = temp_dir("create-exists-dest");
        fs::write(
            destination.join(DATABASE_FILE_NAME),
            b"pre-existing sentinel",
        )
        .expect("seed existing file");

        let error = create_backup(&state, destination.display().to_string())
            .expect_err("must refuse to overwrite");
        assert_eq!(error.code, "backup_destination_exists");
        let preserved =
            fs::read(destination.join(DATABASE_FILE_NAME)).expect("destination still readable");
        assert_eq!(preserved, b"pre-existing sentinel");
    }

    /// Proxy for an interrupted/insufficient-space write failure: the destination directory
    /// cannot actually be created (a file already occupies that path component), so the backup
    /// fails partway through directory preparation — proving the *source* database is completely
    /// unaffected by a failed backup attempt, the acceptance-criteria property that matters, even
    /// though this doesn't literally fill a disk.
    #[test]
    fn create_backup_failure_never_touches_the_source_database() {
        let (state, workspace_dir) = seeded_state("create-fail-source");
        let blocking_file = temp_dir("create-fail-parent").join("blocked");
        fs::write(&blocking_file, b"not a directory").expect("seed a plain file");
        // Using the file itself as a directory path makes `create_dir_all` fail deterministically.
        let impossible_destination = blocking_file.join("nested");

        let error = create_backup(&state, impossible_destination.display().to_string())
            .expect_err("must fail");
        assert!(!error.code.is_empty());

        // The source workspace is still fully intact and readable with its original data.
        let source_path = workspace_dir.join("ark.sqlite3");
        let source_db = Database::open(&source_path).expect("source still opens");
        assert_eq!(
            source_db
                .list_conversations_page(&test_list_request())
                .unwrap()
                .items
                .len(),
            1
        );
    }

    #[test]
    fn preview_restore_reports_accurate_counts_without_touching_the_live_workspace() {
        let (state, workspace_dir) = seeded_state("preview-ok");
        let destination = temp_dir("preview-ok-dest");
        let result =
            create_backup(&state, destination.display().to_string()).expect("backup succeeds");

        let preview =
            preview_restore(&state, result.backup_path.clone()).expect("preview succeeds");
        assert_eq!(preview.conversation_count, 1);
        assert!(preview.schema_supported);
        assert_eq!(preview.detected_schema_version, db::latest_schema_version());
        assert_eq!(
            preview.manifest.expect("manifest present").database_sha256,
            result.manifest.database_sha256
        );

        // The live workspace is untouched — still openable with its original data.
        let source_path = workspace_dir.join("ark.sqlite3");
        let source_db = Database::open(&source_path).expect("source still opens");
        assert_eq!(
            source_db
                .list_conversations_page(&test_list_request())
                .unwrap()
                .items
                .len(),
            1
        );
    }

    #[test]
    fn preview_restore_rejects_a_corrupt_backup_file_without_touching_the_live_workspace() {
        let (state, workspace_dir) = seeded_state("preview-corrupt");
        let fake_backup = temp_dir("preview-corrupt-file").join(DATABASE_FILE_NAME);
        fs::write(&fake_backup, b"this is not a sqlite database").expect("write garbage");

        let error = preview_restore(&state, fake_backup.display().to_string())
            .expect_err("must reject garbage");
        assert!(!error.code.is_empty());

        let source_path = workspace_dir.join("ark.sqlite3");
        let source_db = Database::open(&source_path).expect("source still opens");
        assert_eq!(
            source_db
                .list_conversations_page(&test_list_request())
                .unwrap()
                .items
                .len(),
            1
        );
    }

    #[test]
    fn preview_restore_rejects_a_missing_file() {
        let (state, _workspace_dir) = seeded_state("preview-missing");
        let missing = temp_dir("preview-missing-dir").join("does-not-exist.sqlite3");
        let error = preview_restore(&state, missing.display().to_string())
            .expect_err("must reject a missing file");
        assert_eq!(error.code, "backup_not_found");
    }

    #[test]
    fn restore_backup_creates_an_independent_workspace_and_leaves_the_live_one_untouched() {
        let (state, workspace_dir) = seeded_state("restore-ok");
        let backup_dir = temp_dir("restore-ok-backup");
        let result =
            create_backup(&state, backup_dir.display().to_string()).expect("backup succeeds");

        let restore_target = temp_dir("restore-ok-target");
        // create_backup already made this a real directory with a database in it, but
        // restore_backup requires a *fresh* target — reuse the parent, not the same populated dir.
        let restore_target = restore_target.join("restored-workspace");
        restore_backup(
            &state,
            result.backup_path.clone(),
            restore_target.display().to_string(),
        )
        .expect("restore succeeds");

        let restored_db =
            Database::open(restore_target.join(DATABASE_FILE_NAME)).expect("restored copy opens");
        assert_eq!(
            restored_db
                .list_conversations_page(&test_list_request())
                .unwrap()
                .items
                .len(),
            1
        );

        // The live workspace this session has been using throughout is completely unaffected.
        let source_db =
            Database::open(workspace_dir.join("ark.sqlite3")).expect("source still opens");
        assert_eq!(
            source_db
                .list_conversations_page(&test_list_request())
                .unwrap()
                .items
                .len(),
            1
        );
    }

    #[test]
    fn restore_backup_refuses_an_existing_destination_and_leaves_it_untouched() {
        let (state, _workspace_dir) = seeded_state("restore-exists");
        let backup_dir = temp_dir("restore-exists-backup");
        let result =
            create_backup(&state, backup_dir.display().to_string()).expect("backup succeeds");

        let restore_target = temp_dir("restore-exists-target");
        fs::write(
            restore_target.join(DATABASE_FILE_NAME),
            b"pre-existing sentinel",
        )
        .expect("seed existing file");

        let error = restore_backup(
            &state,
            result.backup_path,
            restore_target.display().to_string(),
        )
        .expect_err("must refuse to overwrite");
        assert_eq!(error.code, "restore_destination_exists");
        let preserved =
            fs::read(restore_target.join(DATABASE_FILE_NAME)).expect("destination still readable");
        assert_eq!(preserved, b"pre-existing sentinel");
    }

    #[test]
    fn copy_workspace_data_seeds_a_new_location_and_preserves_the_original() {
        let (state, workspace_dir) = seeded_state("copy-ok");
        let target = temp_dir("copy-ok-target").join("new-workspace");

        copy_workspace_data(&state, &target).expect("copy succeeds");

        let copied_db = Database::open(target.join(DATABASE_FILE_NAME)).expect("copy opens");
        assert_eq!(
            copied_db
                .list_conversations_page(&test_list_request())
                .unwrap()
                .items
                .len(),
            1
        );

        let source_db =
            Database::open(workspace_dir.join("ark.sqlite3")).expect("source still opens");
        assert_eq!(
            source_db
                .list_conversations_page(&test_list_request())
                .unwrap()
                .items
                .len(),
            1
        );
    }

    #[test]
    fn copy_workspace_data_refuses_an_existing_destination() {
        let (state, _workspace_dir) = seeded_state("copy-exists");
        let target = temp_dir("copy-exists-target");
        fs::write(target.join(DATABASE_FILE_NAME), b"pre-existing sentinel")
            .expect("seed existing file");

        let error = copy_workspace_data(&state, &target).expect_err("must refuse to overwrite");
        assert_eq!(error.code, "restore_destination_exists");
    }
}
