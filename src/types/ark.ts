export type MessageStatus = "pending" | "streaming" | "complete" | "failed" | "cancelled" | "interrupted";

/** UX: an Ark-level behavioral preset, distinct from a real provider parameter like temperature —
 * composed into a fixed instruction sentence appended to the resolved system prompt (see
 * `generation.rs`'s `response_style_instruction`). Not every low-level parameter a provider might
 * support — a deliberately small, human-readable set. */
export type ResponseStyle = "balanced" | "concise" | "detailed" | "explanatory" | "technical" | "creative";

/** UX: mirrors `ResponseStyle` for tone — see `generation.rs`'s `tone_instruction`. */
export type Tone = "neutral" | "professional" | "friendly" | "direct" | "casual";

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
  /** FTR-003: null means unassigned. Set via `setConversationProject`. */
  projectId?: string | null;
  /** FTR-002: null means unpinned. An ISO timestamp (not a boolean) so pin order among
   * multiple pinned conversations is deterministic — most-recently-pinned first. */
  pinnedAt?: string | null;
  /** FTR-003: null means unassigned. Independent of `projectId` — a project groups
   * conversations by subject, a persona defines how the assistant behaves. Set via
   * `setConversationPersona`. */
  personaId?: string | null;
  /** UX: `null` means "no conversation-level override, inherit persona/project." */
  responseStyle?: ResponseStyle | null;
  tone?: Tone | null;
}

/** FTR-003: groups conversations under a shared name, instructions, and default
 * provider/model/temperature/max_tokens. See `generation.rs`'s precedence chain (request ->
 * conversation -> project -> provider default) for how these defaults actually take effect. */
export interface Project {
  id: string;
  name: string;
  /** CODE-003: canonical codebase bound for Ark Code. This is distinct from Ark's storage Workspace. */
  repositoryPath?: string | null;
  /** `null` means no project-level instructions are injected. */
  instructions?: string | null;
  defaultProviderId?: string | null;
  defaultModelId?: string | null;
  defaultTemperature?: number | null;
  defaultMaxTokens?: number | null;
  /** UX: a default for every conversation assigned to this project — resolves at the same tier
   * as `instructions`. */
  responseStyle?: ResponseStyle | null;
  tone?: Tone | null;
  /** `null` means active. An ISO timestamp, matching `Conversation.pinnedAt`'s convention. */
  archivedAt?: string | null;
  createdAt: string;
  updatedAt: string;
}

/** FTR-003: what deleting a project would affect, shown before the user confirms — deletion
 * never deletes conversations, only unassigns them, but the count still needs surfacing. */
export interface ProjectDeletionPreview {
  project: Project;
  conversationCount: number;
  /** Attachments stay with their retained conversations; this count makes that explicit. */
  attachmentCount: number;
}

/** FTR-003: a reusable, named instruction identity a conversation can be assigned to,
 * independent of any project — a project groups conversations by subject, a persona defines
 * how the assistant behaves. `instructions` is always the *current* version's content; editing
 * it creates a new immutable version rather than rewriting history (see `versionNumber`). */
export interface Persona {
  id: string;
  name: string;
  instructions: string;
  defaultTemperature?: number | null;
  defaultMaxTokens?: number | null;
  /** UX: versioned alongside `instructions`/the defaults above — changing it creates a new
   * immutable version, same as changing `instructions`. */
  responseStyle?: ResponseStyle | null;
  tone?: Tone | null;
  /** Which version this is — increments only when `instructions`/the defaults actually change,
   * not on a plain rename. */
  versionNumber: number;
  archivedAt?: string | null;
  createdAt: string;
  updatedAt: string;
}

/** FTR-003: one entry in a persona's version history — `listPersonaVersions`. */
export interface PersonaVersionSummary {
  id: string;
  versionNumber: number;
  instructions: string;
  defaultTemperature?: number | null;
  defaultMaxTokens?: number | null;
  responseStyle?: ResponseStyle | null;
  tone?: Tone | null;
  createdAt: string;
}

/** FTR-003: what deleting a persona would affect, shown before the user confirms — mirrors
 * `ProjectDeletionPreview` exactly. */
export interface PersonaDeletionPreview {
  persona: Persona;
  conversationCount: number;
}

/** CMP-001: a text file attached to an outgoing message. Never carries its content — that's
 * fetched on demand via `getAttachmentContent` — so loading a conversation's attachment list
 * doesn't re-send potentially-large text bodies for every row. `messageId: null` means staged
 * (uploaded, not yet sent) — the "preview/remove before send" state. */
export interface Attachment {
  id: string;
  conversationId: string;
  messageId?: string | null;
  fileName: string;
  byteSize: number;
  sha256: string;
  createdAt: string;
}

/** CMP-003: SEC-009's capability-scope contract, made real by the "notes" tool. `tier` is always
 * `"chat_safe"` today — `"repository_execution"` exists for Ark Code's future CODE-004/CODE-005,
 * never reachable from Ark Chat. */
export interface CapabilityScope {
  tier: "chat_safe" | "repository_execution";
  read: boolean;
  write: boolean;
  network: boolean;
  secret: boolean;
  /** Which data this scope actually covers — a human-readable description, not just the axis. */
  data: string;
}

/** CMP-003: a tool's declared identity and scope, shown before any grant exists — the
 * install/connect-style publisher/source/scope/trust disclosure, applied to a built-in tool. */
export interface ToolDefinition {
  id: string;
  name: string;
  description: string;
  publisher: string;
  scope: CapabilityScope;
}

/** CMP-003: a persisted capability grant. `id` identifies the grant row itself — a tool can be
 * granted, expire, and be re-granted many times, each a distinct row. */
export interface ToolCapabilityGrant {
  id: string;
  toolId: string;
  tier: "chat_safe" | "repository_execution";
  read: boolean;
  write: boolean;
  network: boolean;
  secret: boolean;
  data: string;
  grantedAt: string;
  expiresAt: string;
  revoked: boolean;
}

/** CMP-003: a tool's current status for the Tools panel — its definition plus whichever grant (if
 * any) currently governs it, valid or not, so the UI can show *why* the next write will ask for
 * approval again. */
export interface ToolStatus {
  definition: ToolDefinition;
  activeGrant?: ToolCapabilityGrant | null;
}

export type RepositoryEntryKind = "directory" | "file" | "symlink";

/** CODE-004: one bounded, ignore-aware entry beneath a Project's bound Repository. */
export interface RepositoryEntry {
  path: string;
  kind: RepositoryEntryKind;
  byteSize?: number | null;
  contextEligible: boolean;
}

export interface RepositoryDirectoryListing {
  path: string;
  entries: RepositoryEntry[];
  truncated: boolean;
}

export interface RepositoryFileRead {
  path: string;
  startLine: number;
  endLine: number;
  totalLines: number;
  content: string;
  sha256: string;
  truncated: boolean;
  nextStartLine?: number | null;
}

export interface RepositorySearchMatch {
  path: string;
  lineNumber: number;
  line: string;
}

export interface RepositorySearchResult {
  matches: RepositorySearchMatch[];
  filesScanned: number;
  bytesScanned: number;
  skippedFiles: number;
  truncated: boolean;
}

export interface RepositoryMap {
  entries: RepositoryEntry[];
  inspectedFiles: number;
  skippedFiles: number;
  truncated: boolean;
}

export interface RepositoryGitStatus {
  clean: boolean;
  porcelain: string;
}

export interface RepositoryGitDiff {
  workingTree: string;
  staged: string;
}

export interface CodeRepositorySupport {
  repositoryMap: RepositoryMap;
  gitStatus: RepositoryGitStatus;
  gitDiff: RepositoryGitDiff;
}

/** CODE-005: one search/replace block. `search` must match the target file's current content
 * exactly once (checked sequentially against each prior block's result within the same call). */
export interface EditBlock {
  search: string;
  replace: string;
}

/** CODE-005: an `edit_file` proposal. `callHash`/`previewHash`/`preconditionHash` bind an
 * approval to this exact change — they must be echoed back unchanged to `executeEditFile`, which
 * re-derives all three from current Repository state and refuses if any no longer match. */
export interface EditFilePreview {
  path: string;
  diff: string;
  beforeHash: string;
  expectedAfterHash: string;
  callHash: string;
  previewHash: string;
  preconditionHash: string;
}

/** CODE-005: the result of an approved `edit_file` execution, classified into exactly one of the
 * ADR 0003 file-verifier's recovery outcomes. `diverged` is never auto-corrected. */
export interface EditFileOutcome {
  path: string;
  beforeHash: string;
  expectedAfterHash: string;
  observedAfterHash: string;
  outcome: CodeRecoveryOutcome;
}

export type CodeRunState =
  | "queued"
  | "planning"
  | "awaiting_approval"
  | "executing_tool"
  | "observing"
  | "completed"
  | "failed"
  | "cancelled"
  | "interrupted";

export type CodeRecoveryOutcome = "applied" | "not_applied" | "diverged" | "unknown";

/** CODE-007: one persistent Ark Code thread, owned by an existing Project. */
export interface CodeSession {
  id: string;
  projectId: string;
  title: string;
  archived: boolean;
  createdAt: string;
  updatedAt: string;
}

/** CODE-005: an exact local-user command template. Models receive only its ID and label. */
export interface CodeCommandDefinition {
  id: string;
  label: string;
  program: string;
  arguments: string[];
  timeoutSeconds: number;
  enabled: boolean;
  createdAt: string;
  updatedAt: string;
}

/** ADR 0003: an immutable run attempt. Resume/retry creates a child run. */
export interface CodeAgentRun {
  id: string;
  sessionId: string;
  parentRunId?: string | null;
  providerId: string;
  modelId: string;
  /** CODE-007: what Ark Code was asked to investigate. Immutable once the run is created. */
  task: string;
  repositoryPathSnapshot: string;
  repositoryIdentityHash: string;
  state: CodeRunState;
  maxSteps: number;
  maxActiveMs: number;
  maxTokens: number;
  maxCostMicrounits?: number | null;
  stepsUsed: number;
  activeElapsedMs: number;
  reservedTokens: number;
  actualTokens: number;
  actualCostMicrounits?: number | null;
  cancelRequestedAt?: string | null;
  terminalReason?: string | null;
  recoveryOutcome?: CodeRecoveryOutcome | null;
  createdAt: string;
  updatedAt: string;
  completedAt?: string | null;
}

export interface CodeRunEvent {
  runId: string;
  sequence: number;
  schemaVersion: number;
  kind: string;
  state: CodeRunState;
  summary: string;
  createdAt: string;
}

/** Refetch notification only; `CodeRunDetail` remains authoritative. */
export interface CodeRunUpdatedEvent {
  runId: string;
  sessionId: string;
  sequence: number;
  schemaVersion: number;
  state: CodeRunState;
}

export interface CodeSessionDetail {
  session: CodeSession;
  runs: CodeAgentRun[];
  events: CodeRunEvent[];
}

export type CodeAgentStepState = "reserved" | "dispatched" | "completed" | "failed" | "interrupted";

/** CODE-007: one planning/model turn of a run's synchronous read-only agent loop. */
export interface CodeAgentStep {
  id: string;
  runId: string;
  stepIndex: number;
  state: CodeAgentStepState;
  reservedTokens: number;
  actualTokens?: number | null;
  streamingText?: string | null;
  createdAt: string;
}

export type CodeToolInvocationState =
  "proposed" | "approved" | "executing" | "applied" | "failed" | "denied" | "interrupted";

/** CODE-007: at most one per step in this pass's loop — the tool call the model requested and
 * whether it applied. */
export interface CodeToolInvocation {
  id: string;
  runId: string;
  stepId: string;
  toolName: string;
  canonicalArgumentsJson: string;
  callHash: string;
  state: CodeToolInvocationState;
  preview?: string | null;
  previewHash?: string | null;
  preconditionHash?: string | null;
  approvedAt?: string | null;
  verificationOutcome?: CodeRecoveryOutcome | null;
  createdAt: string;
}

export type CodeObservationKind =
  | "tool_result"
  | "tool_error"
  | "model_text"
  | "system"
  | "completion_rejected";

export interface CodeObservation {
  id: string;
  runId: string;
  stepId: string;
  kind: CodeObservationKind;
  content: string;
  createdAt: string;
}

/** CODE-007: everything `CodeView` needs to render one run's autonomous progress. */
export interface CodeRunDetail {
  run: CodeAgentRun;
  steps: CodeAgentStep[];
  invocations: CodeToolInvocation[];
  observations: CodeObservation[];
  events: CodeRunEvent[];
}

/** CMP-003: the built-in "notes" tool's own data — a short scratch note attached to a
 * conversation. */
export interface ConversationNote {
  id: string;
  conversationId: string;
  content: string;
  createdAt: string;
  updatedAt: string;
}

export type NoteWriteAction = "create" | "update" | "delete";

/** CMP-004: one Brave Search result surfaced through the web_search tool. */
export interface SearchCitation {
  title: string;
  url: string;
  snippet: string;
}

export interface WebSearchResult {
  citations: SearchCitation[];
}

/** CMP-004: already-fetched search results (from `searchWeb`) threaded into `sendChatMessage` as
 * plain data — the frontend performs the search before sending, since a network call cannot
 * happen inside the backend's send-message transaction. */
export interface WebSearchInput {
  query: string;
  citations: SearchCitation[];
}

/** CMP-003: the human-readable preview shown before a side-effecting tool call runs, unless a
 * still-valid narrow grant already covers it. */
export interface SideEffectPreview {
  toolId: string;
  summary: string;
  idempotency: "idempotent" | "requires_fresh_approval";
}

/** CMP-003: one entry in SEC-009's persisted, hash-chained, tamper-evident audit trail. */
export interface AuditEvent {
  sequence: number;
  timestamp: string;
  kind: "granted" | "revoked" | "invoked" | "approval_requested" | "approval_denied";
  toolId: string;
  redactedDetail: string;
  chainHash: string;
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
  /** FTR-005: `null` means unnamed — the frontend falls back to an ordinal "Response N" label. */
  branchName?: string | null;
}

/** PERF-003: the bounded response `getConversationMessages` returns — see
 * `Database::get_active_messages_page`'s doc comment on the Rust side for what "bounded" means. */
export interface ConversationMessagePage {
  messages: Message[];
  /** `true` when the oldest message in `messages` still has a parent that wasn't loaded — shows
   * the "Load earlier messages" affordance. */
  hasMoreOlder: boolean;
}

export interface BranchAlternative {
  messageId: string;
  revisionOfMessageId?: string | null;
  createdAt: string;
  status: MessageStatus;
  contentPreview: string;
  isActive: boolean;
  hasDescendants: boolean;
  branchName?: string | null;
}

/** FTR-005: compact whole-conversation topology node; full content stays behind `getMessage`. */
export interface BranchTopologyNode {
  messageId: string;
  parentMessageId?: string | null;
  revisionOfMessageId?: string | null;
  pathIndex: number;
  role: Message["role"];
  createdAt: string;
  status: MessageStatus;
  contentPreview: string;
  isActive: boolean;
  branchName?: string | null;
  providerId?: string | null;
  modelId?: string | null;
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
  /** True for providers created by the user and therefore eligible for deletion. */
  isUserManaged: boolean;
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

export type ToolCallingMode = "native" | "prompted" | "unsupported";

export interface ModelInfo {
  id: string;
  providerId: string;
  name: string;
  displayName?: string | null;
  contextWindow?: number | null;
  supportsStreaming: boolean;
  supportsTools: boolean;
  toolCallingMode: ToolCallingMode;
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
  /** FTR-003: every project (active and archived) — expected to stay small, unpaginated like
   * `providers`. */
  projects: Project[];
  /** FTR-003: every persona (active and archived), each already carrying its current version's
   * content — mirrors `projects` exactly. */
  personas: Persona[];
  /** FTR-003: portable workspace-wide fallback instructions. */
  applicationInstructions: string | null;
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
  accentPalette: AccentPalette;
  builtInModelPath?: string | null;
  /** FTR-006: absolute device-local override for catalog-managed GGUF storage. Null uses Ark's
   * per-user application-data models directory. */
  managedModelDirectory?: string | null;
  /** OPS-001: opt-in, off by default. Never transmitted anywhere automatically — see the
   * diagnostics bundle export flow for the only way crash/log data ever leaves the device. */
  crashCaptureEnabled: boolean;
  /** CMP-006: opt-in, off by default. When true, a generation that completes, fails, or is
   * interrupted while the main window is unfocused shows a generic native OS notification. */
  completionNotificationsEnabled: boolean;
  /** PERF-001: opt-in, off by default. When true, local performance metrics (durations/counts
   * only, never content) are recorded into the same local diagnostics log crash capture uses,
   * and appear in the diagnostics bundle's "Recent performance metrics" section. */
  perfMetricsEnabled: boolean;
}

export type AccentPalette = "blue" | "violet" | "teal" | "amber" | "graphite";

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

export interface DiskSpaceInfo {
  totalBytes: number;
  availableBytes: number;
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
  /** COR-012/FTR-006: whether the verified development or packaged llama-server resource exists. */
  binaryInstalled: boolean;
  binaryVerified: boolean;
  runtimeProvenance?: RuntimeProvenance | null;
  modelProvenance?: ModelProvenance | null;
  state: RuntimeLifecycleState;
  failure?: RuntimeFailure | null;
}

export interface ManagedModelCompatibility {
  runtime: string;
  runtimeVersion: string;
  format: string;
  platforms: string[];
}

export interface ManagedModelCatalogEntry {
  id: string;
  displayName: string;
  publisher: string;
  description: string;
  sourceRepository: string;
  sourceCommit: string;
  downloadUrl: string;
  allowedDownloadHostSuffixes: string[];
  fileName: string;
  sizeBytes: number;
  sha256: string;
  license: string;
  licenseUrl: string;
  quantization: string;
  contextWindow: number;
  architecture: string;
  parameterCount: string;
  minimumAvailableMemoryBytes: number;
  recommendedAvailableMemoryBytes: number;
  compatibility: ManagedModelCompatibility;
}

export interface ManagedModelStatus {
  model: ManagedModelCatalogEntry;
  storageDirectory: string;
  modelPath: string;
  installed: boolean;
  verified: boolean;
  partialBytes: number;
}

export type ManagedModelOperation = "download" | "load";
export type HardwareFitRisk = "safe" | "warning" | "blocked";

export interface ManagedModelPreflight {
  modelId: string;
  operation: ManagedModelOperation;
  risk: HardwareFitRisk;
  availableMemoryBytes: number;
  minimumAvailableMemoryBytes: number;
  recommendedAvailableMemoryBytes: number;
  availableDiskBytes: number;
  requiredDiskBytes: number;
  advisories: string[];
  advancedOverrideAllowed: boolean;
}

export interface HardwareFitEvidence {
  totalMemoryBytes: number;
  availableMemoryBytes: number;
  executionDevice: "local_device";
  acceleratorMemoryBytes?: number | null;
  methodVersion: "ark-fit-v1";
}

export interface ManagedModelDownloadProgress {
  schemaVersion: number;
  modelId: string;
  status: string;
  completedBytes: number;
  totalBytes: number;
  resumed: boolean;
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

export interface WorkspaceImportPreviewEntry {
  conversationId: string;
  title: string;
  messageCount: number;
  attachmentCount: number;
  duplicateOfLocalId?: string | null;
}

export interface WorkspaceImportPreview {
  scope: string;
  entries: WorkspaceImportPreviewEntry[];
  providerMappings: ImportProviderMapping[];
}

export interface WorkspaceImportResult {
  importedCount: number;
  skippedCount: number;
}

export interface CompanionApiStatus {
  enabled: boolean;
  running: boolean;
  port?: number | null;
  tokenConfigured: boolean;
}

export interface CompanionApiTokenReveal {
  token: string;
  status: CompanionApiStatus;
}
