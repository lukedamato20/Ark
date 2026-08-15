//! ARC-002: verifies every Rust DTO that crosses the Tauri IPC boundary still serializes to
//! exactly the field set recorded in `contract/schema.json` — the single source of truth this
//! test and its TypeScript counterpart (`scripts/check-contract.mjs`, checked via `pnpm run
//! contract:check`) are both independently checked against. Neither side reads the other's
//! source directly; either one drifting from the shared fixture fails its own language's test
//! suite. `#[cfg(test)]`-only: this module (and its `serde_json`/fixture-reading machinery) is
//! never compiled into a release build.
#![cfg(test)]

use crate::attachments::Attachment;
use crate::chat::{
    BranchAlternative, Conversation, ConversationPage, Message, SendChatResult, StreamEvent,
};
use crate::commands::ImportProgressEvent;
use crate::companion_api::{CompanionApiStatus, CompanionApiTokenReveal};
use crate::data_protection::{
    WorkspaceProtectionChange, WorkspaceProtectionMode, WorkspaceProtectionStatus,
};
use crate::diagnostics::{BenchmarkResult, DiagnosticsResult};
use crate::errors::AppError;
use crate::import_export::{
    ImportConversationPreview, ImportConversationResult, ImportProviderMapping,
    WorkspaceImportPreview, WorkspaceImportPreviewEntry, WorkspaceImportResult,
};
use crate::personas::{Persona, PersonaDeletionPreview, PersonaVersionSummary};
use crate::projects::{Project, ProjectDeletionPreview};
use crate::provider_management::{BuiltInRuntimeStatus, RefreshModelsResult};
use crate::providers::{
    ModelInfo, OllamaPullProgress, ProviderCapabilities, ProviderConfig, ProviderHealth,
};
use crate::secret_store::{SecretMetadata, SecretStoreStatus};
use crate::sidecar::{
    RuntimeDiagnostics, RuntimeFailure, RuntimeFailureCategory, RuntimeLifecycleState,
    RuntimeLogEntry,
};
use crate::supply_chain::{InstalledFileProvenance, ModelProvenance, RuntimeProvenance};
use crate::tool_policy::{
    AuditEvent, AuditEventKind, CapabilityScope, CapabilityTier, IdempotencyPolicy,
    SideEffectPreview,
};
use crate::tools::{ConversationNote, ToolCapabilityGrant, ToolDefinition, ToolStatus};
use crate::workspace::WorkspaceInfo;
use crate::workspace_bootstrap::AppBootstrap;
use serde::Serialize;
use serde_json::Value;
use std::collections::{BTreeMap, HashSet};

fn schema() -> BTreeMap<String, Vec<String>> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../contract/schema.json");
    let raw = std::fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "could not read contract fixture at {}: {error}",
            path.display()
        )
    });
    let parsed: Value =
        serde_json::from_str(&raw).expect("contract/schema.json must be valid JSON");
    let types = parsed
        .get("types")
        .expect("contract/schema.json must have a top-level \"types\" object");
    serde_json::from_value(types.clone())
        .expect("contract/schema.json \"types\" must be an object of string arrays")
}

/// Asserts that `value`'s top-level JSON object keys are exactly the field set recorded for
/// `type_name` in `contract/schema.json` — same members, in either order; nothing missing, and
/// nothing this build added without updating the fixture.
fn assert_matches_contract<T: Serialize>(type_name: &str, value: &T) {
    let contract = schema();
    let expected: HashSet<&str> = contract
        .get(type_name)
        .unwrap_or_else(|| panic!("contract/schema.json has no entry for \"{type_name}\" — add one alongside the Rust struct and the TypeScript interface."))
        .iter()
        .map(String::as_str)
        .collect();

    let serialized = serde_json::to_value(value).expect("DTO must serialize to JSON");
    let actual: HashSet<&str> = serialized
        .as_object()
        .unwrap_or_else(|| panic!("\"{type_name}\" must serialize to a JSON object"))
        .keys()
        .map(String::as_str)
        .collect();

    let missing: Vec<&&str> = expected.difference(&actual).collect();
    let unexpected: Vec<&&str> = actual.difference(&expected).collect();
    assert!(
        missing.is_empty() && unexpected.is_empty(),
        "\"{type_name}\" has drifted from contract/schema.json — missing fields: {missing:?}, \
         unexpected fields: {unexpected:?}. If this is an intentional change, update \
         contract/schema.json (and the corresponding TypeScript interface in src/types/ark.ts) \
         in the same change — see docs/protocol-versioning.md."
    );
}

fn sample_conversation() -> Conversation {
    Conversation {
        id: "conversation-1".to_string(),
        title: "Sample".to_string(),
        created_at: "2026-08-13T00:00:00Z".to_string(),
        updated_at: "2026-08-13T00:00:00Z".to_string(),
        provider_id: Some("provider-1".to_string()),
        model_id: Some("model-1".to_string()),
        current_message_id: None,
        system_prompt: None,
        temperature: Some(0.7),
        max_tokens: Some(2048),
        archived: false,
        project_id: None,
        pinned_at: None,
        persona_id: None,
        response_style: Some("concise".to_string()),
        tone: Some("friendly".to_string()),
    }
}

fn sample_message() -> Message {
    Message {
        id: "message-1".to_string(),
        conversation_id: "conversation-1".to_string(),
        parent_message_id: None,
        revision_of_message_id: None,
        path_index: 0,
        role: "user".to_string(),
        content: "Hello".to_string(),
        status: "complete".to_string(),
        created_at: "2026-08-13T00:00:00Z".to_string(),
        updated_at: "2026-08-13T00:00:00Z".to_string(),
        provider_id: None,
        model_id: None,
        token_count: None,
        error_message: None,
        metadata_json: None,
        branch_name: None,
    }
}

fn sample_provider_config() -> ProviderConfig {
    ProviderConfig {
        id: "provider-1".to_string(),
        name: "Ollama".to_string(),
        provider_type: "ollama".to_string(),
        base_url: Some("http://localhost:11434".to_string()),
        api_key_ref: None,
        default_model_id: None,
        default_temperature: Some(0.7),
        default_max_tokens: Some(2048),
        is_local: true,
        allow_insecure_remote: false,
        destination_class: "loopback".to_string(),
        capabilities: crate::providers::ProviderCapabilities::for_provider_type("ollama"),
        is_enabled: true,
        created_at: "2026-08-13T00:00:00Z".to_string(),
        updated_at: "2026-08-13T00:00:00Z".to_string(),
    }
}

fn sample_model_info() -> ModelInfo {
    ModelInfo {
        id: "model-1".to_string(),
        provider_id: "provider-1".to_string(),
        name: "llama3.2:latest".to_string(),
        display_name: None,
        context_window: None,
        supports_streaming: true,
        supports_tools: false,
        supports_vision: false,
        supports_embeddings: false,
        is_available: true,
        last_seen_at: None,
        metadata_json: None,
        created_at: "2026-08-13T00:00:00Z".to_string(),
        updated_at: "2026-08-13T00:00:00Z".to_string(),
    }
}

fn sample_project() -> Project {
    Project {
        id: "project-1".to_string(),
        name: "Research".to_string(),
        instructions: Some("Cite sources.".to_string()),
        default_provider_id: Some("provider-1".to_string()),
        default_model_id: Some("model-1".to_string()),
        default_temperature: Some(0.2),
        default_max_tokens: Some(4096),
        response_style: Some("technical".to_string()),
        tone: Some("direct".to_string()),
        archived_at: None,
        created_at: "2026-08-13T00:00:00Z".to_string(),
        updated_at: "2026-08-13T00:00:00Z".to_string(),
    }
}

fn sample_provider_health() -> ProviderHealth {
    ProviderHealth {
        provider_id: "provider-1".to_string(),
        is_reachable: true,
        status: "ok".to_string(),
        message: "Reachable".to_string(),
        checked_at: "2026-08-14T00:00:00Z".to_string(),
    }
}

#[test]
fn conversation_matches_contract() {
    assert_matches_contract("Conversation", &sample_conversation());
}

#[test]
fn conversation_page_matches_contract() {
    assert_matches_contract(
        "ConversationPage",
        &ConversationPage {
            items: vec![sample_conversation()],
            next_cursor: Some("opaque-cursor".to_string()),
            search_snippets: std::collections::HashMap::from([(
                "conversation-1".to_string(),
                "a matching …snippet…".to_string(),
            )]),
        },
    );
}

#[test]
fn message_matches_contract() {
    assert_matches_contract("Message", &sample_message());
}

#[test]
fn project_matches_contract() {
    assert_matches_contract("Project", &sample_project());
}

#[test]
fn project_deletion_preview_matches_contract() {
    assert_matches_contract(
        "ProjectDeletionPreview",
        &ProjectDeletionPreview {
            project: sample_project(),
            conversation_count: 3,
        },
    );
}

fn sample_persona() -> Persona {
    Persona {
        id: "persona-1".to_string(),
        name: "Terse reviewer".to_string(),
        instructions: "Be terse and cite line numbers.".to_string(),
        default_temperature: Some(0.2),
        default_max_tokens: Some(512),
        response_style: Some("explanatory".to_string()),
        tone: Some("professional".to_string()),
        version_number: 2,
        archived_at: None,
        created_at: "2026-08-13T00:00:00Z".to_string(),
        updated_at: "2026-08-13T00:00:00Z".to_string(),
    }
}

#[test]
fn persona_matches_contract() {
    assert_matches_contract("Persona", &sample_persona());
}

#[test]
fn persona_version_summary_matches_contract() {
    assert_matches_contract(
        "PersonaVersionSummary",
        &PersonaVersionSummary {
            id: "persona-version-1".to_string(),
            version_number: 2,
            instructions: "Be terse and cite line numbers.".to_string(),
            default_temperature: Some(0.2),
            default_max_tokens: Some(512),
            response_style: Some("explanatory".to_string()),
            tone: Some("professional".to_string()),
            created_at: "2026-08-13T00:00:00Z".to_string(),
        },
    );
}

#[test]
fn persona_deletion_preview_matches_contract() {
    assert_matches_contract(
        "PersonaDeletionPreview",
        &PersonaDeletionPreview {
            persona: sample_persona(),
            conversation_count: 2,
        },
    );
}

#[test]
fn attachment_matches_contract() {
    assert_matches_contract(
        "Attachment",
        &Attachment {
            id: "attachment-1".to_string(),
            conversation_id: "conversation-1".to_string(),
            message_id: Some("message-1".to_string()),
            file_name: "notes.txt".to_string(),
            byte_size: 42,
            sha256: "a".repeat(64),
            created_at: "2026-08-13T00:00:00Z".to_string(),
        },
    );
}

fn sample_capability_scope() -> CapabilityScope {
    CapabilityScope {
        tier: CapabilityTier::ChatSafe,
        read: true,
        write: true,
        network: false,
        secret: false,
        data: "This conversation's own notes".to_string(),
    }
}

#[test]
fn capability_scope_matches_contract() {
    assert_matches_contract("CapabilityScope", &sample_capability_scope());
}

#[test]
fn tool_definition_matches_contract() {
    assert_matches_contract(
        "ToolDefinition",
        &ToolDefinition {
            id: "notes".to_string(),
            name: "Notes".to_string(),
            description: "Read and write a short scratch note attached to this conversation."
                .to_string(),
            publisher: "Ark (built-in)".to_string(),
            scope: sample_capability_scope(),
        },
    );
}

fn sample_tool_capability_grant() -> ToolCapabilityGrant {
    ToolCapabilityGrant {
        id: "grant-1".to_string(),
        tool_id: "notes".to_string(),
        tier: CapabilityTier::ChatSafe,
        read: true,
        write: true,
        network: false,
        secret: false,
        data: "This conversation's own notes".to_string(),
        granted_at: "2026-08-15T00:00:00Z".to_string(),
        expires_at: "2026-08-15T00:05:00Z".to_string(),
        revoked: false,
    }
}

#[test]
fn tool_capability_grant_matches_contract() {
    assert_matches_contract("ToolCapabilityGrant", &sample_tool_capability_grant());
}

#[test]
fn tool_status_matches_contract() {
    assert_matches_contract(
        "ToolStatus",
        &ToolStatus {
            definition: ToolDefinition {
                id: "notes".to_string(),
                name: "Notes".to_string(),
                description: "Read and write a short scratch note attached to this conversation."
                    .to_string(),
                publisher: "Ark (built-in)".to_string(),
                scope: sample_capability_scope(),
            },
            active_grant: Some(sample_tool_capability_grant()),
        },
    );
}

#[test]
fn conversation_note_matches_contract() {
    assert_matches_contract(
        "ConversationNote",
        &ConversationNote {
            id: "note-1".to_string(),
            conversation_id: "conversation-1".to_string(),
            content: "Remember to follow up".to_string(),
            created_at: "2026-08-15T00:00:00Z".to_string(),
            updated_at: "2026-08-15T00:00:00Z".to_string(),
        },
    );
}

#[test]
fn side_effect_preview_matches_contract() {
    assert_matches_contract(
        "SideEffectPreview",
        &SideEffectPreview {
            tool_id: "notes".to_string(),
            summary: "Create a new note in this conversation: \"Remember to follow up\""
                .to_string(),
            idempotency: IdempotencyPolicy::RequiresFreshApproval,
        },
    );
}

#[test]
fn audit_event_matches_contract() {
    assert_matches_contract(
        "AuditEvent",
        &AuditEvent {
            sequence: 0,
            timestamp: "2026-08-15T00:00:00Z".to_string(),
            kind: AuditEventKind::Granted,
            tool_id: "notes".to_string(),
            redacted_detail: "granted: notes for 5 min".to_string(),
            chain_hash: "0123456789abcdef".to_string(),
        },
    );
}

#[test]
fn branch_alternative_matches_contract() {
    assert_matches_contract(
        "BranchAlternative",
        &BranchAlternative {
            message_id: "message-1".to_string(),
            revision_of_message_id: None,
            created_at: "2026-08-13T00:00:00Z".to_string(),
            status: "complete".to_string(),
            content_preview: "Hello".to_string(),
            is_active: true,
            has_descendants: false,
            branch_name: None,
        },
    );
}

#[test]
fn provider_config_matches_contract() {
    assert_matches_contract("ProviderConfig", &sample_provider_config());
}

#[test]
fn provider_capabilities_matches_contract() {
    assert_matches_contract(
        "ProviderCapabilities",
        &ProviderCapabilities::for_provider_type("ollama"),
    );
}

#[test]
fn secret_metadata_matches_contract() {
    assert_matches_contract(
        "SecretMetadata",
        &SecretMetadata {
            id: "secret:v1:00000000-0000-4000-8000-000000000000".to_string(),
            masked: "\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}".to_string(),
            available: true,
        },
    );
}

#[test]
fn secret_store_status_matches_contract() {
    assert_matches_contract(
        "SecretStoreStatus",
        &SecretStoreStatus {
            available: true,
            code: "available".to_string(),
            message: "Operating-system credential storage is available.".to_string(),
        },
    );
}

fn sample_workspace_protection_status() -> WorkspaceProtectionStatus {
    WorkspaceProtectionStatus {
        mode: WorkspaceProtectionMode::Encrypted,
        locked: false,
        transition_in_progress: false,
        key_available: true,
        message: "Workspace is encrypted.".to_string(),
    }
}

#[test]
fn workspace_protection_status_matches_contract() {
    assert_matches_contract(
        "WorkspaceProtectionStatus",
        &sample_workspace_protection_status(),
    );
}

#[test]
fn workspace_protection_change_matches_contract() {
    assert_matches_contract(
        "WorkspaceProtectionChange",
        &WorkspaceProtectionChange {
            status: sample_workspace_protection_status(),
            recovery_key: Some("shown-once".to_string()),
        },
    );
}

#[test]
fn model_info_matches_contract() {
    assert_matches_contract("ModelInfo", &sample_model_info());
}

#[test]
fn provider_health_matches_contract() {
    assert_matches_contract("ProviderHealth", &sample_provider_health());
}

#[test]
fn app_bootstrap_matches_contract() {
    assert_matches_contract(
        "AppBootstrap",
        &AppBootstrap {
            conversation_page: ConversationPage {
                items: vec![sample_conversation()],
                next_cursor: None,
                search_snippets: std::collections::HashMap::new(),
            },
            providers: vec![sample_provider_config()],
            models: vec![sample_model_info()],
            projects: vec![sample_project()],
            personas: vec![sample_persona()],
            workspace_path: "C:\\workspace\\ark.sqlite3".to_string(),
            workspace: sample_workspace_info(),
            device_settings: crate::device_settings::DeviceSettings {
                theme: "dark".to_string(),
                built_in_model_path: None,
                crash_capture_enabled: false,
            },
            workspace_open_error: None,
        },
    );
}

#[test]
fn device_settings_matches_contract() {
    assert_matches_contract(
        "DeviceSettings",
        &crate::device_settings::DeviceSettings {
            theme: "dark".to_string(),
            built_in_model_path: Some("model.gguf".to_string()),
            crash_capture_enabled: true,
        },
    );
}

fn sample_workspace_info() -> WorkspaceInfo {
    WorkspaceInfo {
        root_path: "C:\\workspace".to_string(),
        database_path: "C:\\workspace\\ark.sqlite3".to_string(),
        default_root_path: "C:\\workspace".to_string(),
        config_path: "C:\\workspace\\config.json".to_string(),
        is_portable: false,
        requires_restart: false,
    }
}

#[test]
fn workspace_info_matches_contract() {
    assert_matches_contract("WorkspaceInfo", &sample_workspace_info());
}

fn sample_backup_manifest() -> crate::backup::BackupManifest {
    crate::backup::BackupManifest {
        app_version: "0.1.0".to_string(),
        created_at: "2026-08-14T00:00:00Z".to_string(),
        database_sha256: "a".repeat(64),
        database_size_bytes: 4096,
    }
}

#[test]
fn backup_manifest_matches_contract() {
    assert_matches_contract("BackupManifest", &sample_backup_manifest());
}

#[test]
fn backup_result_matches_contract() {
    assert_matches_contract(
        "BackupResult",
        &crate::backup::BackupResult {
            backup_path: "C:\\backups\\ark.sqlite3".to_string(),
            manifest: sample_backup_manifest(),
        },
    );
}

#[test]
fn restore_preview_matches_contract() {
    assert_matches_contract(
        "RestorePreview",
        &crate::backup::RestorePreview {
            manifest: Some(sample_backup_manifest()),
            detected_schema_version: 5,
            schema_supported: true,
            conversation_count: 12,
            message_count: 340,
        },
    );
}

#[test]
fn diagnostics_bundle_matches_contract() {
    assert_matches_contract(
        "DiagnosticsBundle",
        &crate::diagnostics_bundle::DiagnosticsBundle {
            generated_at: "2026-08-14T00:00:00Z".to_string(),
            preview_text: "Ark diagnostics bundle...".to_string(),
        },
    );
}

#[test]
fn send_chat_result_matches_contract() {
    assert_matches_contract(
        "SendChatResult",
        &SendChatResult {
            conversation_id: "conversation-1".to_string(),
            user_message_id: "message-1".to_string(),
            assistant_message_id: "message-2".to_string(),
        },
    );
}

#[test]
fn stream_event_matches_contract() {
    assert_matches_contract(
        "StreamEvent",
        &StreamEvent {
            conversation_id: "conversation-1".to_string(),
            message_id: "message-1".to_string(),
            delta: Some("Hi".to_string()),
            content: None,
            status: "streaming".to_string(),
            error: None,
            revision: Some(1),
            schema_version: crate::chat::STREAM_EVENT_SCHEMA_VERSION,
        },
    );
}

#[test]
fn refresh_models_result_matches_contract() {
    assert_matches_contract(
        "RefreshModelsResult",
        &RefreshModelsResult {
            health: sample_provider_health(),
            models: vec![sample_model_info()],
            provider: sample_provider_config(),
        },
    );
}

#[test]
fn diagnostics_result_matches_contract() {
    assert_matches_contract(
        "DiagnosticsResult",
        &DiagnosticsResult {
            os: "Windows 11".to_string(),
            cpu: "Unknown CPU".to_string(),
            cpu_cores: 8,
            total_memory_bytes: 0,
            available_memory_bytes: 0,
            total_disk_bytes: 0,
            available_disk_bytes: 0,
            gpu: "unknown".to_string(),
            provider_health: sample_provider_health(),
            model_available: true,
            benchmark: Some(sample_benchmark_result()),
            benchmark_failure: None,
            guidance: "Good for small and medium local models.".to_string(),
            runtime: sample_runtime_diagnostics(),
        },
    );
}

fn sample_runtime_failure() -> RuntimeFailure {
    RuntimeFailure {
        category: RuntimeFailureCategory::HealthUnreachable,
        message: "Health endpoint unavailable.".to_string(),
    }
}

fn sample_runtime_log_entry() -> RuntimeLogEntry {
    RuntimeLogEntry {
        timestamp_ms: 1_765_497_600_000,
        stream: "stderr".to_string(),
        message: "safe output".to_string(),
    }
}

fn sample_runtime_diagnostics() -> RuntimeDiagnostics {
    RuntimeDiagnostics {
        state: RuntimeLifecycleState::Degraded,
        pid: Some(42),
        port: Some(11_435),
        model_configured: true,
        failure: Some(sample_runtime_failure()),
        recent_logs: vec![sample_runtime_log_entry()],
    }
}

#[test]
fn runtime_failure_matches_contract() {
    assert_matches_contract("RuntimeFailure", &sample_runtime_failure());
}

#[test]
fn runtime_log_entry_matches_contract() {
    assert_matches_contract("RuntimeLogEntry", &sample_runtime_log_entry());
}

#[test]
fn runtime_diagnostics_matches_contract() {
    assert_matches_contract("RuntimeDiagnostics", &sample_runtime_diagnostics());
}

fn sample_benchmark_result() -> BenchmarkResult {
    BenchmarkResult {
        time_to_first_token_ms: Some(120),
        generation_time_ms: Some(780),
        total_time_ms: 900,
        approximate_tokens_per_second: Some(12.5),
        output_preview: "Hello".to_string(),
    }
}

#[test]
fn benchmark_result_matches_contract() {
    assert_matches_contract("BenchmarkResult", &sample_benchmark_result());
}

#[test]
fn built_in_runtime_status_matches_contract() {
    let installed_file = InstalledFileProvenance {
        name: "llama-server".to_string(),
        size_bytes: 10,
        sha256: "a".repeat(64),
    };
    let runtime_provenance = RuntimeProvenance {
        schema_version: 1,
        runtime: "llama.cpp".to_string(),
        version: "b9859".to_string(),
        source_repository: "https://github.com/ggml-org/llama.cpp".to_string(),
        source_commit: "commit".to_string(),
        license: "MIT".to_string(),
        license_url: "https://example.test/license".to_string(),
        artifact_file_name: "runtime.zip".to_string(),
        artifact_url: "https://example.test/runtime.zip".to_string(),
        artifact_sha256: "b".repeat(64),
        runtime_sha256: "a".repeat(64),
        platform: "win32".to_string(),
        arch: "x64".to_string(),
        verified_at: "2026-08-14T00:00:00Z".to_string(),
        installed_files: vec![installed_file.clone()],
    };
    let model_provenance = ModelProvenance {
        path: "model.gguf".to_string(),
        source: "https://example.test/model".to_string(),
        license: "Apache-2.0".to_string(),
        sha256: "c".repeat(64),
        size_bytes: 20,
        verified_at: "2026-08-14T00:00:00Z".to_string(),
    };
    assert_matches_contract("InstalledFileProvenance", &installed_file);
    assert_matches_contract("RuntimeProvenance", &runtime_provenance);
    assert_matches_contract("ModelProvenance", &model_provenance);
    assert_matches_contract(
        "BuiltInRuntimeStatus",
        &BuiltInRuntimeStatus {
            running: true,
            port: Some(11435),
            model_path: Some("model.gguf".to_string()),
            binary_installed: true,
            binary_verified: true,
            runtime_provenance: Some(runtime_provenance),
            model_provenance: Some(model_provenance),
            state: RuntimeLifecycleState::Healthy,
            failure: None,
        },
    );
}

#[test]
fn app_error_matches_contract_as_app_error_shape() {
    assert_matches_contract(
        "AppErrorShape",
        &AppError::new("invalid_input", "Example error."),
    );
}

#[test]
fn import_conversation_result_matches_contract() {
    assert_matches_contract(
        "ImportConversationResult",
        &ImportConversationResult {
            conversation: sample_conversation(),
            normalized_message_count: 0,
        },
    );
}

fn sample_import_provider_mapping() -> ImportProviderMapping {
    ImportProviderMapping {
        source_provider_id: Some("provider-1".to_string()),
        target_provider_id: "provider-1".to_string(),
        reason: "Matched an existing provider by stable ID.".to_string(),
    }
}

#[test]
fn import_provider_mapping_matches_contract() {
    assert_matches_contract("ImportProviderMapping", &sample_import_provider_mapping());
}

#[test]
fn import_conversation_preview_matches_contract() {
    assert_matches_contract(
        "ImportConversationPreview",
        &ImportConversationPreview {
            conversation_count: 1,
            message_count: 2,
            maximum_branch_depth: 2,
            normalized_message_count: 0,
            conflicts: Vec::new(),
            provider_mappings: vec![sample_import_provider_mapping()],
            estimated_storage_bytes: 1024,
        },
    );
}

#[test]
fn import_progress_event_matches_contract() {
    assert_matches_contract(
        "ImportProgressEvent",
        &ImportProgressEvent {
            import_id: "import-1".to_string(),
            completed_messages: 100,
            total_messages: 200,
        },
    );
}

#[test]
fn ollama_pull_progress_matches_contract() {
    assert_matches_contract(
        "OllamaPullProgress",
        &OllamaPullProgress {
            provider_id: "provider-1".to_string(),
            model_name: "llama3.2:latest".to_string(),
            status: "pulling manifest".to_string(),
            total: Some(100),
            completed: Some(50),
            digest: None,
            error: None,
        },
    );
}

fn sample_workspace_import_preview_entry() -> WorkspaceImportPreviewEntry {
    WorkspaceImportPreviewEntry {
        conversation_id: "conversation-1".to_string(),
        title: "Example conversation".to_string(),
        message_count: 4,
        duplicate_of_local_id: None,
    }
}

#[test]
fn workspace_import_preview_entry_matches_contract() {
    assert_matches_contract(
        "WorkspaceImportPreviewEntry",
        &sample_workspace_import_preview_entry(),
    );
}

#[test]
fn workspace_import_preview_matches_contract() {
    assert_matches_contract(
        "WorkspaceImportPreview",
        &WorkspaceImportPreview {
            scope: "workspace".to_string(),
            entries: vec![sample_workspace_import_preview_entry()],
            provider_mappings: vec![sample_import_provider_mapping()],
        },
    );
}

#[test]
fn workspace_import_result_matches_contract() {
    assert_matches_contract(
        "WorkspaceImportResult",
        &WorkspaceImportResult {
            imported_count: 3,
            skipped_count: 1,
        },
    );
}

fn sample_companion_api_status() -> CompanionApiStatus {
    CompanionApiStatus {
        enabled: true,
        running: true,
        port: Some(51234),
        token_configured: true,
    }
}

#[test]
fn companion_api_status_matches_contract() {
    assert_matches_contract("CompanionApiStatus", &sample_companion_api_status());
}

#[test]
fn companion_api_token_reveal_matches_contract() {
    assert_matches_contract(
        "CompanionApiTokenReveal",
        &CompanionApiTokenReveal {
            token: "example-token-value".to_string(),
            status: sample_companion_api_status(),
        },
    );
}
