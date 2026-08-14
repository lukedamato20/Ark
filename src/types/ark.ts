export type MessageStatus = "pending" | "streaming" | "complete" | "failed" | "cancelled" | "interrupted";

export interface Conversation {
  id: string;
  title: string;
  createdAt: string;
  updatedAt: string;
  providerId?: string | null;
  modelId?: string | null;
  currentMessageId?: string | null;
  /** FTR-004: null means "no conversation-level override, inherit the provider's default." */
  systemPrompt?: string | null;
  temperature?: number | null;
  maxTokens?: number | null;
  archived: boolean;
  /** ARC-007: populated once FTR-003 project assignment exists; null for current conversations. */
  projectId?: string | null;
  /** FTR-002: null means unpinned. An ISO timestamp (not a boolean) so pin order among
   * multiple pinned conversations is deterministic — most-recently-pinned first. */
  pinnedAt?: string | null;
}

export interface ConversationPage {
  items: Conversation[];
  nextCursor?: string | null;
  /** FTR-002: conversation id -> a short plain-text excerpt of the matching title/message
   * content. Present only for conversations a search query actually matched; empty when no
   * query was given. */
  searchSnippets: Record<string, string>;
}

export interface Message {
  id: string;
  conversationId: string;
  parentMessageId?: string | null;
  revisionOfMessageId?: string | null;
  pathIndex: number;
  role: "system" | "user" | "assistant" | "tool";
  content: string;
  status: MessageStatus;
  createdAt: string;
  updatedAt: string;
  providerId?: string | null;
  modelId?: string | null;
  tokenCount?: number | null;
  errorMessage?: string | null;
  metadataJson?: string | null;
}

export interface BranchAlternative {
  messageId: string;
  revisionOfMessageId?: string | null;
  createdAt: string;
  status: MessageStatus;
  contentPreview: string;
  isActive: boolean;
  hasDescendants: boolean;
}

export type DestinationClass = "loopback" | "private_lan" | "public";

/**
 * ARC-003: what a provider *type* (protocol) supports, independent of any one configured
 * instance — computed in Rust from `providerType` (see `ProviderCapabilities::for_provider_type`
 * in `src-tauri/src/providers/mod.rs`), never stored. UI affordances (e.g. showing a "pull
 * model" control) should check these flags rather than hardcoding assumptions per providerType.
 */
export interface ProviderCapabilities {
  streaming: boolean;
  modelListing: boolean;
  modelPull: boolean;
  modelDelete: boolean;
  modelUnload: boolean;
  requiresAuth: boolean;
  reportsContextWindow: boolean;
  vision: boolean;
  embeddings: boolean;
  tools: boolean;
}

export interface ProviderConfig {
  id: string;
  name: string;
  providerType: string;
  baseUrl?: string | null;
  apiKeyRef?: string | null;
  defaultModelId?: string | null;
  defaultTemperature?: number | null;
  defaultMaxTokens?: number | null;
  isLocal: boolean;
  /** SEC-001: explicit development-mode exception; never inferred from the URL in the UI. */
  allowInsecureRemote: boolean;
  /** SEC-001: computed in Rust from the provider's base URL — the single source of truth. */
  destinationClass: DestinationClass;
  capabilities: ProviderCapabilities;
  isEnabled: boolean;
  createdAt: string;
  updatedAt: string;
}

/** SEC-005: public credential state never includes the credential value. */
export interface SecretMetadata {
  id: string;
  masked: string;
  available: boolean;
}

export interface SecretStoreStatus {
  available: boolean;
  code: string;
  message: string;
}

export type WorkspaceProtectionMode = "plaintext" | "encrypted";

export interface WorkspaceProtectionStatus {
  mode: WorkspaceProtectionMode;
  locked: boolean;
  transitionInProgress: boolean;
  keyAvailable: boolean;
  message: string;
}

export interface WorkspaceProtectionChange {
  status: WorkspaceProtectionStatus;
  /** Present only once after enabling encryption or rotating the key. */
  recoveryKey?: string | null;
}

export interface ModelInfo {
  id: string;
  providerId: string;
  name: string;
  displayName?: string | null;
  contextWindow?: number | null;
  supportsStreaming: boolean;
  supportsTools: boolean;
  supportsVision: boolean;
  supportsEmbeddings: boolean;
  isAvailable: boolean;
  lastSeenAt?: string | null;
  metadataJson?: string | null;
  createdAt: string;
  updatedAt: string;
}

export interface ProviderHealth {
  providerId: string;
  isReachable: boolean;
  status: string;
  message: string;
  /** FTR-009: when this health check actually ran — an ISO timestamp, so the UI can show
   * "checked N ago" and distinguish a fresh result from state a newer refresh hasn't replaced yet. */
  checkedAt: string;
}

export interface AppBootstrap {
  conversationPage: ConversationPage;
  providers: ProviderConfig[];
  models: ModelInfo[];
  workspacePath: string;
  workspace: WorkspaceInfo;
  /** ARC-006: device-scoped (theme, built-in runtime model path) — see docs/settings-catalog.md. */
  deviceSettings: DeviceSettings;
  /** COR-010: present when the workspace database failed to open and this reflects a temporary in-memory fallback. */
  workspaceOpenError?: AppErrorShape | null;
}

/**
 * ARC-006: settings scoped to this device — never synced through the portable workspace
 * database. Persisted by the backend at the OS's per-user app-config directory, independent of
 * which workspace is currently open. See `src-tauri/src/device_settings.rs`.
 */
export interface DeviceSettings {
  theme: ThemeMode;
  builtInModelPath?: string | null;
  /** OPS-001: opt-in, off by default. Never transmitted anywhere automatically — see the
   * diagnostics bundle export flow for the only way crash/log data ever leaves the device. */
  crashCaptureEnabled: boolean;
}

export interface WorkspaceInfo {
  rootPath: string;
  databasePath: string;
  defaultRootPath: string;
  configPath: string;
  isPortable: boolean;
  requiresRestart: boolean;
}

export interface BackupManifest {
  appVersion: string;
  createdAt: string;
  databaseSha256: string;
  databaseSizeBytes: number;
}

export interface BackupResult {
  backupPath: string;
  manifest: BackupManifest;
}

export interface RestorePreview {
  manifest?: BackupManifest | null;
  detectedSchemaVersion: number;
  schemaSupported: boolean;
  conversationCount: number;
  messageCount: number;
}

export interface DiagnosticsBundle {
  generatedAt: string;
  /** The exact, already-redacted text a save writes verbatim — review this before saving. */
  previewText: string;
}

export type ThemeMode = "dark" | "light";

export interface SendChatResult {
  conversationId: string;
  userMessageId: string;
  assistantMessageId: string;
}

export interface StreamEvent {
  conversationId: string;
  messageId: string;
  delta?: string | null;
  content?: string | null;
  status: MessageStatus;
  error?: string | null;
  /** COR-002 (partial): monotonic per-message delta sequence; null on non-delta events. */
  revision?: number | null;
  /**
   * ARC-002: identifies which version of this event's shape the backend used. See
   * `KNOWN_STREAM_EVENT_SCHEMA_VERSION` in `lib/ArkClient.ts` and `STREAM_EVENT_SCHEMA_VERSION`
   * in `src-tauri/src/chat/mod.rs`.
   */
  schemaVersion: number;
}

export interface RefreshModelsResult {
  health: ProviderHealth;
  models: ModelInfo[];
  provider: ProviderConfig;
}

export interface DiagnosticsResult {
  os: string;
  cpu: string;
  cpuCores: number;
  totalMemoryBytes: number;
  availableMemoryBytes: number;
  totalDiskBytes: number;
  availableDiskBytes: number;
  gpu: string;
  providerHealth: ProviderHealth;
  modelAvailable: boolean;
  benchmark?: BenchmarkResult | null;
  benchmarkFailure?: AppErrorShape | null;
  guidance: string;
  runtime: RuntimeDiagnostics;
}

export type RuntimeLifecycleState =
  "stopped" | "starting" | "healthy" | "degraded" | "stopping" | "crashed" | "unavailable_binary" | "unavailable_model";

export type RuntimeFailureCategory =
  | "binary_unavailable"
  | "model_unavailable"
  | "port_unavailable"
  | "spawn_failed"
  | "authentication_failed"
  | "health_rejected"
  | "health_unreachable"
  | "readiness_timeout"
  | "process_exited"
  | "process_monitor_failed"
  | "stop_failed"
  | "state_unavailable"
  | "supply_chain_verification_failed";

export interface RuntimeFailure {
  category: RuntimeFailureCategory;
  message: string;
}

export interface RuntimeLogEntry {
  timestampMs: number;
  stream: string;
  message: string;
}

export interface RuntimeDiagnostics {
  state: RuntimeLifecycleState;
  pid?: number | null;
  port?: number | null;
  modelConfigured: boolean;
  failure?: RuntimeFailure | null;
  recentLogs: RuntimeLogEntry[];
}

export interface BenchmarkResult {
  timeToFirstTokenMs?: number | null;
  generationTimeMs?: number | null;
  totalTimeMs: number;
  approximateTokensPerSecond?: number | null;
  outputPreview: string;
}

export interface InstalledFileProvenance {
  name: string;
  sizeBytes: number;
  sha256: string;
}

export interface RuntimeProvenance {
  schemaVersion: number;
  runtime: string;
  version: string;
  sourceRepository: string;
  sourceCommit: string;
  license: string;
  licenseUrl: string;
  artifactFileName: string;
  artifactUrl: string;
  artifactSha256: string;
  runtimeSha256: string;
  platform: string;
  arch: string;
  verifiedAt: string;
  installedFiles: InstalledFileProvenance[];
}

export interface ModelProvenance {
  path: string;
  source: string;
  license: string;
  sha256: string;
  sizeBytes: number;
  verifiedAt: string;
}

export interface BuiltInRuntimeStatus {
  running: boolean;
  port?: number | null;
  modelPath?: string | null;
  /** COR-012: whether the llama-server binary is actually present on disk (not bundled by default). */
  binaryInstalled: boolean;
  binaryVerified: boolean;
  runtimeProvenance?: RuntimeProvenance | null;
  modelProvenance?: ModelProvenance | null;
  state: RuntimeLifecycleState;
  failure?: RuntimeFailure | null;
}

export interface AppErrorShape {
  code?: string;
  message?: string;
}

export interface ImportConversationResult {
  conversation: Conversation;
  normalizedMessageCount: number;
}

export interface ImportProviderMapping {
  sourceProviderId?: string | null;
  targetProviderId: string;
  reason: string;
}

export interface ImportConversationPreview {
  conversationCount: number;
  messageCount: number;
  maximumBranchDepth: number;
  normalizedMessageCount: number;
  conflicts: string[];
  providerMappings: ImportProviderMapping[];
  estimatedStorageBytes: number;
}

export interface ImportProgressEvent {
  importId: string;
  completedMessages: number;
  totalMessages: number;
}

export interface OllamaPullProgress {
  providerId: string;
  modelName: string;
  status: string;
  total?: number | null;
  completed?: number | null;
  digest?: string | null;
  error?: string | null;
}
