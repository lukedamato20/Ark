import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { openUrl } from "@tauri-apps/plugin-opener";
import type {
  AppBootstrap,
  BackupResult,
  BranchAlternative,
  BuiltInRuntimeStatus,
  Conversation,
  ConversationPage,
  DeviceSettings,
  DiagnosticsBundle,
  DiagnosticsResult,
  ImportConversationPreview,
  ImportConversationResult,
  ImportProgressEvent,
  Message,
  OllamaPullProgress,
  ProviderConfig,
  RefreshModelsResult,
  RestorePreview,
  SendChatResult,
  SecretMetadata,
  SecretStoreStatus,
  StreamEvent,
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
  deleteConversation(id: string): Promise<void>;
  getConversationMessages(conversationId: string): Promise<Message[]>;
  getAssistantAlternatives(conversationId: string, messageId: string): Promise<BranchAlternative[]>;
  switchActiveBranch(conversationId: string, messageId: string): Promise<Message[]>;
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

  runDiagnostics(providerId: string, model?: string | null, includeRuntimeLogs?: boolean): Promise<DiagnosticsResult>;

  exportConversationMarkdown(conversationId: string): Promise<string>;
  exportConversationJson(conversationId: string): Promise<string>;
  previewConversationImport(json: string): Promise<ImportConversationPreview>;
  importConversationJson(importId: string, json: string): Promise<ImportConversationResult>;
  cancelImport(importId: string): Promise<void>;

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
    deleteConversation: (id) => invoke<void>("delete_conversation", { id }),
    getConversationMessages: (conversationId) => invoke<Message[]>("get_conversation_messages", { conversationId }),
    getAssistantAlternatives: (conversationId, messageId) =>
      invoke<BranchAlternative[]>("get_assistant_alternatives", { request: { conversationId, messageId } }),
    switchActiveBranch: (conversationId, messageId) =>
      invoke<Message[]>("switch_active_branch", { request: { conversationId, messageId } }),
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
    deleteConversation: async () => undefined,
    getConversationMessages: async () => [],
    getAssistantAlternatives: async () => [],
    switchActiveBranch: notImplemented("switchActiveBranch"),
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

    runDiagnostics: notImplemented("runDiagnostics"),

    exportConversationMarkdown: notImplemented("exportConversationMarkdown"),
    exportConversationJson: notImplemented("exportConversationJson"),
    previewConversationImport: notImplemented("previewConversationImport"),
    importConversationJson: notImplemented("importConversationJson"),
    cancelImport: async () => undefined,

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
