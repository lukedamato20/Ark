export type MessageStatus = "pending" | "streaming" | "complete" | "failed" | "cancelled";

export interface Conversation {
  id: string;
  title: string;
  createdAt: string;
  updatedAt: string;
  providerId?: string | null;
  modelId?: string | null;
  currentMessageId?: string | null;
  systemPrompt?: string | null;
  temperature?: number | null;
  maxTokens?: number | null;
  streamingEnabled: boolean;
  archived: boolean;
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

export interface ProviderConfig {
  id: string;
  name: string;
  providerType: string;
  baseUrl?: string | null;
  apiKeyRef?: string | null;
  defaultModelId?: string | null;
  defaultTemperature?: number | null;
  defaultMaxTokens?: number | null;
  streamingEnabled: boolean;
  isLocal: boolean;
  isEnabled: boolean;
  createdAt: string;
  updatedAt: string;
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
}

export interface AppBootstrap {
  conversations: Conversation[];
  providers: ProviderConfig[];
  models: ModelInfo[];
  workspacePath: string;
  workspace: WorkspaceInfo;
  theme: ThemeMode;
}

export interface WorkspaceInfo {
  rootPath: string;
  databasePath: string;
  defaultRootPath: string;
  configPath: string;
  isPortable: boolean;
  requiresRestart: boolean;
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
  guidance: string;
}

export interface BenchmarkResult {
  timeToFirstTokenMs?: number | null;
  totalTimeMs: number;
  approximateTokensPerSecond?: number | null;
  outputPreview: string;
}

export interface BuiltInRuntimeStatus {
  running: boolean;
  port?: number | null;
  modelPath?: string | null;
}

export interface AppErrorShape {
  code?: string;
  message?: string;
}
