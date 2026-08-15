import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { openUrl } from "@tauri-apps/plugin-opener";
import type {
  AppBootstrap,
  Attachment,
  AuditEvent,
  BackupResult,
  BranchAlternative,
  BuiltInRuntimeStatus,
  Conversation,
  ConversationNote,
  ConversationPage,
  DeviceSettings,
  DiagnosticsBundle,
  DiagnosticsResult,
  CompanionApiStatus,
  CompanionApiTokenReveal,
  ImportConversationPreview,
  ImportConversationResult,
  ImportProgressEvent,
  Message,
  NoteWriteAction,
  OllamaPullProgress,
  Persona,
  PersonaDeletionPreview,
  PersonaVersionSummary,
  Project,
  ProjectDeletionPreview,
  ProviderConfig,
  RefreshModelsResult,
  RestorePreview,
  SendChatResult,
  SecretMetadata,
  SecretStoreStatus,
  SideEffectPreview,
  StreamEvent,
  ToolCapabilityGrant,
  ToolStatus,
  WorkspaceImportPreview,
  WorkspaceImportResult,
  WorkspaceInfo,
  WorkspaceProtectionChange,
  WorkspaceProtectionStatus,
} from "../types/ark";

export interface SendChatMessageInput {
  conversationId: string;
  content: string;
  providerId: string;
  model: string;
  temperature?: number | null;
  maxTokens?: number | null;
  /** CMP-001: ids of staged attachments to link to this message and disclose to the provider. */
  attachmentIds?: string[];
}

export interface EditUserMessageInput {
  conversationId: string;
  messageId: string;
  content: string;
  providerId: string;
  model: string;
  temperature?: number | null;
  maxTokens?: number | null;
}

export interface RegenerateAssistantMessageInput {
  conversationId: string;
  messageId: string;
  providerId: string;
  model: string;
  temperature?: number | null;
  maxTokens?: number | null;
}

export interface UpdateProviderInput {
  providerId: string;
  baseUrl: string;
  defaultModelId?: string | null;
  temperature?: number | null;
  maxTokens?: number | null;
  /** SEC-001: must be true to save a base URL that classifies as a public/remote destination. */
  acknowledgeRemoteRisk?: boolean;
  /** Explicit transition from the local-only provider class to Remote. */
  convertToRemoteProvider?: boolean;
  /** Explicit development-mode exception for non-loopback HTTP. */
  allowInsecureRemote?: boolean;
}

export interface UpdateConversationSettingsInput {
  id: string;
  systemPrompt?: string | null;
  temperature?: number | null;
  maxTokens?: number | null;
}

export interface UpdateProjectInput {
  id: string;
  name: string;
  instructions?: string | null;
  defaultProviderId?: string | null;
  defaultModelId?: string | null;
  defaultTemperature?: number | null;
  defaultMaxTokens?: number | null;
}

export interface CreatePersonaInput {
  name: string;
  instructions: string;
  defaultTemperature?: number | null;
  defaultMaxTokens?: number | null;
}

export interface UpdatePersonaInput {
  id: string;
  name: string;
  instructions: string;
  defaultTemperature?: number | null;
  defaultMaxTokens?: number | null;
}

export interface ListConversationsInput {
  limit?: number;
  cursor?: string | null;
  query?: string | null;
  /** false = active, true = archived, null/undefined = both. */
  archived?: boolean | null;
  projectId?: string | null;
}

/**
 * ARC-002: the single typed boundary between the frontend and the Rust backend. Every bridge
 * operation (command invocation or event subscription) goes through this interface — no
 * component calls `@tauri-apps/api` directly. That is what makes `TauriArkClient` swappable for
 * a fake in component tests (see `createFakeArkClient`) without a global Tauri mock: a test
 * constructs a fake implementing this same interface and injects it via `ArkClientProvider`
 * (see `ArkClientContext.tsx`).
 */
export interface ArkClient {
  getAppBootstrap(): Promise<AppBootstrap>;
  retryWorkspaceOpen(): Promise<AppBootstrap>;
  /** FTR-001: `copyData` seeds the new location with a verified copy of the current workspace
   * database before repointing to it — omitted/`false` is "start empty" (the pre-existing
   * behavior). There is deliberately no "move" option; see `backup.rs`'s doc comment. */
  setWorkspace(rootPath: string, copyData?: boolean): Promise<WorkspaceInfo>;
  resetWorkspace(): Promise<WorkspaceInfo>;
  /** FTR-001: creates a verified, hash-manifested backup of the current workspace database at
   * `destinationDir`, which must not already contain one. Never touches the live workspace. */
  createWorkspaceBackup(destinationDir: string): Promise<BackupResult>;
  /** FTR-001: read-only inspection of a backup file — integrity, schema compatibility, and
   * conversation/message counts — without touching the live workspace or the backup itself. */
  previewWorkspaceRestore(backupPath: string): Promise<RestorePreview>;
  /** FTR-001: restores `backupPath` into a brand-new directory at `targetRoot` (must not already
   * contain a workspace database). Never touches the live workspace; use `setWorkspace` after to
   * actually switch to it. */
  restoreWorkspaceBackup(backupPath: string, targetRoot: string): Promise<void>;
  /** OPS-001: assembles the reviewable diagnostics bundle text — call this and show the exact
   * `previewText` to the user before ever calling `saveDiagnosticsBundle`. */
  exportDiagnosticsBundle(): Promise<DiagnosticsBundle>;
  /** OPS-001: writes `bundleText` verbatim to `destinationPath` — always pass back the exact
   * text the user reviewed from `exportDiagnosticsBundle`, never a re-derived value. */
  saveDiagnosticsBundle(destinationPath: string, bundleText: string): Promise<void>;
  getWorkspaceProtectionStatus(): Promise<WorkspaceProtectionStatus>;
  enableWorkspaceEncryption(): Promise<WorkspaceProtectionChange>;
  rotateWorkspaceEncryption(): Promise<WorkspaceProtectionChange>;
  disableWorkspaceEncryption(): Promise<WorkspaceProtectionStatus>;
  restoreWorkspaceRecoveryKey(recoveryKey: string): Promise<WorkspaceProtectionStatus>;
  /** ARC-006: theme and the built-in runtime's model path — device-scoped, not workspace-scoped. */
  updateDeviceSettings(settings: DeviceSettings): Promise<DeviceSettings>;

  /**
   * SEC-008: opens a URL through the OS's default browser/handler via the Tauri opener plugin,
   * never by navigating the app's own webview. Callers must validate the URL themselves first
   * (see `src/lib/externalLinks.ts`) — this method is a thin pass-through with no orchestration,
   * matching every other method on this interface; the opener plugin's own capability grant
   * (`opener:allow-default-urls`, `src-tauri/capabilities/default.json`) is a second, independent
   * scheme allowlist enforced natively as defense in depth.
   */
  openExternalUrl(url: string): Promise<void>;

  listConversations(input: ListConversationsInput): Promise<ConversationPage>;
  createConversation(title?: string): Promise<Conversation>;
  renameConversation(id: string, title: string): Promise<Conversation>;
  /** FTR-004: each field independently `null`/omitted clears that override tier back to
   * "inherit the provider default" — always send the complete current draft, not a partial patch. */
  updateConversationSettings(input: UpdateConversationSettingsInput): Promise<Conversation>;
  /** FTR-002: undo is simply calling this again with the opposite value — no separate undo
   * mechanism exists because none is needed for a mutation this cheap and reversible. */
  setConversationArchived(id: string, archived: boolean): Promise<Conversation>;
  setConversationPinned(id: string, pinned: boolean): Promise<Conversation>;
  /** FTR-003: `projectId: null` unassigns the conversation from any project. */
  setConversationProject(id: string, projectId: string | null): Promise<Conversation>;
  deleteConversation(id: string): Promise<void>;
  getConversationMessages(conversationId: string): Promise<Message[]>;
  /** FTR-005: full content, unlike `getAssistantAlternatives`' 140-character preview — used by
   * the branch comparison view. */
  getMessage(id: string): Promise<Message>;
  getAssistantAlternatives(conversationId: string, messageId: string): Promise<BranchAlternative[]>;
  switchActiveBranch(conversationId: string, messageId: string): Promise<Message[]>;
  /** FTR-005: `name: null` clears the label back to the default ordinal presentation. Only
   * assistant messages can be named. */
  setBranchName(messageId: string, name: string | null): Promise<Message>;
  keepPartialMessage(messageId: string): Promise<Message>;
  discardInterruptedMessage(conversationId: string, messageId: string): Promise<Message[]>;

  sendChatMessage(input: SendChatMessageInput): Promise<SendChatResult>;
  editUserMessage(input: EditUserMessageInput): Promise<SendChatResult>;
  regenerateAssistantMessage(input: RegenerateAssistantMessageInput): Promise<SendChatResult>;
  startPendingStream(messageId: string): Promise<void>;
  cancelStream(messageId: string): Promise<void>;

  refreshModels(providerId: string): Promise<RefreshModelsResult>;
  updateProvider(input: UpdateProviderInput): Promise<ProviderConfig>;
  getSecretStoreStatus(): Promise<SecretStoreStatus>;
  upsertProviderSecret(providerId: string, secret: string): Promise<SecretMetadata>;
  getProviderSecretMetadata(providerId: string): Promise<SecretMetadata | null>;
  deleteProviderSecret(providerId: string): Promise<void>;
  pullOllamaModel(providerId: string, modelName: string): Promise<void>;
  deleteOllamaModel(providerId: string, modelName: string): Promise<void>;
  /** FTR-006: signals cancellation to an in-flight pull for this provider, if any — a no-op if
   * none is running. Ollama has no documented pull-cancel endpoint; this stops Ark from reading
   * the response stream, which closes the connection and lets Ollama detect the abort itself. */
  cancelOllamaPull(providerId: string): Promise<void>;

  listProjects(): Promise<Project[]>;
  createProject(name: string): Promise<Project>;
  /** Sends the complete current draft, not a partial patch — matching
   * `updateConversationSettings`'s convention. */
  updateProject(input: UpdateProjectInput): Promise<Project>;
  /** FTR-003: undo is calling this again with the opposite value. */
  setProjectArchived(id: string, archived: boolean): Promise<Project>;
  /** What deleting this project would affect — call before `deleteProject` so the user can
   * confirm, since deletion unassigns (not deletes) every conversation still in the project. */
  previewProjectDeletion(id: string): Promise<ProjectDeletionPreview>;
  deleteProject(id: string): Promise<void>;

  /** FTR-003: `personaId: null` unassigns the conversation from any persona — independent of
   * `setConversationProject`. */
  setConversationPersona(id: string, personaId: string | null): Promise<Conversation>;
  listPersonas(): Promise<Persona[]>;
  createPersona(input: CreatePersonaInput): Promise<Persona>;
  /** Sends the complete current draft, not a partial patch — matching `updateProject`'s
   * convention. Whether this creates a new immutable version or just renames in place is decided
   * server-side (`Database::update_persona`): only if `instructions`/the defaults actually
   * changed from the persona's current version. */
  updatePersona(input: UpdatePersonaInput): Promise<Persona>;
  /** FTR-003 criterion 2: every version ever created for this persona, newest first — the
   * visible proof that versioning is real. */
  listPersonaVersions(id: string): Promise<PersonaVersionSummary[]>;
  /** FTR-003: undo is calling this again with the opposite value. */
  setPersonaArchived(id: string, archived: boolean): Promise<Persona>;
  /** What deleting this persona would affect — call before `deletePersona` so the user can
   * confirm, since deletion unassigns (not deletes) every conversation still assigned to it. */
  previewPersonaDeletion(id: string): Promise<PersonaDeletionPreview>;
  deletePersona(id: string): Promise<void>;

  /** CMP-001: stages a text attachment against a conversation, before the message it will be
   * sent with even exists — the "preview/remove before send" flow. Content-sniffed and
   * size-bounded server-side; the client-side accept-extension list is a UX nicety only. */
  attachTextFile(conversationId: string, fileName: string, content: string): Promise<Attachment>;
  listConversationAttachments(conversationId: string): Promise<Attachment[]>;
  getAttachmentContent(id: string): Promise<string>;
  /** Only succeeds while the attachment is still staged (`messageId` still `null`) — one already
   * part of a sent message is not offered for deletion. */
  deleteAttachment(id: string): Promise<void>;

  /** CMP-003: every built-in tool's declared definition plus whichever grant (if any) currently
   * governs it. Today this is always exactly the one built-in "notes" tool. */
  listTools(): Promise<ToolStatus[]>;
  /** Proactively grants a tool access for `ttlMinutes` (1-60) without going through the
   * preview/approve flow — the Settings-panel "grant this tool access" path. */
  grantToolCapability(toolId: string, ttlMinutes: number): Promise<ToolCapabilityGrant>;
  /** Immediate and independent of expiry. */
  revokeToolCapability(id: string): Promise<void>;
  /** The full, ordered, tamper-evident audit trail — every grant/revoke/invocation across every
   * tool, oldest first. */
  listToolAuditEvents(): Promise<AuditEvent[]>;
  /** Recomputes the persisted audit chain's hashes from scratch; `true` means it is genuinely
   * unmodified since it was written. */
  verifyToolAuditTrail(): Promise<boolean>;

  listConversationNotes(conversationId: string): Promise<ConversationNote[]>;
  /** A human-readable preview of a notes write, shown before the user approves it. `content` is
   * required for `"create"`/`"update"`, ignored for `"delete"`. */
  previewNoteWrite(action: NoteWriteAction, content?: string | null): Promise<SideEffectPreview>;
  /** `approve: true` only after the user has seen `previewNoteWrite`'s output and confirmed it —
   * unnecessary (and ignored) while a still-valid grant already covers this tool. Rejects with
   * `{ code: "approval_required" }` if no valid grant exists and `approve` is `false`. */
  createNote(conversationId: string, content: string, approve: boolean): Promise<ConversationNote>;
  updateNote(id: string, content: string, approve: boolean): Promise<ConversationNote>;
  deleteNote(id: string, approve: boolean): Promise<void>;

  runDiagnostics(providerId: string, model?: string | null, includeRuntimeLogs?: boolean): Promise<DiagnosticsResult>;

  exportConversationMarkdown(conversationId: string): Promise<string>;
  exportConversationJson(conversationId: string): Promise<string>;
  previewConversationImport(json: string): Promise<ImportConversationPreview>;
  importConversationJson(importId: string, json: string): Promise<ImportConversationResult>;
  cancelImport(importId: string): Promise<void>;

  /** FTR-008: `projectId` omitted (or null) exports every conversation in the workspace. */
  exportWorkspaceJson(projectId?: string | null): Promise<string>;
  exportWorkspaceMarkdown(projectId?: string | null): Promise<string>;
  previewWorkspaceImport(json: string): Promise<WorkspaceImportPreview>;
  /** Imports only the conversations whose IDs are in `includeConversationIds` — anything else
   * in the bundle is skipped, matching the preview's per-entry include choice. */
  importWorkspaceJson(json: string, includeConversationIds: string[]): Promise<WorkspaceImportResult>;

  /** FTR-010: the local companion/integration API — disabled by default, loopback-only in this
   * build (paired-LAN mode depends on MOB-009's device pairing, not yet built). */
  getCompanionApiStatus(): Promise<CompanionApiStatus>;
  setCompanionApiEnabled(enabled: boolean): Promise<CompanionApiStatus>;
  /** Returns the new bearer token exactly once — it is never retrievable again after this call,
   * matching the workspace-encryption recovery-key convention. */
  regenerateCompanionApiToken(): Promise<CompanionApiTokenReveal>;

  getBuiltInRuntimeStatus(): Promise<BuiltInRuntimeStatus>;
  startBuiltInRuntime(modelPath: string, modelSource: string, modelLicense: string): Promise<BuiltInRuntimeStatus>;
  stopBuiltInRuntime(): Promise<void>;

  /**
   * ARC-002: every `StreamEvent` carries `schemaVersion` (see `types/ark.ts`). A handler
   * registered through these methods only ever sees events at a schema version this build
   * understands — `guardStreamEventVersion` (used by `createTauriArkClient` below) is where
   * forward-compatible "ignore and log" handling for a future, newer version lives, instead of
   * every call site having to check.
   */
  onStreamDelta(handler: (event: StreamEvent) => void): Promise<UnlistenFn>;
  onStreamComplete(handler: (event: StreamEvent) => void): Promise<UnlistenFn>;
  onStreamError(handler: (event: StreamEvent) => void): Promise<UnlistenFn>;
  onStreamCancelled(handler: (event: StreamEvent) => void): Promise<UnlistenFn>;
  onStreamInterrupted(handler: (event: StreamEvent) => void): Promise<UnlistenFn>;
  onOllamaPullProgress(handler: (event: OllamaPullProgress) => void): Promise<UnlistenFn>;
  onImportProgress(handler: (event: ImportProgressEvent) => void): Promise<UnlistenFn>;
}

/** The highest `StreamEvent.schemaVersion` this build knows how to interpret. */
export const KNOWN_STREAM_EVENT_SCHEMA_VERSION = 1;

function guardStreamEventVersion(handler: (event: StreamEvent) => void) {
  return (event: StreamEvent) => {
    if (event.schemaVersion > KNOWN_STREAM_EVENT_SCHEMA_VERSION) {
      // Forward-compatible unknown-version handling: a future Ark backend build sending a
      // newer event shape must not crash or corrupt state in an older frontend build — log and
      // drop rather than acting on fields this build was never told about.
      console.warn(
        `Ignoring chat stream event with unknown schemaVersion ${event.schemaVersion} ` +
          `(this build understands up to ${KNOWN_STREAM_EVENT_SCHEMA_VERSION}).`,
      );
      return;
    }
    handler(event);
  };
}

/** The real adapter: every method is a thin call into `@tauri-apps/api` — no orchestration. */
export function createTauriArkClient(): ArkClient {
  return {
    getAppBootstrap: () => invoke<AppBootstrap>("get_app_bootstrap"),
    retryWorkspaceOpen: () => invoke<AppBootstrap>("retry_workspace_open"),
    setWorkspace: (rootPath, copyData) => invoke<WorkspaceInfo>("set_workspace", { request: { rootPath, copyData } }),
    resetWorkspace: () => invoke<WorkspaceInfo>("reset_workspace"),
    createWorkspaceBackup: (destinationDir) => invoke<BackupResult>("create_workspace_backup", { destinationDir }),
    previewWorkspaceRestore: (backupPath) => invoke<RestorePreview>("preview_workspace_restore", { backupPath }),
    restoreWorkspaceBackup: (backupPath, targetRoot) =>
      invoke<void>("restore_workspace_backup", { backupPath, targetRoot }),
    exportDiagnosticsBundle: () => invoke<DiagnosticsBundle>("export_diagnostics_bundle"),
    saveDiagnosticsBundle: (destinationPath, bundleText) =>
      invoke<void>("save_diagnostics_bundle", { destinationPath, bundleText }),
    getWorkspaceProtectionStatus: () => invoke<WorkspaceProtectionStatus>("get_workspace_protection_status"),
    enableWorkspaceEncryption: () => invoke<WorkspaceProtectionChange>("enable_workspace_encryption"),
    rotateWorkspaceEncryption: () => invoke<WorkspaceProtectionChange>("rotate_workspace_encryption"),
    disableWorkspaceEncryption: () => invoke<WorkspaceProtectionStatus>("disable_workspace_encryption"),
    restoreWorkspaceRecoveryKey: (recoveryKey) =>
      invoke<WorkspaceProtectionStatus>("restore_workspace_recovery_key", {
        request: { recoveryKey },
      }),
    updateDeviceSettings: (settings) => invoke<DeviceSettings>("update_device_settings", { settings }),
    openExternalUrl: (url) => openUrl(url),

    listConversations: (input) =>
      invoke<ConversationPage>("list_conversations", {
        request: {
          limit: input.limit,
          cursor: input.cursor ?? undefined,
          query: input.query ?? undefined,
          archived: input.archived ?? undefined,
          projectId: input.projectId ?? undefined,
        },
      }),
    createConversation: (title) => invoke<Conversation>("create_conversation", { title }),
    renameConversation: (id, title) => invoke<Conversation>("rename_conversation", { request: { id, title } }),
    updateConversationSettings: (input) =>
      invoke<Conversation>("update_conversation_settings", {
        request: {
          id: input.id,
          systemPrompt: input.systemPrompt ?? null,
          temperature: input.temperature ?? null,
          maxTokens: input.maxTokens ?? null,
        },
      }),
    setConversationArchived: (id, archived) => invoke<Conversation>("set_conversation_archived", { id, archived }),
    setConversationPinned: (id, pinned) => invoke<Conversation>("set_conversation_pinned", { id, pinned }),
    setConversationProject: (id, projectId) => invoke<Conversation>("set_conversation_project", { id, projectId }),
    deleteConversation: (id) => invoke<void>("delete_conversation", { id }),
    getConversationMessages: (conversationId) => invoke<Message[]>("get_conversation_messages", { conversationId }),
    getMessage: (id) => invoke<Message>("get_message", { id }),
    getAssistantAlternatives: (conversationId, messageId) =>
      invoke<BranchAlternative[]>("get_assistant_alternatives", { request: { conversationId, messageId } }),
    switchActiveBranch: (conversationId, messageId) =>
      invoke<Message[]>("switch_active_branch", { request: { conversationId, messageId } }),
    setBranchName: (messageId, name) => invoke<Message>("set_branch_name", { messageId, name }),
    keepPartialMessage: (messageId) => invoke<Message>("keep_partial_message", { messageId }),
    discardInterruptedMessage: (conversationId, messageId) =>
      invoke<Message[]>("discard_interrupted_message", { request: { conversationId, messageId } }),

    sendChatMessage: (input) =>
      invoke<SendChatResult>("send_chat_message", {
        request: {
          conversationId: input.conversationId,
          content: input.content,
          providerId: input.providerId,
          model: input.model,
          temperature: input.temperature ?? undefined,
          maxTokens: input.maxTokens ?? undefined,
          attachmentIds: input.attachmentIds ?? [],
        },
      }),
    editUserMessage: (input) =>
      invoke<SendChatResult>("edit_user_message", {
        request: {
          conversationId: input.conversationId,
          messageId: input.messageId,
          content: input.content,
          providerId: input.providerId,
          model: input.model,
          temperature: input.temperature ?? undefined,
          maxTokens: input.maxTokens ?? undefined,
        },
      }),
    regenerateAssistantMessage: (input) =>
      invoke<SendChatResult>("regenerate_assistant_message", {
        request: {
          conversationId: input.conversationId,
          messageId: input.messageId,
          providerId: input.providerId,
          model: input.model,
          temperature: input.temperature ?? undefined,
          maxTokens: input.maxTokens ?? undefined,
        },
      }),
    startPendingStream: (messageId) => invoke<void>("start_pending_stream", { messageId }),
    cancelStream: (messageId) => invoke<void>("cancel_stream", { messageId }),

    refreshModels: (providerId) => invoke<RefreshModelsResult>("refresh_models", { providerId }),
    updateProvider: (input) =>
      invoke<ProviderConfig>("update_provider", {
        request: {
          providerId: input.providerId,
          baseUrl: input.baseUrl,
          defaultModelId: input.defaultModelId ?? null,
          temperature: input.temperature ?? null,
          maxTokens: input.maxTokens ?? null,
          acknowledgeRemoteRisk: input.acknowledgeRemoteRisk ?? false,
          convertToRemoteProvider: input.convertToRemoteProvider ?? false,
          allowInsecureRemote: input.allowInsecureRemote ?? false,
        },
      }),
    getSecretStoreStatus: () => invoke<SecretStoreStatus>("get_secret_store_status"),
    upsertProviderSecret: (providerId, secret) =>
      invoke<SecretMetadata>("upsert_provider_secret", { providerId, secret }),
    getProviderSecretMetadata: (providerId) =>
      invoke<SecretMetadata | null>("get_provider_secret_metadata", { providerId }),
    deleteProviderSecret: (providerId) => invoke<void>("delete_provider_secret", { providerId }),
    pullOllamaModel: (providerId, modelName) =>
      invoke<void>("pull_ollama_model", { request: { providerId, modelName } }),
    deleteOllamaModel: (providerId, modelName) =>
      invoke<void>("delete_ollama_model", { request: { providerId, modelName } }),
    cancelOllamaPull: (providerId) => invoke<void>("cancel_ollama_pull", { providerId }),

    listProjects: () => invoke<Project[]>("list_projects"),
    createProject: (name) => invoke<Project>("create_project", { name }),
    updateProject: (input) => invoke<Project>("update_project", { request: input }),
    setProjectArchived: (id, archived) => invoke<Project>("set_project_archived", { id, archived }),
    previewProjectDeletion: (id) => invoke<ProjectDeletionPreview>("preview_project_deletion", { id }),
    deleteProject: (id) => invoke<void>("delete_project", { id }),

    setConversationPersona: (id, personaId) => invoke<Conversation>("set_conversation_persona", { id, personaId }),
    listPersonas: () => invoke<Persona[]>("list_personas"),
    createPersona: (input) => invoke<Persona>("create_persona", { request: input }),
    updatePersona: (input) => invoke<Persona>("update_persona", { request: input }),
    listPersonaVersions: (id) => invoke<PersonaVersionSummary[]>("list_persona_versions", { id }),
    setPersonaArchived: (id, archived) => invoke<Persona>("set_persona_archived", { id, archived }),
    previewPersonaDeletion: (id) => invoke<PersonaDeletionPreview>("preview_persona_deletion", { id }),
    deletePersona: (id) => invoke<void>("delete_persona", { id }),

    attachTextFile: (conversationId, fileName, content) =>
      invoke<Attachment>("attach_text_file", { conversationId, fileName, content }),
    listConversationAttachments: (conversationId) =>
      invoke<Attachment[]>("list_conversation_attachments", { conversationId }),
    getAttachmentContent: (id) => invoke<string>("get_attachment_content", { id }),
    deleteAttachment: (id) => invoke<void>("delete_attachment", { id }),

    listTools: () => invoke<ToolStatus[]>("list_tools"),
    grantToolCapability: (toolId, ttlMinutes) =>
      invoke<ToolCapabilityGrant>("grant_tool_capability", { request: { toolId, ttlMinutes } }),
    revokeToolCapability: (id) => invoke<void>("revoke_tool_capability", { id }),
    listToolAuditEvents: () => invoke<AuditEvent[]>("list_tool_audit_events"),
    verifyToolAuditTrail: () => invoke<boolean>("verify_tool_audit_trail"),

    listConversationNotes: (conversationId) =>
      invoke<ConversationNote[]>("list_conversation_notes", { conversationId }),
    previewNoteWrite: (action, content) =>
      invoke<SideEffectPreview>("preview_note_write", { request: { action, content: content ?? null } }),
    createNote: (conversationId, content, approve) =>
      invoke<ConversationNote>("create_note", { request: { conversationId, content, approve } }),
    updateNote: (id, content, approve) =>
      invoke<ConversationNote>("update_note", { request: { id, content, approve } }),
    deleteNote: (id, approve) => invoke<void>("delete_note", { request: { id, approve } }),

    runDiagnostics: (providerId, model, includeRuntimeLogs = false) =>
      invoke<DiagnosticsResult>("run_diagnostics", {
        providerId,
        model: model ?? null,
        includeRuntimeLogs,
      }),

    exportConversationMarkdown: (conversationId) => invoke<string>("export_conversation_markdown", { conversationId }),
    exportConversationJson: (conversationId) => invoke<string>("export_conversation_json", { conversationId }),
    previewConversationImport: (json) => invoke<ImportConversationPreview>("preview_conversation_import", { json }),
    importConversationJson: (importId, json) =>
      invoke<ImportConversationResult>("import_conversation_json", { request: { importId, json } }),
    cancelImport: (importId) => invoke<void>("cancel_import", { importId }),

    exportWorkspaceJson: (projectId) => invoke<string>("export_workspace_json", { projectId: projectId ?? null }),
    exportWorkspaceMarkdown: (projectId) =>
      invoke<string>("export_workspace_markdown", { projectId: projectId ?? null }),
    previewWorkspaceImport: (json) => invoke<WorkspaceImportPreview>("preview_workspace_import", { json }),
    importWorkspaceJson: (json, includeConversationIds) =>
      invoke<WorkspaceImportResult>("import_workspace_json", { json, includeConversationIds }),

    getCompanionApiStatus: () => invoke<CompanionApiStatus>("get_companion_api_status"),
    setCompanionApiEnabled: (enabled) => invoke<CompanionApiStatus>("set_companion_api_enabled", { enabled }),
    regenerateCompanionApiToken: () => invoke<CompanionApiTokenReveal>("regenerate_companion_api_token"),

    getBuiltInRuntimeStatus: () => invoke<BuiltInRuntimeStatus>("get_built_in_runtime_status"),
    startBuiltInRuntime: (modelPath, modelSource, modelLicense) =>
      invoke<BuiltInRuntimeStatus>("start_built_in_runtime", { modelPath, modelSource, modelLicense }),
    stopBuiltInRuntime: () => invoke<void>("stop_built_in_runtime"),

    onStreamDelta: (handler) =>
      listen<StreamEvent>("chat:stream-delta", (e) => guardStreamEventVersion(handler)(e.payload)),
    onStreamComplete: (handler) =>
      listen<StreamEvent>("chat:stream-complete", (e) => guardStreamEventVersion(handler)(e.payload)),
    onStreamError: (handler) =>
      listen<StreamEvent>("chat:stream-error", (e) => guardStreamEventVersion(handler)(e.payload)),
    onStreamCancelled: (handler) =>
      listen<StreamEvent>("chat:stream-cancelled", (e) => guardStreamEventVersion(handler)(e.payload)),
    onStreamInterrupted: (handler) =>
      listen<StreamEvent>("chat:stream-interrupted", (e) => guardStreamEventVersion(handler)(e.payload)),
    onOllamaPullProgress: (handler) => listen<OllamaPullProgress>("ollama:pull-progress", (e) => handler(e.payload)),
    onImportProgress: (handler) => listen<ImportProgressEvent>("import:progress", (e) => handler(e.payload)),
  };
}

/**
 * ARC-002 acceptance evidence: a minimal in-memory fake implementing the exact same `ArkClient`
 * interface as `createTauriArkClient`, proving the interface is small and coherent enough to
 * substitute without a global Tauri mock. `overrides` lets a specific test replace only the
 * methods it cares about; every other method has a harmless default. Event methods resolve to a
 * no-op unsubscribe function by default — a test that needs to actually fire an event should
 * override the relevant `onX` method to capture and later invoke the handler it's given.
 */
export function createFakeArkClient(overrides: Partial<ArkClient> = {}): ArkClient {
  const notImplemented = (method: string) => () => {
    throw new Error(`createFakeArkClient: "${method}" was called without an override.`);
  };
  const noopUnlisten: UnlistenFn = () => undefined;
  const noopSubscribe = async () => noopUnlisten;

  const defaults: ArkClient = {
    getAppBootstrap: notImplemented("getAppBootstrap"),
    retryWorkspaceOpen: notImplemented("retryWorkspaceOpen"),
    setWorkspace: notImplemented("setWorkspace"),
    resetWorkspace: notImplemented("resetWorkspace"),
    createWorkspaceBackup: notImplemented("createWorkspaceBackup"),
    previewWorkspaceRestore: notImplemented("previewWorkspaceRestore"),
    restoreWorkspaceBackup: notImplemented("restoreWorkspaceBackup"),
    exportDiagnosticsBundle: notImplemented("exportDiagnosticsBundle"),
    saveDiagnosticsBundle: notImplemented("saveDiagnosticsBundle"),
    getWorkspaceProtectionStatus: async () => ({
      mode: "plaintext",
      locked: false,
      transitionInProgress: false,
      keyAvailable: false,
      message: "Workspace database is plaintext.",
    }),
    enableWorkspaceEncryption: notImplemented("enableWorkspaceEncryption"),
    rotateWorkspaceEncryption: notImplemented("rotateWorkspaceEncryption"),
    disableWorkspaceEncryption: notImplemented("disableWorkspaceEncryption"),
    restoreWorkspaceRecoveryKey: notImplemented("restoreWorkspaceRecoveryKey"),
    updateDeviceSettings: async (settings) => settings,
    openExternalUrl: async () => undefined,

    listConversations: async () => ({ items: [], nextCursor: null, searchSnippets: {} }),
    createConversation: notImplemented("createConversation"),
    renameConversation: notImplemented("renameConversation"),
    updateConversationSettings: notImplemented("updateConversationSettings"),
    setConversationArchived: notImplemented("setConversationArchived"),
    setConversationPinned: notImplemented("setConversationPinned"),
    setConversationProject: notImplemented("setConversationProject"),
    deleteConversation: async () => undefined,
    getConversationMessages: async () => [],
    getMessage: notImplemented("getMessage"),
    getAssistantAlternatives: async () => [],
    switchActiveBranch: notImplemented("switchActiveBranch"),
    setBranchName: notImplemented("setBranchName"),
    keepPartialMessage: notImplemented("keepPartialMessage"),
    discardInterruptedMessage: notImplemented("discardInterruptedMessage"),

    sendChatMessage: notImplemented("sendChatMessage"),
    editUserMessage: notImplemented("editUserMessage"),
    regenerateAssistantMessage: notImplemented("regenerateAssistantMessage"),
    startPendingStream: async () => undefined,
    cancelStream: async () => undefined,

    refreshModels: notImplemented("refreshModels"),
    updateProvider: notImplemented("updateProvider"),
    getSecretStoreStatus: async () => ({
      available: true,
      code: "available",
      message: "Operating-system credential storage is available.",
    }),
    upsertProviderSecret: notImplemented("upsertProviderSecret"),
    getProviderSecretMetadata: async () => null,
    deleteProviderSecret: async () => undefined,
    pullOllamaModel: async () => undefined,
    deleteOllamaModel: async () => undefined,
    cancelOllamaPull: async () => undefined,

    listProjects: async () => [],
    createProject: notImplemented("createProject"),
    updateProject: notImplemented("updateProject"),
    setProjectArchived: notImplemented("setProjectArchived"),
    previewProjectDeletion: notImplemented("previewProjectDeletion"),
    deleteProject: async () => undefined,

    setConversationPersona: notImplemented("setConversationPersona"),
    listPersonas: async () => [],
    createPersona: notImplemented("createPersona"),
    updatePersona: notImplemented("updatePersona"),
    listPersonaVersions: async () => [],
    setPersonaArchived: notImplemented("setPersonaArchived"),
    previewPersonaDeletion: notImplemented("previewPersonaDeletion"),
    deletePersona: async () => undefined,

    attachTextFile: notImplemented("attachTextFile"),
    listConversationAttachments: async () => [],
    getAttachmentContent: notImplemented("getAttachmentContent"),
    deleteAttachment: async () => undefined,

    listTools: async () => [],
    grantToolCapability: notImplemented("grantToolCapability"),
    revokeToolCapability: async () => undefined,
    listToolAuditEvents: async () => [],
    verifyToolAuditTrail: async () => true,
    listConversationNotes: async () => [],
    previewNoteWrite: notImplemented("previewNoteWrite"),
    createNote: notImplemented("createNote"),
    updateNote: notImplemented("updateNote"),
    deleteNote: async () => undefined,

    runDiagnostics: notImplemented("runDiagnostics"),

    exportConversationMarkdown: notImplemented("exportConversationMarkdown"),
    exportConversationJson: notImplemented("exportConversationJson"),
    previewConversationImport: notImplemented("previewConversationImport"),
    importConversationJson: notImplemented("importConversationJson"),
    cancelImport: async () => undefined,

    exportWorkspaceJson: notImplemented("exportWorkspaceJson"),
    exportWorkspaceMarkdown: notImplemented("exportWorkspaceMarkdown"),
    previewWorkspaceImport: notImplemented("previewWorkspaceImport"),
    importWorkspaceJson: notImplemented("importWorkspaceJson"),

    getCompanionApiStatus: async () => ({
      enabled: false,
      running: false,
      port: null,
      tokenConfigured: false,
    }),
    setCompanionApiEnabled: notImplemented("setCompanionApiEnabled"),
    regenerateCompanionApiToken: notImplemented("regenerateCompanionApiToken"),

    getBuiltInRuntimeStatus: async () => ({
      running: false,
      binaryInstalled: false,
      binaryVerified: false,
      state: "unavailable_binary",
      failure: null,
    }),
    startBuiltInRuntime: notImplemented("startBuiltInRuntime"),
    stopBuiltInRuntime: async () => undefined,

    onStreamDelta: noopSubscribe,
    onStreamComplete: noopSubscribe,
    onStreamError: noopSubscribe,
    onStreamCancelled: noopSubscribe,
    onStreamInterrupted: noopSubscribe,
    onOllamaPullProgress: noopSubscribe,
    onImportProgress: noopSubscribe,
  };

  return { ...defaults, ...overrides };
}
