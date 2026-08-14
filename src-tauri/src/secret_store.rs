use crate::errors::AppError;
use serde::Serialize;
#[cfg(test)]
use std::collections::HashMap;
#[cfg(test)]
use std::sync::{Mutex, MutexGuard};
use zeroize::Zeroize;

const PROVIDER_SERVICE: &str = "dev.ark.desktop.provider-secret";
const WORKSPACE_KEY_SERVICE: &str = "dev.ark.desktop.workspace-key";
const REFERENCE_PREFIX: &str = "secret:v1:";
const WORKSPACE_KEY_REFERENCE_PREFIX: &str = "workspace-key:v1:";
const MAX_SECRET_BYTES: usize = 16 * 1024;

pub struct SecretValue(String);

impl SecretValue {
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl Drop for SecretValue {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SecretStoreErrorKind {
    Unavailable,
    Locked,
    NotFound,
    Invalid,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecretStoreError {
    kind: SecretStoreErrorKind,
    message: String,
}

impl SecretStoreError {
    fn new(kind: SecretStoreErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }

    fn to_app_error(&self) -> AppError {
        let code = match self.kind {
            SecretStoreErrorKind::Unavailable => "secret_store_unavailable",
            SecretStoreErrorKind::Locked => "secret_store_locked",
            SecretStoreErrorKind::NotFound => "secret_not_found",
            SecretStoreErrorKind::Invalid => "secret_reference_invalid",
            SecretStoreErrorKind::Failed => "secret_store_failed",
        };
        AppError::new(code, self.message.clone())
    }
}

pub trait SecretStore: Send + Sync {
    fn status(&self) -> Result<(), SecretStoreError>;
    fn create(&self, reference: &str, value: SecretValue) -> Result<(), SecretStoreError>;
    fn read(&self, reference: &str) -> Result<SecretValue, SecretStoreError>;
    fn update(&self, reference: &str, value: SecretValue) -> Result<(), SecretStoreError>;
    fn delete(&self, reference: &str) -> Result<(), SecretStoreError>;
}

#[derive(Clone, Copy)]
pub struct SystemSecretStore;

impl SystemSecretStore {
    fn entry(reference: &str) -> Result<keyring::Entry, SecretStoreError> {
        let service = validate_reference(reference)?;
        keyring::Entry::new(service, reference).map_err(map_keyring_error)
    }
}

impl SecretStore for SystemSecretStore {
    fn status(&self) -> Result<(), SecretStoreError> {
        keyring::Entry::store_status()
            .as_ref()
            .map_err(map_keyring_error_ref)
            .copied()
    }

    fn create(&self, reference: &str, value: SecretValue) -> Result<(), SecretStoreError> {
        Self::entry(reference)?
            .set_password(value.expose())
            .map_err(map_keyring_error)
    }

    fn read(&self, reference: &str) -> Result<SecretValue, SecretStoreError> {
        Self::entry(reference)?
            .get_password()
            .map(SecretValue)
            .map_err(map_keyring_error)
    }

    fn update(&self, reference: &str, value: SecretValue) -> Result<(), SecretStoreError> {
        Self::entry(reference)?
            .set_password(value.expose())
            .map_err(map_keyring_error)
    }

    fn delete(&self, reference: &str) -> Result<(), SecretStoreError> {
        match Self::entry(reference)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(map_keyring_error(error)),
        }
    }
}

fn map_keyring_error(error: keyring::Error) -> SecretStoreError {
    match error {
        keyring::Error::NoStorageAccess(_) => SecretStoreError::new(
            SecretStoreErrorKind::Locked,
            "The operating-system credential store is locked or access was denied. Unlock it and retry.",
        ),
        keyring::Error::NoEntry => SecretStoreError::new(
            SecretStoreErrorKind::NotFound,
            "The credential is no longer present. Reconnect this provider.",
        ),
        keyring::Error::NoDefaultStore
        | keyring::Error::NotSupportedByStore(_)
        | keyring::Error::PlatformFailure(_) => SecretStoreError::new(
            SecretStoreErrorKind::Unavailable,
            "The operating-system credential store is unavailable. Start or unlock the platform keychain service and retry.",
        ),
        keyring::Error::Invalid(_, _) | keyring::Error::TooLong(_, _) => SecretStoreError::new(
            SecretStoreErrorKind::Invalid,
            "The operating-system credential store rejected Ark's opaque credential identifier.",
        ),
        _ => SecretStoreError::new(
            SecretStoreErrorKind::Failed,
            "The operating-system credential store could not complete the request.",
        ),
    }
}

fn map_keyring_error_ref(error: &keyring::Error) -> SecretStoreError {
    match error {
        keyring::Error::NoStorageAccess(_) => SecretStoreError::new(
            SecretStoreErrorKind::Locked,
            "The operating-system credential store is locked or access was denied. Unlock it and retry.",
        ),
        keyring::Error::NoDefaultStore
        | keyring::Error::NotSupportedByStore(_)
        | keyring::Error::PlatformFailure(_) => SecretStoreError::new(
            SecretStoreErrorKind::Unavailable,
            "The operating-system credential store is unavailable. Start or unlock the platform keychain service and retry.",
        ),
        _ => SecretStoreError::new(
            SecretStoreErrorKind::Failed,
            "The operating-system credential store could not be initialized.",
        ),
    }
}

fn new_reference() -> String {
    format!("{REFERENCE_PREFIX}{}", uuid::Uuid::new_v4())
}

pub(crate) fn new_workspace_key_reference() -> String {
    format!("{WORKSPACE_KEY_REFERENCE_PREFIX}{}", uuid::Uuid::new_v4())
}

fn validate_reference(reference: &str) -> Result<&'static str, SecretStoreError> {
    let (uuid, service) = if let Some(uuid) = reference.strip_prefix(REFERENCE_PREFIX) {
        (uuid, PROVIDER_SERVICE)
    } else if let Some(uuid) = reference.strip_prefix(WORKSPACE_KEY_REFERENCE_PREFIX) {
        (uuid, WORKSPACE_KEY_SERVICE)
    } else {
        return Err(SecretStoreError::new(
            SecretStoreErrorKind::Invalid,
            "Stored credential reference is not a supported opaque identifier.",
        ));
    };
    uuid::Uuid::parse_str(uuid).map_err(|_| {
        SecretStoreError::new(
            SecretStoreErrorKind::Invalid,
            "Stored credential reference is not a supported opaque identifier.",
        )
    })?;
    Ok(service)
}

pub(crate) fn store_workspace_key(reference: &str, key: &str) -> Result<(), AppError> {
    validate_reference(reference).map_err(|error| error.to_app_error())?;
    SystemSecretStore
        .create(reference, SecretValue(key.to_string()))
        .map_err(|error| error.to_app_error())
}

pub(crate) fn read_workspace_key(reference: &str) -> Result<zeroize::Zeroizing<String>, AppError> {
    validate_reference(reference).map_err(|error| error.to_app_error())?;
    SystemSecretStore
        .read(reference)
        .map(|value| zeroize::Zeroizing::new(value.expose().to_string()))
        .map_err(|error| error.to_app_error())
}

pub(crate) fn delete_workspace_key(reference: &str) -> Result<(), AppError> {
    validate_reference(reference).map_err(|error| error.to_app_error())?;
    SystemSecretStore
        .delete(reference)
        .map_err(|error| error.to_app_error())
}

fn validate_secret(value: String) -> Result<SecretValue, AppError> {
    if value.is_empty() || value.len() > MAX_SECRET_BYTES {
        return Err(AppError::invalid_input(format!(
            "Credential must be non-empty and at most {MAX_SECRET_BYTES} bytes."
        )));
    }
    Ok(SecretValue(value))
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SecretMetadata {
    pub id: String,
    pub masked: String,
    pub available: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SecretStoreStatus {
    pub available: bool,
    pub code: String,
    pub message: String,
}

pub async fn get_status() -> SecretStoreStatus {
    match tokio::task::spawn_blocking(|| SystemSecretStore.status()).await {
        Ok(Ok(())) => SecretStoreStatus {
            available: true,
            code: "available".to_string(),
            message: "Operating-system credential storage is available.".to_string(),
        },
        Ok(Err(error)) => {
            let app_error = error.to_app_error();
            SecretStoreStatus {
                available: false,
                code: app_error.code,
                message: app_error.message,
            }
        }
        Err(_) => SecretStoreStatus {
            available: false,
            code: "secret_store_failed".to_string(),
            message: "Credential-store worker did not complete. Retry the operation.".to_string(),
        },
    }
}

pub async fn upsert_provider_secret(
    state: &crate::AppState,
    provider_id: String,
    secret: String,
) -> Result<SecretMetadata, AppError> {
    let provider_id =
        crate::validation::validate_entity_id(&provider_id, "Provider ID")?.to_string();
    let value = validate_secret(secret)?;
    let existing = crate::commands::lock_db(state)?
        .get_provider(&provider_id)?
        .api_key_ref;
    let reference = existing.clone().unwrap_or_else(new_reference);
    let is_update = existing.is_some();
    validate_reference(&reference).map_err(|error| error.to_app_error())?;
    let reference_for_store = reference.clone();
    tokio::task::spawn_blocking(move || {
        if is_update {
            SystemSecretStore.update(&reference_for_store, value)
        } else {
            SystemSecretStore.create(&reference_for_store, value)
        }
    })
    .await
    .map_err(|_| {
        AppError::new(
            "secret_store_failed",
            "Credential-store worker did not complete. Retry.",
        )
    })?
    .map_err(|error| error.to_app_error())?;

    if !is_update {
        let linkage_result = {
            let db = crate::commands::lock_db(state)?;
            db.set_provider_api_key_ref(&provider_id, Some(&reference))
        };
        if let Err(database_error) = linkage_result {
            let compensation_reference = reference.clone();
            let compensation = tokio::task::spawn_blocking(move || {
                SystemSecretStore.delete(&compensation_reference)
            })
            .await;
            if !matches!(compensation, Ok(Ok(()))) {
                return Err(AppError::new(
                    "secret_store_compensation_failed",
                    "Credential storage succeeded but linking it to the provider failed, and Ark could not remove the orphaned credential. Retry deletion from the provider credential controls.",
                ));
            }
            return Err(database_error);
        }
    }
    Ok(metadata(reference, true))
}

pub async fn get_provider_secret_metadata(
    state: &crate::AppState,
    provider_id: String,
) -> Result<Option<SecretMetadata>, AppError> {
    let provider_id = crate::validation::validate_entity_id(&provider_id, "Provider ID")?;
    let Some(reference) = crate::commands::lock_db(state)?
        .get_provider(provider_id)?
        .api_key_ref
    else {
        return Ok(None);
    };
    validate_reference(&reference).map_err(|error| error.to_app_error())?;
    let reference_for_store = reference.clone();
    let result = tokio::task::spawn_blocking(move || SystemSecretStore.read(&reference_for_store))
        .await
        .map_err(|_| {
            AppError::new(
                "secret_store_failed",
                "Credential-store worker did not complete. Retry.",
            )
        })?;
    match result {
        Ok(value) => {
            drop(value);
            Ok(Some(metadata(reference, true)))
        }
        Err(error) if error.kind == SecretStoreErrorKind::NotFound => {
            Ok(Some(metadata(reference, false)))
        }
        Err(error) => Err(error.to_app_error()),
    }
}

pub async fn delete_provider_secret(
    state: &crate::AppState,
    provider_id: String,
) -> Result<(), AppError> {
    let provider_id =
        crate::validation::validate_entity_id(&provider_id, "Provider ID")?.to_string();
    let Some(reference) = crate::commands::lock_db(state)?
        .get_provider(&provider_id)?
        .api_key_ref
    else {
        return Ok(());
    };
    validate_reference(&reference).map_err(|error| error.to_app_error())?;
    tokio::task::spawn_blocking(move || SystemSecretStore.delete(&reference))
        .await
        .map_err(|_| {
            AppError::new(
                "secret_store_failed",
                "Credential-store worker did not complete. Retry.",
            )
        })?
        .map_err(|error| error.to_app_error())?;
    crate::commands::lock_db(state)?.set_provider_api_key_ref(&provider_id, None)?;
    Ok(())
}

fn metadata(id: String, available: bool) -> SecretMetadata {
    SecretMetadata {
        id,
        masked: "\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}".to_string(),
        available,
    }
}

#[cfg(test)]
#[derive(Default)]
struct InMemorySecretStore {
    values: Mutex<HashMap<String, String>>,
}

#[cfg(test)]
impl InMemorySecretStore {
    fn values(&self) -> Result<MutexGuard<'_, HashMap<String, String>>, SecretStoreError> {
        self.values.lock().map_err(|_| {
            SecretStoreError::new(
                SecretStoreErrorKind::Failed,
                "In-memory secret store lock failed.",
            )
        })
    }
}

#[cfg(test)]
impl SecretStore for InMemorySecretStore {
    fn status(&self) -> Result<(), SecretStoreError> {
        Ok(())
    }

    fn create(&self, reference: &str, value: SecretValue) -> Result<(), SecretStoreError> {
        validate_reference(reference)?;
        self.values()?
            .insert(reference.to_string(), value.expose().to_string());
        Ok(())
    }

    fn read(&self, reference: &str) -> Result<SecretValue, SecretStoreError> {
        validate_reference(reference)?;
        self.values()?
            .get(reference)
            .cloned()
            .map(SecretValue)
            .ok_or_else(|| {
                SecretStoreError::new(SecretStoreErrorKind::NotFound, "Credential not found.")
            })
    }

    fn update(&self, reference: &str, value: SecretValue) -> Result<(), SecretStoreError> {
        validate_reference(reference)?;
        let mut values = self.values()?;
        if !values.contains_key(reference) {
            return Err(SecretStoreError::new(
                SecretStoreErrorKind::NotFound,
                "Credential not found.",
            ));
        }
        values.insert(reference.to_string(), value.expose().to_string());
        Ok(())
    }

    fn delete(&self, reference: &str) -> Result<(), SecretStoreError> {
        validate_reference(reference)?;
        self.values()?.remove(reference);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicBool;

    #[test]
    fn in_memory_port_covers_create_read_update_delete_without_metadata_leakage() {
        let store = InMemorySecretStore::default();
        let reference = new_reference();
        store
            .create(&reference, SecretValue("first-secret".to_string()))
            .expect("create");
        assert_eq!(
            store.read(&reference).expect("read").expose(),
            "first-secret"
        );
        store
            .update(&reference, SecretValue("second-secret".to_string()))
            .expect("update");
        assert_eq!(
            store.read(&reference).expect("read").expose(),
            "second-secret"
        );
        let public = metadata(reference.clone(), true);
        let json = serde_json::to_string(&public).expect("serialize metadata");
        assert!(json.contains("\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}"));
        assert!(!json.contains("first-secret"));
        assert!(!json.contains("second-secret"));
        store.delete(&reference).expect("delete");
        assert!(matches!(
            store.read(&reference),
            Err(SecretStoreError {
                kind: SecretStoreErrorKind::NotFound,
                ..
            })
        ));
    }

    #[test]
    fn opaque_references_and_secret_limits_fail_closed() {
        assert!(validate_reference(&new_reference()).is_ok());
        assert!(validate_reference("provider:openai").is_err());
        assert!(validate_secret(String::new()).is_err());
        assert!(validate_secret("x".repeat(MAX_SECRET_BYTES + 1)).is_err());
    }

    #[test]
    fn platform_errors_are_typed_and_never_echo_sensitive_details() {
        let marker = "credential-value-from-platform-error";
        let locked = map_keyring_error(keyring::Error::NoStorageAccess(Box::new(
            std::io::Error::other(marker),
        )))
        .to_app_error();
        assert_eq!(locked.code, "secret_store_locked");
        assert!(!locked.message.contains(marker));

        let unavailable = map_keyring_error(keyring::Error::PlatformFailure(Box::new(
            std::io::Error::other(marker),
        )))
        .to_app_error();
        assert_eq!(unavailable.code, "secret_store_unavailable");
        assert!(!unavailable.message.contains(marker));
    }

    #[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
    #[tokio::test]
    async fn platform_credential_store_and_provider_linkage_round_trip() {
        use crate::config::DEFAULT_PROVIDER_ID;
        use crate::db::Database;
        use crate::sidecar::SidecarState;
        use std::sync::Arc;

        let store = SystemSecretStore;
        store.status().expect("platform credential store available");
        let path =
            std::env::temp_dir().join(format!("ark-secret-store-{}.sqlite3", uuid::Uuid::new_v4()));
        let state = crate::AppState {
            db: Mutex::new(Database::open(&path).expect("writer database opens")),
            workspace: Mutex::new(crate::workspace::WorkspaceInfo {
                root_path: path.parent().expect("parent").display().to_string(),
                database_path: path.display().to_string(),
                default_root_path: path.parent().expect("parent").display().to_string(),
                config_path: path.with_extension("json").display().to_string(),
                is_portable: false,
                requires_restart: false,
            }),
            read_db: Mutex::new(Database::open_read_replica(&path).expect("read database opens")),
            workspace_open_error: Mutex::new(None),
            active_streams: Mutex::new(HashMap::new()),
            pending_streams: Mutex::new(HashMap::new()),
            active_imports: Mutex::new(HashMap::new()),
            storage_maintenance: AtomicBool::new(false),
            sidecar: Arc::new(Mutex::new(SidecarState::new())),
        };

        let first = format!("platform-first-{}", uuid::Uuid::new_v4());
        let second = format!("platform-second-{}", uuid::Uuid::new_v4());
        let created =
            upsert_provider_secret(&state, DEFAULT_PROVIDER_ID.to_string(), first.clone())
                .await
                .expect("create and link platform credential");
        let result = async {
            assert_eq!(
                store
                    .read(&created.id)
                    .map_err(|error| error.to_app_error())?
                    .expose(),
                first
            );
            assert_eq!(
                crate::commands::lock_db(&state)?
                    .get_provider(DEFAULT_PROVIDER_ID)?
                    .api_key_ref
                    .as_deref(),
                Some(created.id.as_str())
            );
            let updated =
                upsert_provider_secret(&state, DEFAULT_PROVIDER_ID.to_string(), second.clone())
                    .await?;
            assert_eq!(updated.id, created.id);
            assert_eq!(
                store
                    .read(&created.id)
                    .map_err(|error| error.to_app_error())?
                    .expose(),
                second
            );
            let public = get_provider_secret_metadata(&state, DEFAULT_PROVIDER_ID.to_string())
                .await?
                .expect("metadata exists");
            assert!(public.available);
            assert_eq!(
                public.masked,
                "\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}"
            );
            delete_provider_secret(&state, DEFAULT_PROVIDER_ID.to_string()).await?;
            assert!(crate::commands::lock_db(&state)?
                .get_provider(DEFAULT_PROVIDER_ID)?
                .api_key_ref
                .is_none());
            assert!(matches!(
                store.read(&created.id),
                Err(SecretStoreError {
                    kind: SecretStoreErrorKind::NotFound,
                    ..
                })
            ));
            Ok::<(), AppError>(())
        }
        .await;
        let _ = store.delete(&created.id);
        drop(state);
        for candidate in [
            path.clone(),
            path.with_extension("sqlite3-wal"),
            path.with_extension("sqlite3-shm"),
        ] {
            let _ = std::fs::remove_file(candidate);
        }
        result.expect("platform credential and database-reference CRUD");
    }
}
