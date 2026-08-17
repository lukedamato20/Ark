use crate::errors::AppError;
use serde::Serialize;
#[cfg(test)]
use std::collections::HashMap;
#[cfg(test)]
use std::sync::{Mutex, MutexGuard};
use zeroize::Zeroize;

const PROVIDER_SERVICE: &str = "dev.ark.desktop.provider-secret";
const WORKSPACE_KEY_SERVICE: &str = "dev.ark.desktop.workspace-key";
const COMPANION_API_TOKEN_SERVICE: &str = "dev.ark.desktop.companion-api-token";
const TOOL_SECRET_SERVICE: &str = "dev.ark.desktop.tool-secret";
const REFERENCE_PREFIX: &str = "secret:v1:";
const WORKSPACE_KEY_REFERENCE_PREFIX: &str = "workspace-key:v1:";
const COMPANION_API_TOKEN_REFERENCE_PREFIX: &str = "companion-api-token:v1:";
const TOOL_SECRET_REFERENCE_PREFIX: &str = "tool-secret:v1:";
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

pub(crate) fn new_companion_api_token_reference() -> String {
    format!(
        "{COMPANION_API_TOKEN_REFERENCE_PREFIX}{}",
        uuid::Uuid::new_v4()
    )
}

fn new_tool_secret_reference() -> String {
    format!("{TOOL_SECRET_REFERENCE_PREFIX}{}", uuid::Uuid::new_v4())
}

fn validate_reference(reference: &str) -> Result<&'static str, SecretStoreError> {
    let (uuid, service) = if let Some(uuid) = reference.strip_prefix(REFERENCE_PREFIX) {
        (uuid, PROVIDER_SERVICE)
    } else if let Some(uuid) = reference.strip_prefix(WORKSPACE_KEY_REFERENCE_PREFIX) {
        (uuid, WORKSPACE_KEY_SERVICE)
    } else if let Some(uuid) = reference.strip_prefix(COMPANION_API_TOKEN_REFERENCE_PREFIX) {
        (uuid, COMPANION_API_TOKEN_SERVICE)
    } else if let Some(uuid) = reference.strip_prefix(TOOL_SECRET_REFERENCE_PREFIX) {
        (uuid, TOOL_SECRET_SERVICE)
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

fn validate_reference_service(
    reference: &str,
    expected_service: &'static str,
) -> Result<(), SecretStoreError> {
    if validate_reference(reference)? != expected_service {
        return Err(SecretStoreError::new(
            SecretStoreErrorKind::Invalid,
            "Stored credential reference belongs to a different secret family.",
        ));
    }
    Ok(())
}

pub(crate) fn store_workspace_key(reference: &str, key: &str) -> Result<(), AppError> {
    validate_reference_service(reference, WORKSPACE_KEY_SERVICE)
        .map_err(|error| error.to_app_error())?;
    SystemSecretStore
        .create(reference, SecretValue(key.to_string()))
        .map_err(|error| error.to_app_error())
}

pub(crate) fn read_workspace_key(reference: &str) -> Result<zeroize::Zeroizing<String>, AppError> {
    validate_reference_service(reference, WORKSPACE_KEY_SERVICE)
        .map_err(|error| error.to_app_error())?;
    SystemSecretStore
        .read(reference)
        .map(|value| zeroize::Zeroizing::new(value.expose().to_string()))
        .map_err(|error| error.to_app_error())
}

pub(crate) fn delete_workspace_key(reference: &str) -> Result<(), AppError> {
    validate_reference_service(reference, WORKSPACE_KEY_SERVICE)
        .map_err(|error| error.to_app_error())?;
    SystemSecretStore
        .delete(reference)
        .map_err(|error| error.to_app_error())
}

/// FTR-007: reads a provider's stored credential value for internal use only — attaching it to
/// an outgoing provider request. Never exposed to the frontend or logged, unlike
/// `get_provider_secret_metadata`, which returns only masked/availability info over IPC.
/// Deliberately kept out of `commands/mod.rs` (the Tauri command surface) — see
/// `scripts/check-secret-boundaries.mjs`'s guard that `commands/mod.rs` must never reference this
/// function's name, so a raw secret can never be one accidental `#[tauri::command]` away from
/// reaching the frontend.
pub(crate) fn read_provider_secret(
    reference: &str,
) -> Result<zeroize::Zeroizing<String>, AppError> {
    validate_reference_service(reference, PROVIDER_SERVICE)
        .map_err(|error| error.to_app_error())?;
    SystemSecretStore
        .read(reference)
        .map(|value| zeroize::Zeroizing::new(value.expose().to_string()))
        .map_err(|error| error.to_app_error())
}

/// FTR-010: stores a freshly generated companion API bearer token under a new reference —
/// mirrors `store_workspace_key`'s "create" shape, kept as its own function (rather than reused
/// directly) so the companion API's storage concern stays self-contained here even though the
/// underlying keychain call is identical.
pub(crate) fn store_companion_api_token(reference: &str, token: &str) -> Result<(), AppError> {
    validate_reference_service(reference, COMPANION_API_TOKEN_SERVICE)
        .map_err(|error| error.to_app_error())?;
    SystemSecretStore
        .create(reference, SecretValue(token.to_string()))
        .map_err(|error| error.to_app_error())
}

/// FTR-010: replaces an existing companion API token's value in place (regeneration) — the
/// reference itself, and therefore the frontend's `tokenConfigured` signal, is unchanged.
pub(crate) fn update_companion_api_token(reference: &str, token: &str) -> Result<(), AppError> {
    validate_reference_service(reference, COMPANION_API_TOKEN_SERVICE)
        .map_err(|error| error.to_app_error())?;
    SystemSecretStore
        .update(reference, SecretValue(token.to_string()))
        .map_err(|error| error.to_app_error())
}

/// FTR-010: reads the companion API's stored bearer token for internal use only — checked
/// against each inbound request's `Authorization` header by `companion_api.rs`. Never exposed to
/// the frontend (unlike the one-time reveal returned by `regenerate_companion_api_token` itself,
/// which is the token's only intentional trip across the IPC boundary) and, like
/// `read_provider_secret`, deliberately not referenced from `commands/mod.rs` —
/// `scripts/check-secret-boundaries.mjs` guards both by name.
pub(crate) fn read_companion_api_token(
    reference: &str,
) -> Result<zeroize::Zeroizing<String>, AppError> {
    validate_reference_service(reference, COMPANION_API_TOKEN_SERVICE)
        .map_err(|error| error.to_app_error())?;
    SystemSecretStore
        .read(reference)
        .map(|value| zeroize::Zeroizing::new(value.expose().to_string()))
        .map_err(|error| error.to_app_error())
}

/// CMP-004: reads a built-in tool's stored credential (e.g. `web_search`'s Brave Search API key)
/// for internal use only — attaching it to an outgoing tool request. Never exposed to the
/// frontend or logged, unlike `get_tool_secret_metadata`, which returns only masked/availability
/// info over IPC. Deliberately kept out of `commands/mod.rs`, like `read_provider_secret`/
/// `read_companion_api_token` — see `scripts/check-secret-boundaries.mjs`'s matching guard.
pub(crate) fn read_tool_secret(reference: &str) -> Result<zeroize::Zeroizing<String>, AppError> {
    validate_reference_service(reference, TOOL_SECRET_SERVICE)
        .map_err(|error| error.to_app_error())?;
    SystemSecretStore
        .read(reference)
        .map(|value| zeroize::Zeroizing::new(value.expose().to_string()))
        .map_err(|error| error.to_app_error())
}

/// SEC-002/FTR-007: the bearer token to attach to this provider's outgoing requests. For the
/// built-in provider type, that's the sidecar-generated token in front of the managed
/// llama-server (never persisted, never returned to the frontend). For any other provider with a
/// stored credential (`api_key_ref` set — e.g. a remote OpenAI-compatible endpoint configured
/// with an API key), it's that credential, read from the OS keychain via `read_provider_secret`
/// above. A user-configured "local inference host" with no stored credential manages its own
/// authentication independently of Ark and gets no header, same as before this covered the cloud
/// case.
///
/// A configured credential reference is a security boundary, not a best-effort hint: if the
/// operating-system store is locked, unavailable, or no longer contains the value, propagate its
/// typed recoverable error rather than silently downgrading the request to unauthenticated.
pub(crate) fn resolve_bearer_token(
    state: &crate::AppState,
    provider: &crate::providers::ProviderConfig,
) -> Result<Option<String>, AppError> {
    if provider.provider_type == crate::config::BUILT_IN_PROVIDER_TYPE {
        return Ok(state
            .sidecar
            .lock()
            .map_err(|_| AppError::new("state_error", "Could not access runtime state."))?
            .api_key());
    }
    let Some(reference) = provider.api_key_ref.as_deref() else {
        return Ok(None);
    };
    read_provider_secret(reference).map(|value| Some(value.to_string()))
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
    validate_reference_service(&reference, PROVIDER_SERVICE)
        .map_err(|error| error.to_app_error())?;
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
    validate_reference_service(&reference, PROVIDER_SERVICE)
        .map_err(|error| error.to_app_error())?;
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
    validate_reference_service(&reference, PROVIDER_SERVICE)
        .map_err(|error| error.to_app_error())?;
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

/// FTR-007: deletes a user-managed provider and its credential as one compensating operation.
/// SQLite and the OS keychain cannot share a transaction, so Ark removes the credential first,
/// retains it only in zeroizing memory, and restores it if the database transaction fails.
pub(crate) async fn delete_user_provider_and_secret(
    state: &crate::AppState,
    provider: crate::providers::ProviderConfig,
) -> Result<(), AppError> {
    let reference = provider.api_key_ref.clone();
    let removed_secret = if let Some(reference) = reference.as_deref() {
        validate_reference_service(reference, PROVIDER_SERVICE)
            .map_err(|error| error.to_app_error())?;
        let read_reference = reference.to_string();
        let existing = tokio::task::spawn_blocking(move || SystemSecretStore.read(&read_reference))
            .await
            .map_err(|_| {
                AppError::new(
                    "secret_store_failed",
                    "Credential-store worker did not complete. Retry provider deletion.",
                )
            })?;
        let value = match existing {
            Ok(value) => Some(value),
            Err(error) if error.kind == SecretStoreErrorKind::NotFound => None,
            Err(error) => return Err(error.to_app_error()),
        };
        let delete_reference = reference.to_string();
        tokio::task::spawn_blocking(move || SystemSecretStore.delete(&delete_reference))
            .await
            .map_err(|_| {
                AppError::new(
                    "secret_store_failed",
                    "Credential-store worker did not complete. Retry provider deletion.",
                )
            })?
            .map_err(|error| error.to_app_error())?;
        value
    } else {
        None
    };

    let database_result = crate::commands::lock_db(state)?.delete_user_provider(&provider.id);
    if let Err(database_error) = database_result {
        if let (Some(reference), Some(value)) = (reference, removed_secret) {
            let compensation =
                tokio::task::spawn_blocking(move || SystemSecretStore.create(&reference, value))
                    .await;
            if !matches!(compensation, Ok(Ok(()))) {
                return Err(AppError::new(
                    "secret_store_compensation_failed",
                    "Provider deletion failed and Ark could not restore its credential. Re-add the credential before retrying.",
                ));
            }
        }
        return Err(database_error);
    }
    Ok(())
}

/// CMP-004: stores or replaces a built-in tool's credential — structurally identical to
/// `upsert_provider_secret` above (existing-reference lookup, `spawn_blocking` keyring
/// create/update, compensating deletion if the DB link write fails), keyed by `tool_id` against
/// `tool_secrets` instead of `providers.api_key_ref`.
pub async fn upsert_tool_secret(
    state: &crate::AppState,
    tool_id: String,
    secret: String,
) -> Result<SecretMetadata, AppError> {
    let tool_id = crate::validation::validate_entity_id(&tool_id, "Tool ID")?.to_string();
    let value = validate_secret(secret)?;
    let existing = crate::commands::lock_db(state)?.get_tool_secret_ref(&tool_id)?;
    let reference = existing.clone().unwrap_or_else(new_tool_secret_reference);
    let is_update = existing.is_some();
    validate_reference_service(&reference, TOOL_SECRET_SERVICE)
        .map_err(|error| error.to_app_error())?;
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
        let linkage_result =
            crate::commands::lock_db(state)?.set_tool_secret_ref(&tool_id, &reference);
        if let Err(database_error) = linkage_result {
            let compensation_reference = reference.clone();
            let compensation = tokio::task::spawn_blocking(move || {
                SystemSecretStore.delete(&compensation_reference)
            })
            .await;
            if !matches!(compensation, Ok(Ok(()))) {
                return Err(AppError::new(
                    "secret_store_compensation_failed",
                    "Credential storage succeeded but linking it to the tool failed, and Ark could not remove the orphaned credential. Retry deletion from the tool's credential controls.",
                ));
            }
            return Err(database_error);
        }
    }
    Ok(metadata(reference, true))
}

pub async fn get_tool_secret_metadata(
    state: &crate::AppState,
    tool_id: String,
) -> Result<Option<SecretMetadata>, AppError> {
    let tool_id = crate::validation::validate_entity_id(&tool_id, "Tool ID")?.to_string();
    let Some(reference) = crate::commands::lock_db(state)?.get_tool_secret_ref(&tool_id)? else {
        return Ok(None);
    };
    validate_reference_service(&reference, TOOL_SECRET_SERVICE)
        .map_err(|error| error.to_app_error())?;
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

pub async fn delete_tool_secret(state: &crate::AppState, tool_id: String) -> Result<(), AppError> {
    let tool_id = crate::validation::validate_entity_id(&tool_id, "Tool ID")?.to_string();
    let Some(reference) = crate::commands::lock_db(state)?.get_tool_secret_ref(&tool_id)? else {
        return Ok(());
    };
    validate_reference_service(&reference, TOOL_SECRET_SERVICE)
        .map_err(|error| error.to_app_error())?;
    tokio::task::spawn_blocking(move || SystemSecretStore.delete(&reference))
        .await
        .map_err(|_| {
            AppError::new(
                "secret_store_failed",
                "Credential-store worker did not complete. Retry.",
            )
        })?
        .map_err(|error| error.to_app_error())?;
    crate::commands::lock_db(state)?.delete_tool_secret_ref(&tool_id)?;
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
        assert!(validate_reference(&new_tool_secret_reference()).is_ok());
        assert!(validate_reference("provider:openai").is_err());
        assert!(validate_secret(String::new()).is_err());
        assert!(validate_secret("x".repeat(MAX_SECRET_BYTES + 1)).is_err());
    }

    #[test]
    fn typed_secret_wrappers_reject_cross_family_references_before_platform_access() {
        let provider = new_reference();
        let workspace = new_workspace_key_reference();
        let companion = new_companion_api_token_reference();
        let tool = new_tool_secret_reference();

        assert!(validate_reference_service(&provider, PROVIDER_SERVICE).is_ok());
        assert!(validate_reference_service(&workspace, WORKSPACE_KEY_SERVICE).is_ok());
        assert!(validate_reference_service(&companion, COMPANION_API_TOKEN_SERVICE).is_ok());
        assert!(validate_reference_service(&tool, TOOL_SECRET_SERVICE).is_ok());

        for (reference, wrong_service) in [
            (&workspace, PROVIDER_SERVICE),
            (&companion, PROVIDER_SERVICE),
            (&tool, PROVIDER_SERVICE),
            (&provider, WORKSPACE_KEY_SERVICE),
            (&provider, COMPANION_API_TOKEN_SERVICE),
            (&provider, TOOL_SECRET_SERVICE),
        ] {
            let error = validate_reference_service(reference, wrong_service)
                .expect_err("cross-family reference must be rejected");
            assert_eq!(error.kind, SecretStoreErrorKind::Invalid);
        }

        for result in [
            read_provider_secret(&workspace).map(|_| ()),
            read_workspace_key(&provider).map(|_| ()),
            read_companion_api_token(&provider).map(|_| ()),
            read_tool_secret(&provider).map(|_| ()),
        ] {
            let error = result.expect_err("typed wrapper must reject a different secret family");
            assert_eq!(error.code, "secret_reference_invalid");
        }
    }

    /// CMP-004: the tool-secret reference family (4th prefix) round-trips through the same
    /// generic `SecretStore` port as the other three — proving `validate_reference`'s new arm and
    /// `new_tool_secret_reference` actually produce a working reference, not just a
    /// pattern-matchable string.
    #[test]
    fn tool_secret_reference_round_trips_through_the_in_memory_port() {
        let store = InMemorySecretStore::default();
        let reference = new_tool_secret_reference();
        store
            .create(&reference, SecretValue("brave-api-key".to_string()))
            .expect("create");
        assert_eq!(
            store.read(&reference).expect("read").expose(),
            "brave-api-key"
        );
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
            active_ollama_pulls: Mutex::new(HashMap::new()),
            active_provider_refreshes: Mutex::new(HashMap::new()),
            active_managed_model_downloads: Mutex::new(HashMap::new()),
            storage_maintenance: AtomicBool::new(false),
            sidecar: Arc::new(Mutex::new(SidecarState::new())),
            observability_log: Arc::new(Mutex::new(crate::observability::DiagnosticsLog::new())),
            companion_api: Mutex::new(None),
        };

        let first = format!("platform-first-{}", uuid::Uuid::new_v4());
        let second = format!("platform-second-{}", uuid::Uuid::new_v4());
        let created =
            upsert_provider_secret(&state, DEFAULT_PROVIDER_ID.to_string(), first.clone())
                .await
                .expect("create and link platform credential");
        let remote_cleanup = Arc::new(Mutex::new(None::<String>));
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
            // FTR-007: `read_provider_secret` is the internal-only path `resolve_bearer_token`
            // uses to attach this credential to outgoing requests for a non-built-in provider —
            // must return the exact same value the public metadata endpoint only masks.
            assert_eq!(*read_provider_secret(&created.id)?, second);
            let provider = crate::commands::lock_db(&state)?.get_provider(DEFAULT_PROVIDER_ID)?;
            assert_eq!(
                resolve_bearer_token(&state, &provider)?,
                Some(second.clone())
            );
            let mut provider_without_credential = provider.clone();
            provider_without_credential.api_key_ref = None;
            assert_eq!(
                resolve_bearer_token(&state, &provider_without_credential)?,
                None
            );
            let mut provider_with_invalid_reference = provider.clone();
            provider_with_invalid_reference.api_key_ref = Some("not-an-opaque-reference".into());
            let invalid_reference = resolve_bearer_token(&state, &provider_with_invalid_reference)
                .expect_err("a configured invalid reference must fail closed");
            assert_eq!(invalid_reference.code, "secret_reference_invalid");
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

            // FTR-007 deletion acceptance: a confirmed user-provider deletion removes both the
            // SQLite reference/row and the actual platform credential, not just one side.
            let remote = crate::commands::lock_db(&state)?.create_remote_provider(
                crate::db::CreateRemoteProviderChanges {
                    name: "Platform deletion provider",
                    provider_type: crate::config::OPENAI_PROVIDER_TYPE,
                    base_url: crate::config::OPENAI_PROVIDER_BASE_URL,
                    acknowledge_remote_risk: true,
                    allow_insecure_remote: false,
                },
            )?;
            let remote_secret = upsert_provider_secret(
                &state,
                remote.id.clone(),
                format!("platform-remote-{}", uuid::Uuid::new_v4()),
            )
            .await?;
            *remote_cleanup.lock().expect("cleanup lock") = Some(remote_secret.id.clone());
            crate::provider_management::delete_provider(&state, remote.id.clone(), true).await?;
            assert!(crate::commands::lock_db(&state)?
                .get_provider(&remote.id)
                .is_err());
            assert!(matches!(
                store.read(&remote_secret.id),
                Err(SecretStoreError {
                    kind: SecretStoreErrorKind::NotFound,
                    ..
                })
            ));
            Ok::<(), AppError>(())
        }
        .await;
        let _ = store.delete(&created.id);
        if let Some(reference) = remote_cleanup.lock().expect("cleanup lock").take() {
            let _ = store.delete(&reference);
        }
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
