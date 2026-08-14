import type {
  AppBootstrap,
  BuiltInRuntimeStatus,
  Conversation,
  Message,
  ModelInfo,
  ProviderConfig,
  WorkspaceProtectionStatus,
} from "../types/ark";
import { createFakeArkClient, type ArkClient } from "./ArkClient";

/**
 * Development-only deterministic bridge fixture for browser-level UI verification. It is
 * selected only by `?fixture=runtime-provenance` in a Vite development build; Tauri and every
 * production build always use the native adapter.
 */
export function createRuntimeProvenanceFixtureClient(): ArkClient {
  const timestamp = "2026-08-14T06:24:26Z";
  const conversation: Conversation = {
    id: "fixture-conversation",
    title: "Runtime provenance review",
    createdAt: timestamp,
    updatedAt: timestamp,
    providerId: "built_in",
    modelId: "fixture-model",
    archived: false,
  };
  const provider: ProviderConfig = {
    id: "built_in",
    name: "Built-in llama.cpp",
    providerType: "built_in",
    baseUrl: "http://127.0.0.1:49152",
    defaultModelId: "fixture-model",
    defaultTemperature: 0.7,
    defaultMaxTokens: 2048,
    streamingEnabled: true,
    isLocal: true,
    allowInsecureRemote: false,
    destinationClass: "loopback",
    capabilities: {
      streaming: true,
      modelListing: true,
      modelPull: false,
      modelDelete: false,
      modelUnload: false,
      requiresAuth: true,
      reportsContextWindow: true,
      vision: false,
      embeddings: false,
      tools: false,
    },
    isEnabled: true,
    createdAt: timestamp,
    updatedAt: timestamp,
  };
  const model: ModelInfo = {
    id: "fixture-model",
    providerId: provider.id,
    name: "fixture-model.gguf",
    displayName: "Fixture model",
    contextWindow: 4096,
    supportsStreaming: true,
    supportsTools: false,
    supportsVision: false,
    supportsEmbeddings: false,
    isAvailable: true,
    lastSeenAt: timestamp,
    createdAt: timestamp,
    updatedAt: timestamp,
  };
  const status: BuiltInRuntimeStatus = {
    running: false,
    modelPath: "C:\\Models\\fixture-model.gguf",
    binaryInstalled: true,
    binaryVerified: true,
    state: "stopped",
    failure: null,
    runtimeProvenance: {
      schemaVersion: 1,
      runtime: "llama.cpp",
      version: "b9859",
      sourceRepository: "https://github.com/ggml-org/llama.cpp",
      sourceCommit: "4fc4ec5541b243957ae5099edb67372f8f3b550e",
      license: "MIT",
      licenseUrl: "https://raw.githubusercontent.com/ggml-org/llama.cpp/b9859/LICENSE",
      artifactFileName: "llama-b9859-bin-win-cpu-x64.zip",
      artifactUrl: "https://github.com/ggml-org/llama.cpp/releases/download/b9859/llama-b9859-bin-win-cpu-x64.zip",
      artifactSha256: "c9aa80f233a7d1749341860f11723b912d4cfd6eec19434c3d00bba0abc9f85c",
      runtimeSha256: "63c4371211e1d2c146e294d74cf5962dd2247c74573e5705f0817fb0b8f78054",
      platform: "win32",
      arch: "x64",
      verifiedAt: timestamp,
      installedFiles: [
        {
          name: "llama-server.exe",
          sizeBytes: 10_000_000,
          sha256: "63c4371211e1d2c146e294d74cf5962dd2247c74573e5705f0817fb0b8f78054",
        },
      ],
    },
    modelProvenance: {
      path: "C:\\Models\\fixture-model.gguf",
      source: "https://huggingface.co/example/fixture-model",
      license: "Apache-2.0",
      sha256: "a".repeat(64),
      sizeBytes: 4_200_000_000,
      verifiedAt: timestamp,
    },
  };
  const bootstrap: AppBootstrap = {
    conversationPage: { items: [conversation], nextCursor: null },
    providers: [provider],
    models: [model],
    workspacePath: "C:\\Ark",
    workspace: {
      rootPath: "C:\\Ark",
      databasePath: "C:\\Ark\\ark.sqlite3",
      defaultRootPath: "C:\\Ark",
      configPath: "C:\\Ark\\workspace.json",
      isPortable: false,
      requiresRestart: false,
    },
    deviceSettings: { theme: "dark", builtInModelPath: status.modelPath },
    workspaceOpenError: null,
  };

  return createFakeArkClient({
    getAppBootstrap: async () => bootstrap,
    getBuiltInRuntimeStatus: async () => status,
    getConversationMessages: async () => [],
    refreshModels: async () => ({
      health: { providerId: provider.id, isReachable: false, status: "stopped", message: "Runtime is stopped." },
      models: [model],
      provider,
    }),
    startBuiltInRuntime: async () => status,
  });
}

/** SEC-005 browser fixture: starts locked, becomes available on Retry, then supports masked CRUD. */
export function createSecretStoreFixtureClient(): ArkClient {
  const timestamp = "2026-08-14T12:00:00Z";
  const conversation: Conversation = {
    id: "fixture-secret-conversation",
    title: "Credential storage review",
    createdAt: timestamp,
    updatedAt: timestamp,
    providerId: "fixture-authenticated-provider",
    archived: false,
  };
  const provider: ProviderConfig = {
    id: "fixture-authenticated-provider",
    name: "Authenticated provider fixture",
    providerType: "local_inference_host",
    baseUrl: "https://provider.example.test/v1",
    apiKeyRef: null,
    defaultModelId: null,
    defaultTemperature: 0.7,
    defaultMaxTokens: 2048,
    streamingEnabled: true,
    isLocal: false,
    allowInsecureRemote: false,
    destinationClass: "public",
    capabilities: {
      streaming: true,
      modelListing: true,
      modelPull: false,
      modelDelete: false,
      modelUnload: false,
      requiresAuth: true,
      reportsContextWindow: false,
      vision: false,
      embeddings: false,
      tools: false,
    },
    isEnabled: true,
    createdAt: timestamp,
    updatedAt: timestamp,
  };
  const bootstrap: AppBootstrap = {
    conversationPage: { items: [conversation], nextCursor: null },
    providers: [provider],
    models: [],
    workspacePath: "C:\\Ark",
    workspace: {
      rootPath: "C:\\Ark",
      databasePath: "C:\\Ark\\ark.sqlite3",
      defaultRootPath: "C:\\Ark",
      configPath: "C:\\Ark\\workspace.json",
      isPortable: false,
      requiresRestart: false,
    },
    deviceSettings: { theme: "dark", builtInModelPath: null },
    workspaceOpenError: null,
  };
  let statusChecks = 0;
  let metadata: { id: string; masked: string; available: boolean } | null = null;

  return createFakeArkClient({
    getAppBootstrap: async () => bootstrap,
    getConversationMessages: async () => [],
    getSecretStoreStatus: async () => {
      statusChecks += 1;
      // React StrictMode intentionally runs the mount effect twice in development. Keep both
      // probes locked so the first user-initiated Retry is what transitions the fixture.
      return statusChecks <= 2
        ? {
            available: false,
            code: "secret_store_locked",
            message: "The operating-system credential store is locked. Unlock it and retry.",
          }
        : {
            available: true,
            code: "available",
            message: "Operating-system credential storage is available.",
          };
    },
    getProviderSecretMetadata: async () => metadata,
    upsertProviderSecret: async (_providerId, secret) => {
      if (!secret) throw new Error("Fixture requires a credential.");
      metadata = {
        id: "secret:v1:00000000-0000-4000-8000-000000000000",
        masked: "••••••••",
        available: true,
      };
      return metadata;
    },
    deleteProviderSecret: async () => {
      metadata = null;
    },
    refreshModels: async () => ({
      health: { providerId: provider.id, isReachable: false, status: "unavailable", message: "Fixture only." },
      models: [],
      provider,
    }),
  });
}

/**
 * SEC-006 browser fixture: starts plaintext, supports enable -> rotate -> forgotten-key restore
 * against an in-memory recovery key ledger, and rejects a stale or invalid recovery key exactly
 * the way the real `data_protection` module does.
 */
export function createWorkspaceProtectionFixtureClient(): ArkClient {
  const timestamp = "2026-08-14T12:00:00Z";
  const conversation: Conversation = {
    id: "fixture-protection-conversation",
    title: "Workspace encryption review",
    createdAt: timestamp,
    updatedAt: timestamp,
    providerId: "built_in",
    archived: false,
  };
  const bootstrap: AppBootstrap = {
    conversationPage: { items: [conversation], nextCursor: null },
    providers: [],
    models: [],
    workspacePath: "C:\\Ark",
    workspace: {
      rootPath: "C:\\Ark",
      databasePath: "C:\\Ark\\ark.sqlite3",
      defaultRootPath: "C:\\Ark",
      configPath: "C:\\Ark\\workspace.json",
      isPortable: false,
      requiresRestart: false,
    },
    deviceSettings: { theme: "dark", builtInModelPath: null },
    workspaceOpenError: null,
  };

  let mode: "plaintext" | "encrypted" = "plaintext";
  let locked = false;
  let currentRecoveryKey: string | null = null;
  let rotationCount = 0;

  function status(message: string): WorkspaceProtectionStatus {
    return { mode, locked, transitionInProgress: false, keyAvailable: mode === "encrypted" && !locked, message };
  }

  return createFakeArkClient({
    getAppBootstrap: async () => bootstrap,
    getConversationMessages: async () => [],
    getWorkspaceProtectionStatus: async () =>
      status(
        mode === "plaintext"
          ? "Workspace is plaintext SQLite."
          : locked
            ? "The encrypted workspace key is unavailable. Unlock the OS credential store or restore the recovery key; Ark cannot reset forgotten keys."
            : "Workspace is encrypted with SQLCipher; its key is protected by the operating-system credential store.",
      ),
    enableWorkspaceEncryption: async () => {
      mode = "encrypted";
      locked = false;
      rotationCount += 1;
      currentRecoveryKey = `ark-recovery-v1:fixture-key-${rotationCount}`;
      return { status: status("Workspace encrypted."), recoveryKey: currentRecoveryKey };
    },
    rotateWorkspaceEncryption: async () => {
      rotationCount += 1;
      currentRecoveryKey = `ark-recovery-v1:fixture-key-${rotationCount}`;
      return { status: status("Workspace key rotated."), recoveryKey: currentRecoveryKey };
    },
    disableWorkspaceEncryption: async () => {
      mode = "plaintext";
      locked = false;
      currentRecoveryKey = null;
      return status("Workspace decrypted.");
    },
    restoreWorkspaceRecoveryKey: async (recoveryKey) => {
      if (recoveryKey !== currentRecoveryKey) {
        throw {
          code: "workspace_recovery_key_invalid",
          message:
            "That recovery key does not unlock this workspace. Ark left the database and credential store untouched.",
        };
      }
      locked = false;
      return status("Workspace unlocked with the recovery key.");
    },
  });
}

/**
 * UX-003 browser fixture: a long, scrollable conversation (alternating short user prompts and
 * long assistant responses with a fenced code block, well past one viewport) for verifying
 * near-bottom auto-follow, reading-position preservation, and the jump-to-latest control live —
 * the other fixtures above all return an empty message list, which cannot exercise any of that.
 */
export function createLongConversationFixtureClient(): ArkClient {
  const timestamp = "2026-08-14T06:24:26Z";
  const conversation: Conversation = {
    id: "fixture-long-conversation",
    title: "Long conversation scroll fixture",
    createdAt: timestamp,
    updatedAt: timestamp,
    providerId: "built_in",
    modelId: "fixture-model",
    archived: false,
  };
  const provider: ProviderConfig = {
    id: "built_in",
    name: "Built-in llama.cpp",
    providerType: "built_in",
    baseUrl: "http://127.0.0.1:49152",
    defaultModelId: "fixture-model",
    defaultTemperature: 0.7,
    defaultMaxTokens: 2048,
    streamingEnabled: true,
    isLocal: true,
    allowInsecureRemote: false,
    destinationClass: "loopback",
    capabilities: {
      streaming: true,
      modelListing: true,
      modelPull: false,
      modelDelete: false,
      modelUnload: false,
      requiresAuth: true,
      reportsContextWindow: true,
      vision: false,
      embeddings: false,
      tools: false,
    },
    isEnabled: true,
    createdAt: timestamp,
    updatedAt: timestamp,
  };
  const model: ModelInfo = {
    id: "fixture-model",
    providerId: provider.id,
    name: "fixture-model.gguf",
    displayName: "Fixture model",
    contextWindow: 4096,
    supportsStreaming: true,
    supportsTools: false,
    supportsVision: false,
    supportsEmbeddings: false,
    isAvailable: true,
    lastSeenAt: timestamp,
    createdAt: timestamp,
    updatedAt: timestamp,
  };

  const messageCount = 24;
  const messages: Message[] = Array.from({ length: messageCount }, (_, index) => {
    const isUser = index % 2 === 0;
    const pairIndex = Math.floor(index / 2) + 1;
    return {
      id: `fixture-message-${index}`,
      conversationId: conversation.id,
      parentMessageId: index === 0 ? null : `fixture-message-${index - 1}`,
      revisionOfMessageId: null,
      pathIndex: index + 1,
      role: isUser ? "user" : "assistant",
      content: isUser
        ? `Question ${pairIndex}: can you explain point ${pairIndex} in more depth?`
        : `Response ${pairIndex}. Here is a longer explanation with several sentences of prose ` +
          `so the assistant bubble has realistic reading width, followed by a code sample.\n\n` +
          "```ts\n" +
          `function pointExample${pairIndex}(input: number): number {\n` +
          `  // A representative multi-line snippet wide enough to exercise the code block's\n` +
          `  // own internal horizontal scroll rather than shrinking the surrounding bubble.\n` +
          `  return input * ${pairIndex} + Math.floor(Math.random() * ${pairIndex + 1});\n` +
          `}\n` +
          "```",
      status: "complete" as const,
      createdAt: timestamp,
      updatedAt: timestamp,
      providerId: provider.id,
      modelId: model.name,
    };
  });

  const bootstrap: AppBootstrap = {
    conversationPage: { items: [conversation], nextCursor: null },
    providers: [provider],
    models: [model],
    workspacePath: "C:\\Ark",
    workspace: {
      rootPath: "C:\\Ark",
      databasePath: "C:\\Ark\\ark.sqlite3",
      defaultRootPath: "C:\\Ark",
      configPath: "C:\\Ark\\workspace.json",
      isPortable: false,
      requiresRestart: false,
    },
    deviceSettings: { theme: "dark", builtInModelPath: null },
    workspaceOpenError: null,
  };

  return createFakeArkClient({
    getAppBootstrap: async () => bootstrap,
    getConversationMessages: async () => messages,
    refreshModels: async () => ({
      health: { providerId: provider.id, isReachable: true, status: "running", message: "Runtime is running." },
      models: [model],
      provider,
    }),
  });
}

/**
 * UX-004 browser fixture: `getAppBootstrap` rejects every call, exercising `App.tsx`'s
 * `BootstrapFailurePanel` — the total-bootstrap-failure recovery state, distinct from the
 * partial `workspaceOpenError` case the other fixtures' `bootstrap` objects can carry.
 */
export function createBootstrapFailureFixtureClient(): ArkClient {
  return createFakeArkClient({
    getAppBootstrap: async () => {
      throw { code: "ipc_unavailable", message: "Ark could not reach its local runtime." };
    },
    getBuiltInRuntimeStatus: async () => {
      throw { code: "ipc_unavailable", message: "Ark could not reach its local runtime." };
    },
  });
}
