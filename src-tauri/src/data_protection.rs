//! SEC-006: optional SQLCipher workspace encryption with OS-protected keys.
//!
//! Protection metadata contains opaque key references only. Every plaintext/encrypted mode
//! change exports into a sibling database, verifies it independently, then atomically swaps it
//! into place. The transition journal lets startup determine whether a crash occurred before or
//! after the swap and finalize or roll back without guessing.

use crate::db::Database;
use crate::errors::AppError;
use crate::secret_store;
use crate::AppState;
use rusqlite::{params, Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use zeroize::{Zeroize, Zeroizing};

const METADATA_FILE: &str = "ark-protection.json";
const RECOVERY_PREFIX: &str = "ark-recovery-v1:";
const KEY_HEX_LENGTH: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum WorkspaceProtectionMode {
    Plaintext,
    Encrypted,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceProtectionStatus {
    pub mode: WorkspaceProtectionMode,
    pub locked: bool,
    pub transition_in_progress: bool,
    pub key_available: bool,
    pub message: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceProtectionChange {
    pub status: WorkspaceProtectionStatus,
    /// Returned exactly once after enable/rotation. It is never persisted outside the OS store.
    pub recovery_key: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProtectionMetadata {
    schema_version: u32,
    mode: WorkspaceProtectionMode,
    active_key_ref: Option<String>,
    pending_mode: Option<WorkspaceProtectionMode>,
    pending_key_ref: Option<String>,
}

impl ProtectionMetadata {
    fn encrypted(active_key_ref: String) -> Self {
        Self {
            schema_version: 1,
            mode: WorkspaceProtectionMode::Encrypted,
            active_key_ref: Some(active_key_ref),
            pending_mode: None,
            pending_key_ref: None,
        }
    }
}

struct MaintenanceGuard<'a>(&'a AppState);

impl Drop for MaintenanceGuard<'_> {
    fn drop(&mut self) {
        self.0.storage_maintenance.store(false, Ordering::Release);
    }
}

fn begin_maintenance(state: &AppState) -> Result<MaintenanceGuard<'_>, AppError> {
    state
        .storage_maintenance
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .map_err(|_| {
            AppError::new(
                "workspace_maintenance_busy",
                "Another workspace protection operation is already running.",
            )
        })?;
    let guard = MaintenanceGuard(state);
    let streams_active = !state
        .active_streams
        .lock()
        .map_err(|_| AppError::new("state_error", "Could not inspect active generations."))?
        .is_empty();
    let imports_active = !state
        .active_imports
        .lock()
        .map_err(|_| AppError::new("state_error", "Could not inspect active imports."))?
        .is_empty();
    if streams_active || imports_active {
        return Err(AppError::new(
            "workspace_maintenance_busy",
            "Finish or cancel active generations and imports before changing workspace protection.",
        ));
    }
    Ok(guard)
}

fn current_database_path(state: &AppState) -> Result<PathBuf, AppError> {
    let open_error = state
        .workspace_open_error
        .lock()
        .map_err(|_| AppError::new("state_error", "Could not inspect workspace state."))?;
    if let Some(error) = open_error.as_ref() {
        return Err(AppError::new(
            "workspace_not_open",
            format!(
                "Open or unlock the workspace before changing its protection: {}",
                error.message
            ),
        ));
    }
    drop(open_error);
    let path = state
        .workspace
        .lock()
        .map_err(|_| AppError::new("state_error", "Could not access workspace state."))?
        .database_path
        .clone();
    Ok(PathBuf::from(path))
}

fn metadata_path(database_path: &Path) -> Result<PathBuf, AppError> {
    database_path
        .parent()
        .map(|parent| parent.join(METADATA_FILE))
        .ok_or_else(|| {
            AppError::new(
                "workspace_error",
                "Workspace database has no parent folder.",
            )
        })
}

fn read_metadata(database_path: &Path) -> Result<Option<ProtectionMetadata>, AppError> {
    let path = metadata_path(database_path)?;
    if !path.exists() {
        return Ok(None);
    }
    let bytes = fs::read(&path)?;
    let metadata: ProtectionMetadata = serde_json::from_slice(&bytes).map_err(|_| {
        AppError::new(
            "workspace_protection_metadata_invalid",
            "Workspace protection metadata is invalid. Ark left the workspace untouched; restore the metadata from backup.",
        )
    })?;
    if metadata.schema_version != 1 {
        return Err(AppError::new(
            "workspace_protection_version_unsupported",
            "This workspace uses a newer protection metadata version. Open it with a newer Ark build.",
        ));
    }
    match metadata.mode {
        WorkspaceProtectionMode::Encrypted if metadata.active_key_ref.is_none() => {
            Err(AppError::new(
                "workspace_protection_metadata_invalid",
                "Encrypted workspace metadata is missing its opaque key reference.",
            ))
        }
        WorkspaceProtectionMode::Plaintext if metadata.active_key_ref.is_some() => {
            Err(AppError::new(
                "workspace_protection_metadata_invalid",
                "Plaintext workspace metadata unexpectedly contains an active key reference.",
            ))
        }
        _ => Ok(Some(metadata)),
    }
}

fn write_metadata(database_path: &Path, metadata: &ProtectionMetadata) -> Result<(), AppError> {
    let path = metadata_path(database_path)?;
    let next = path.with_extension("json.next");
    let previous = path.with_extension("json.previous");
    if next.exists() || previous.exists() {
        return Err(AppError::new(
            "workspace_protection_transition_interrupted",
            "Workspace protection journal files already exist. Reopen Ark so it can reconcile the interrupted operation.",
        ));
    }
    let bytes = serde_json::to_vec_pretty(metadata).map_err(|error| {
        AppError::new(
            "workspace_protection_metadata_invalid",
            format!("Could not serialize workspace protection metadata: {error}"),
        )
    })?;
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&next)?;
    crate::file_permissions::harden_file(&next)?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    drop(file);
    if path.exists() {
        fs::rename(&path, &previous)?;
    }
    if let Err(error) = fs::rename(&next, &path) {
        if previous.exists() {
            fs::rename(&previous, &path)?;
        }
        return Err(AppError::new(
            "workspace_protection_transition_interrupted",
            format!("Could not commit workspace protection metadata: {error}"),
        ));
    }
    if previous.exists() {
        fs::remove_file(previous)?;
    }
    Ok(())
}

fn remove_metadata(database_path: &Path) -> Result<(), AppError> {
    let path = metadata_path(database_path)?;
    if path.exists() {
        fs::remove_file(path)?;
    }
    Ok(())
}

fn generate_key() -> Result<Zeroizing<String>, AppError> {
    let mut bytes = Zeroizing::new([0u8; 32]);
    getrandom::fill(bytes.as_mut()).map_err(|_| {
        AppError::new(
            "workspace_key_generation_failed",
            "The operating system could not provide secure random bytes for a workspace key.",
        )
    })?;
    let mut encoded = String::with_capacity(KEY_HEX_LENGTH);
    for byte in bytes.iter() {
        use std::fmt::Write as _;
        write!(&mut encoded, "{byte:02x}").map_err(|_| {
            AppError::new(
                "workspace_key_generation_failed",
                "Could not encode workspace key.",
            )
        })?;
    }
    Ok(Zeroizing::new(encoded))
}

fn recovery_key(key: &str) -> String {
    format!("{RECOVERY_PREFIX}{key}")
}

fn parse_recovery_key(value: String) -> Result<Zeroizing<String>, AppError> {
    let mut value = Zeroizing::new(value);
    let key = value
        .strip_prefix(RECOVERY_PREFIX)
        .ok_or_else(|| AppError::invalid_input("Recovery key must start with ark-recovery-v1:."))?;
    if key.len() != KEY_HEX_LENGTH || !key.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(AppError::invalid_input(
            "Recovery key contains an invalid workspace key payload.",
        ));
    }
    let normalized = Zeroizing::new(key.to_ascii_lowercase());
    value.zeroize();
    Ok(normalized)
}

fn read_mode_key(
    mode: WorkspaceProtectionMode,
    reference: Option<&str>,
) -> Result<Option<Zeroizing<String>>, AppError> {
    match mode {
        WorkspaceProtectionMode::Plaintext => Ok(None),
        WorkspaceProtectionMode::Encrypted => {
            let reference = reference.ok_or_else(|| {
                AppError::new(
                    "workspace_protection_metadata_invalid",
                    "Encrypted workspace metadata is missing its key reference.",
                )
            })?;
            secret_store::read_workspace_key(reference).map(Some).map_err(|_| {
                AppError::new(
                    "workspace_locked",
                    "The encrypted workspace key is unavailable. Unlock the operating-system credential store or restore the recovery key; Ark will not reset or overwrite the workspace.",
                )
            })
        }
    }
}

fn database_accepts_key(path: &Path, key: Option<&str>) -> bool {
    path.exists() && Database::open_read_replica_with_key(path, key).is_ok()
}

/// Called before every real workspace open. If a crash left a transition journal, the database
/// itself is authoritative: whichever old/new key can read the schema determines rollback or
/// finalization. No write is attempted until one side is proven readable.
pub(crate) fn key_for_database_open(
    database_path: &Path,
) -> Result<Option<Zeroizing<String>>, AppError> {
    if database_path == Path::new(":memory:") {
        return Ok(None);
    }
    let Some(mut metadata) = read_metadata(database_path)? else {
        return Ok(None);
    };
    let Some(pending_mode) = metadata.pending_mode else {
        return read_mode_key(metadata.mode, metadata.active_key_ref.as_deref());
    };
    let intended_key = read_mode_key(pending_mode, metadata.pending_key_ref.as_deref())
        .ok()
        .flatten();
    if let Some(ref key) = intended_key {
        if database_accepts_key(database_path, Some(key.as_str())) {
            let old_reference = metadata.active_key_ref.take();
            metadata.mode = pending_mode;
            metadata.active_key_ref = metadata.pending_key_ref.take();
            metadata.pending_mode = None;
            if pending_mode == WorkspaceProtectionMode::Plaintext {
                remove_metadata(database_path)?;
            } else {
                write_metadata(database_path, &metadata)?;
            }
            if let Some(reference) = old_reference {
                secret_store::delete_workspace_key(&reference)?;
            }
            return Ok(intended_key);
        }
    } else if pending_mode == WorkspaceProtectionMode::Plaintext
        && database_accepts_key(database_path, None)
    {
        let old_reference = metadata.active_key_ref.take();
        remove_metadata(database_path)?;
        if let Some(reference) = old_reference {
            secret_store::delete_workspace_key(&reference)?;
        }
        return Ok(None);
    }

    let current_key = read_mode_key(metadata.mode, metadata.active_key_ref.as_deref())?;
    if database_accepts_key(database_path, current_key.as_deref().map(String::as_str)) {
        if let Some(reference) = metadata.pending_key_ref.take() {
            secret_store::delete_workspace_key(&reference)?;
        }
        metadata.pending_mode = None;
        write_metadata(database_path, &metadata)?;
        return Ok(current_key);
    }
    Err(AppError::new(
        "workspace_locked",
        "Neither the current nor pending workspace key can unlock this database. Restore a matching backup and recovery key; Ark left every file untouched.",
    ))
}

pub fn get_status(state: &AppState) -> Result<WorkspaceProtectionStatus, AppError> {
    let database_path = PathBuf::from(
        state
            .workspace
            .lock()
            .map_err(|_| AppError::new("state_error", "Could not access workspace state."))?
            .database_path
            .clone(),
    );
    status_for_path(&database_path)
}

fn status_for_path(database_path: &Path) -> Result<WorkspaceProtectionStatus, AppError> {
    let Some(metadata) = read_metadata(database_path)? else {
        return Ok(WorkspaceProtectionStatus {
            mode: WorkspaceProtectionMode::Plaintext,
            locked: false,
            transition_in_progress: false,
            key_available: false,
            message: "Workspace database is plaintext. Access relies on this OS account and full-disk encryption.".to_string(),
        });
    };
    let key_available = metadata
        .active_key_ref
        .as_deref()
        .and_then(|reference| secret_store::read_workspace_key(reference).ok())
        .is_some();
    Ok(WorkspaceProtectionStatus {
        mode: metadata.mode,
        locked: metadata.mode == WorkspaceProtectionMode::Encrypted && !key_available,
        transition_in_progress: metadata.pending_mode.is_some(),
        key_available,
        message: if key_available {
            "Workspace is encrypted with SQLCipher; its key is protected by the operating-system credential store.".to_string()
        } else {
            "The encrypted workspace key is unavailable. Unlock the OS credential store or restore the recovery key; Ark cannot reset forgotten keys.".to_string()
        },
    })
}

pub fn enable_encryption(state: &AppState) -> Result<WorkspaceProtectionChange, AppError> {
    let _maintenance = begin_maintenance(state)?;
    let database_path = current_database_path(state)?;
    if read_metadata(&database_path)?.is_some() {
        return Err(AppError::new(
            "workspace_already_encrypted",
            "This workspace is already encrypted or has an interrupted protection transition.",
        ));
    }
    let key = generate_key()?;
    let reference = secret_store::new_workspace_key_reference();
    secret_store::store_workspace_key(&reference, key.as_str())?;
    let transition = ProtectionMetadata {
        schema_version: 1,
        mode: WorkspaceProtectionMode::Plaintext,
        active_key_ref: None,
        pending_mode: Some(WorkspaceProtectionMode::Encrypted),
        pending_key_ref: Some(reference.clone()),
    };
    if let Err(error) = write_metadata(&database_path, &transition) {
        secret_store::delete_workspace_key(&reference)?;
        return Err(error);
    }
    migrate_and_reopen(state, &database_path, None, Some(key.as_str()))?;
    write_metadata(&database_path, &ProtectionMetadata::encrypted(reference))?;
    Ok(WorkspaceProtectionChange {
        status: status_for_path(&database_path)?,
        recovery_key: Some(recovery_key(key.as_str())),
    })
}

pub fn rotate_key(state: &AppState) -> Result<WorkspaceProtectionChange, AppError> {
    let _maintenance = begin_maintenance(state)?;
    let database_path = current_database_path(state)?;
    let mut metadata = read_metadata(&database_path)?.ok_or_else(|| {
        AppError::new(
            "workspace_not_encrypted",
            "Encrypt the workspace before rotating its key.",
        )
    })?;
    if metadata.mode != WorkspaceProtectionMode::Encrypted || metadata.pending_mode.is_some() {
        return Err(AppError::new(
            "workspace_protection_transition_interrupted",
            "Reopen Ark to reconcile the existing workspace protection transition before rotating its key.",
        ));
    }
    let old_reference = metadata.active_key_ref.clone().ok_or_else(|| {
        AppError::new(
            "workspace_protection_metadata_invalid",
            "Active key reference is missing.",
        )
    })?;
    let old_key = secret_store::read_workspace_key(&old_reference)?;
    let new_key = generate_key()?;
    let new_reference = secret_store::new_workspace_key_reference();
    secret_store::store_workspace_key(&new_reference, new_key.as_str())?;
    metadata.pending_mode = Some(WorkspaceProtectionMode::Encrypted);
    metadata.pending_key_ref = Some(new_reference.clone());
    write_metadata(&database_path, &metadata)?;
    migrate_and_reopen(
        state,
        &database_path,
        Some(old_key.as_str()),
        Some(new_key.as_str()),
    )?;
    write_metadata(
        &database_path,
        &ProtectionMetadata::encrypted(new_reference),
    )?;
    secret_store::delete_workspace_key(&old_reference)?;
    Ok(WorkspaceProtectionChange {
        status: status_for_path(&database_path)?,
        recovery_key: Some(recovery_key(new_key.as_str())),
    })
}

pub fn disable_encryption(state: &AppState) -> Result<WorkspaceProtectionStatus, AppError> {
    let _maintenance = begin_maintenance(state)?;
    let database_path = current_database_path(state)?;
    let mut metadata = read_metadata(&database_path)?.ok_or_else(|| {
        AppError::new(
            "workspace_not_encrypted",
            "This workspace is already plaintext.",
        )
    })?;
    if metadata.mode != WorkspaceProtectionMode::Encrypted || metadata.pending_mode.is_some() {
        return Err(AppError::new(
            "workspace_protection_transition_interrupted",
            "Reopen Ark to reconcile the existing workspace protection transition before decrypting it.",
        ));
    }
    let reference = metadata.active_key_ref.clone().ok_or_else(|| {
        AppError::new(
            "workspace_protection_metadata_invalid",
            "Active key reference is missing.",
        )
    })?;
    let key = secret_store::read_workspace_key(&reference)?;
    metadata.pending_mode = Some(WorkspaceProtectionMode::Plaintext);
    metadata.pending_key_ref = None;
    write_metadata(&database_path, &metadata)?;
    migrate_and_reopen(state, &database_path, Some(key.as_str()), None)?;
    remove_metadata(&database_path)?;
    secret_store::delete_workspace_key(&reference)?;
    status_for_path(&database_path)
}

pub fn restore_recovery_key(
    state: &AppState,
    recovery: String,
) -> Result<WorkspaceProtectionStatus, AppError> {
    let _maintenance = begin_maintenance(state)?;
    let database_path = PathBuf::from(
        state
            .workspace
            .lock()
            .map_err(|_| AppError::new("state_error", "Could not access workspace state."))?
            .database_path
            .clone(),
    );
    let metadata = read_metadata(&database_path)?.ok_or_else(|| {
        AppError::new(
            "workspace_not_encrypted",
            "This workspace does not require a recovery key.",
        )
    })?;
    let reference = metadata.active_key_ref.ok_or_else(|| {
        AppError::new(
            "workspace_protection_metadata_invalid",
            "Active key reference is missing.",
        )
    })?;
    let key = parse_recovery_key(recovery)?;
    if !database_accepts_key(&database_path, Some(key.as_str())) {
        return Err(AppError::new(
            "workspace_recovery_key_invalid",
            "That recovery key does not unlock this workspace. Ark left the database and credential store untouched.",
        ));
    }
    secret_store::store_workspace_key(&reference, key.as_str())?;
    let (db, read_db) = crate::open_database_pair_with_key(&database_path, Some(key.as_str()))?;
    *state
        .db
        .lock()
        .map_err(|_| AppError::new("state_error", "Could not replace workspace database."))? = db;
    *state
        .read_db
        .lock()
        .map_err(|_| AppError::new("state_error", "Could not replace workspace reader."))? =
        read_db;
    *state
        .workspace_open_error
        .lock()
        .map_err(|_| AppError::new("state_error", "Could not update workspace state."))? = None;
    status_for_path(&database_path)
}

fn migrate_and_reopen(
    state: &AppState,
    database_path: &Path,
    source_key: Option<&str>,
    target_key: Option<&str>,
) -> Result<(), AppError> {
    let mut writer = state
        .db
        .lock()
        .map_err(|_| AppError::new("state_error", "Could not lock workspace database."))?;
    let mut reader = state
        .read_db
        .lock()
        .map_err(|_| AppError::new("state_error", "Could not lock workspace reader."))?;
    writer.checkpoint()?;
    reader.disconnect()?;
    writer.disconnect()?;
    if let Err(error) = copy_verify_swap(database_path, source_key, target_key) {
        if let Ok((restored_writer, restored_reader)) =
            crate::open_database_pair_with_key(database_path, source_key)
        {
            *writer = restored_writer;
            *reader = restored_reader;
        }
        return Err(error);
    }
    let (next_writer, next_reader) = crate::open_database_pair_with_key(database_path, target_key)?;
    *writer = next_writer;
    *reader = next_reader;
    Ok(())
}

fn copy_verify_swap(
    database_path: &Path,
    source_key: Option<&str>,
    target_key: Option<&str>,
) -> Result<(), AppError> {
    let staged = database_path.with_extension("sqlite3.protection-next");
    let previous = database_path.with_extension("sqlite3.protection-previous");
    if staged.exists() || previous.exists() {
        return Err(AppError::new(
            "workspace_protection_transition_interrupted",
            "Database files from an interrupted protection migration exist. Ark did not overwrite them; reopen or restore the workspace first.",
        ));
    }
    let source = open_sqlcipher_connection(database_path, source_key, false)?;
    source
        .execute(
            "ATTACH DATABASE ?1 AS migrated KEY ?2",
            params![staged.to_string_lossy(), target_key.unwrap_or("")],
        )
        .map_err(migration_error)?;
    let export_result = source.query_row("SELECT sqlcipher_export('migrated')", [], |_| Ok(()));
    let detach_result = source.execute_batch("DETACH DATABASE migrated");
    export_result.map_err(migration_error)?;
    detach_result.map_err(migration_error)?;
    crate::file_permissions::harden_file(&staged)?;
    verify_copy(database_path, source_key, &staged, target_key)?;
    drop(source);
    remove_sqlite_sidecars(database_path)?;

    fs::rename(database_path, &previous)?;
    if let Err(error) = fs::rename(&staged, database_path) {
        fs::rename(&previous, database_path)?;
        return Err(AppError::new(
            "workspace_protection_migration_failed",
            format!("Could not install the verified workspace copy: {error}. The original was restored."),
        ));
    }
    if let Err(error) = verify_database(database_path, target_key) {
        let failed = database_path.with_extension(format!(
            "sqlite3.protection-failed-{}",
            uuid::Uuid::new_v4()
        ));
        fs::rename(database_path, &failed)?;
        fs::rename(&previous, database_path)?;
        return Err(AppError::new(
            "workspace_protection_migration_failed",
            format!(
                "The installed workspace copy failed verification ({error}). The original was restored; the failed copy was preserved at {}.",
                failed.display()
            ),
        ));
    }
    fs::remove_file(previous)?;
    crate::file_permissions::harden_file(database_path)?;
    Ok(())
}

fn open_sqlcipher_connection(
    path: &Path,
    key: Option<&str>,
    read_only: bool,
) -> Result<Connection, AppError> {
    let flags = if read_only {
        OpenFlags::SQLITE_OPEN_READ_ONLY
    } else {
        // CREATE is also inherited by `ATTACH DATABASE`; without it SQLCipher can open the
        // existing source but cannot create the staged export file.
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE
    };
    let connection = Connection::open_with_flags(path, flags)?;
    if let Some(key) = key {
        connection
            .pragma_update(None, "key", key)
            .map_err(migration_error)?;
    }
    connection
        .query_row("SELECT count(*) FROM sqlite_master", [], |row| {
            row.get::<_, i64>(0)
        })
        .map_err(migration_error)?;
    Ok(connection)
}

fn verify_database(path: &Path, key: Option<&str>) -> Result<(), AppError> {
    let connection = open_sqlcipher_connection(path, key, true)?;
    let integrity: String = connection
        .pragma_query_value(None, "integrity_check", |row| row.get(0))
        .map_err(migration_error)?;
    if integrity != "ok" {
        return Err(AppError::new(
            "workspace_protection_verification_failed",
            format!("SQLite integrity check returned {integrity}."),
        ));
    }
    if key.is_some() {
        let mut statement = connection
            .prepare("PRAGMA cipher_integrity_check")
            .map_err(migration_error)?;
        let failures: Vec<String> = statement
            .query_map([], |row| row.get(0))
            .map_err(migration_error)?
            .collect::<Result<_, _>>()
            .map_err(migration_error)?;
        if !failures.is_empty() {
            return Err(AppError::new(
                "workspace_protection_verification_failed",
                "SQLCipher authentication/integrity verification failed for the copied workspace.",
            ));
        }
    }
    Ok(())
}

fn verify_copy(
    source_path: &Path,
    source_key: Option<&str>,
    target_path: &Path,
    target_key: Option<&str>,
) -> Result<(), AppError> {
    verify_database(target_path, target_key)?;
    let source = open_sqlcipher_connection(source_path, source_key, true)?;
    let target = open_sqlcipher_connection(target_path, target_key, true)?;
    for table in [
        "schema_migrations",
        "conversations",
        "messages",
        "providers",
        "models",
        "settings",
    ] {
        let source_exists: bool = source
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
                [table],
                |row| row.get(0),
            )
            .map_err(migration_error)?;
        let target_exists: bool = target
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
                [table],
                |row| row.get(0),
            )
            .map_err(migration_error)?;
        if source_exists != target_exists {
            return Err(AppError::new(
                "workspace_protection_verification_failed",
                format!("Copied workspace schema differs for {table}. The original is untouched."),
            ));
        }
        if !source_exists {
            continue;
        }
        let sql = format!("SELECT count(*) FROM {table}");
        let source_count: i64 = source
            .query_row(&sql, [], |row| row.get(0))
            .map_err(migration_error)?;
        let target_count: i64 = target
            .query_row(&sql, [], |row| row.get(0))
            .map_err(migration_error)?;
        if source_count != target_count {
            return Err(AppError::new(
                "workspace_protection_verification_failed",
                format!(
                    "Copied workspace row count differs for {table}. The original is untouched."
                ),
            ));
        }
    }
    Ok(())
}

fn remove_sqlite_sidecars(path: &Path) -> Result<(), AppError> {
    let display = path.as_os_str().to_string_lossy();
    for suffix in ["-wal", "-shm"] {
        let sidecar = PathBuf::from(format!("{display}{suffix}"));
        if sidecar.exists() {
            fs::remove_file(sidecar)?;
        }
    }
    Ok(())
}

fn migration_error(error: rusqlite::Error) -> AppError {
    AppError::new(
        "workspace_protection_migration_failed",
        format!("SQLCipher could not copy or verify the workspace: {error}"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sidecar::SidecarState;
    use std::collections::HashMap;
    use std::sync::atomic::AtomicBool;
    use std::sync::{Arc, Mutex};

    fn temp_database(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "ark-protection-{name}-{}.sqlite3",
            uuid::Uuid::new_v4()
        ))
    }

    /// Builds a real `AppState` (not a mock) against a fresh temp workspace so
    /// `enable_encryption`/`rotate_key`/`restore_recovery_key` exercise their actual
    /// orchestration — metadata transitions, `db`/`read_db` swap, and the platform secret
    /// store — the same way `generation::tests::test_state` does for the generation lifecycle.
    fn test_state(path: &Path) -> AppState {
        let db = Database::open(path).expect("writer opens");
        let read_db = Database::open_read_replica(path).expect("read replica opens");
        AppState {
            db: Mutex::new(db),
            workspace: Mutex::new(crate::workspace::WorkspaceInfo {
                root_path: path
                    .parent()
                    .expect("test path parent")
                    .display()
                    .to_string(),
                database_path: path.display().to_string(),
                default_root_path: path
                    .parent()
                    .expect("test path parent")
                    .display()
                    .to_string(),
                config_path: path.with_extension("json").display().to_string(),
                is_portable: false,
                requires_restart: false,
            }),
            read_db: Mutex::new(read_db),
            workspace_open_error: Mutex::new(None),
            active_streams: Mutex::new(HashMap::new()),
            pending_streams: Mutex::new(HashMap::new()),
            active_imports: Mutex::new(HashMap::new()),
            storage_maintenance: AtomicBool::new(false),
            sidecar: Arc::new(Mutex::new(SidecarState::new())),
        }
    }

    /// Covers the acceptance-criteria surface the lower-level `copy_verify_swap`/
    /// `database_accepts_key` tests above do not reach: the full `AppState`-driven
    /// enable -> rotate -> forgotten-key restore flow, including that a stale (rotated-away)
    /// key and an invalid recovery key are both safely rejected without touching the database
    /// or credential store, and that data survives every transition.
    #[test]
    fn rotate_key_and_restore_recovery_key_round_trip() {
        let path = temp_database("rotate-restore");
        {
            let seed = Database::open(&path).expect("create plaintext database");
            seed.create_conversation(Some("survives rotation".to_string()))
                .expect("seed row");
            seed.checkpoint().expect("checkpoint");
        }

        let state = test_state(&path);
        let enabled = enable_encryption(&state).expect("enable encryption");
        assert_eq!(enabled.status.mode, WorkspaceProtectionMode::Encrypted);
        let first_recovery_key = enabled.recovery_key.expect("recovery key issued on enable");

        let rotated = rotate_key(&state).expect("rotate key");
        assert_eq!(rotated.status.mode, WorkspaceProtectionMode::Encrypted);
        assert!(!rotated.status.locked);
        let second_recovery_key = rotated
            .recovery_key
            .expect("recovery key issued on rotation");
        assert_ne!(
            first_recovery_key, second_recovery_key,
            "rotation must issue a fresh recovery key, not repeat the previous one"
        );

        // Data survived the encrypt-then-rotate copy/verify/swap sequence.
        {
            let reopened = state.db.lock().expect("lock db");
            assert_eq!(conversation_count(&reopened), 1);
        }

        // The pre-rotation recovery key must no longer unlock the workspace.
        let stale_attempt = restore_recovery_key(&state, first_recovery_key);
        assert!(
            stale_attempt.is_err(),
            "a rotated-away recovery key must be rejected"
        );
        assert_eq!(
            stale_attempt.unwrap_err().code,
            "workspace_recovery_key_invalid"
        );

        // A syntactically invalid recovery key is rejected the same safe way.
        let invalid_attempt = restore_recovery_key(&state, "not-a-recovery-key".to_string());
        assert!(invalid_attempt.is_err());

        // The current recovery key restores access (the "forgotten key" / relock path).
        let restored =
            restore_recovery_key(&state, second_recovery_key).expect("restore with current key");
        assert_eq!(restored.mode, WorkspaceProtectionMode::Encrypted);
        assert!(!restored.locked);
        {
            let reopened = state.db.lock().expect("lock db");
            assert_eq!(conversation_count(&reopened), 1);
        }

        let database_path = PathBuf::from(
            state
                .workspace
                .lock()
                .expect("lock workspace")
                .database_path
                .clone(),
        );
        drop(state);

        // Leave no trace in the real OS credential store: remove the final active key entry
        // this test created, the same way `disable_encryption` would on a real teardown.
        if let Ok(Some(metadata)) = read_metadata(&database_path) {
            if let Some(reference) = metadata.active_key_ref {
                let _ = secret_store::delete_workspace_key(&reference);
            }
        }
        let _ = remove_metadata(&database_path);
        let _ = fs::remove_file(&database_path);
    }

    fn conversation_count(database: &Database) -> usize {
        database
            .list_conversations_page(&crate::chat::ConversationListRequest {
                limit: None,
                cursor: None,
                query: None,
                archived: None,
                project_id: None,
            })
            .expect("list conversations")
            .items
            .len()
    }

    #[test]
    fn recovery_keys_are_strictly_versioned_and_normalized() {
        let key = "A".repeat(KEY_HEX_LENGTH);
        let parsed = parse_recovery_key(format!("{RECOVERY_PREFIX}{key}")).expect("valid key");
        assert_eq!(parsed.as_str(), "a".repeat(KEY_HEX_LENGTH));
        assert!(parse_recovery_key("wrong".to_string()).is_err());
        assert!(parse_recovery_key(format!("{RECOVERY_PREFIX}{}", "z".repeat(64))).is_err());
    }

    #[test]
    fn plaintext_to_encrypted_and_back_is_copy_based_and_preserves_rows() {
        let path = temp_database("round-trip");
        let db = Database::open(&path).expect("create plaintext database");
        db.create_conversation(Some("preserved".to_string()))
            .expect("seed row");
        db.checkpoint().expect("checkpoint");
        drop(db);
        let key = "0123456789abcdef".repeat(4);

        copy_verify_swap(&path, None, Some(&key)).expect("encrypt copy");
        assert!(Database::open_read_replica(&path).is_err());
        let encrypted = Database::open_with_key(&path, Some(&key)).expect("open encrypted");
        assert_eq!(conversation_count(&encrypted), 1);
        encrypted.checkpoint().expect("checkpoint encrypted");
        drop(encrypted);

        copy_verify_swap(&path, Some(&key), None).expect("decrypt copy");
        let plaintext = Database::open(&path).expect("open plaintext");
        assert_eq!(conversation_count(&plaintext), 1);
        drop(plaintext);
        fs::remove_file(path).expect("remove database");
    }

    #[test]
    fn wrong_key_cannot_open_encrypted_database() {
        let path = temp_database("wrong-key");
        let db = Database::open(&path).expect("create plaintext database");
        db.checkpoint().expect("checkpoint");
        drop(db);
        let key = "a".repeat(KEY_HEX_LENGTH);
        copy_verify_swap(&path, None, Some(&key)).expect("encrypt");
        assert!(!database_accepts_key(
            &path,
            Some(&"b".repeat(KEY_HEX_LENGTH))
        ));
        assert!(database_accepts_key(&path, Some(&key)));
        fs::remove_file(path).expect("remove database");
    }
}
