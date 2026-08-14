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
    conversationPage: { items: [conversation], nextCursor: null, searchSnippets: {} },
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
    deviceSettings: { theme: "dark", builtInModelPath: status.modelPath, crashCaptureEnabled: false },
    workspaceOpenError: null,
  };

  return createFakeArkClient({
    getAppBootstrap: async () => bootstrap,
    getBuiltInRuntimeStatus: async () => status,
    getConversationMessages: async () => [],
    refreshModels: async () => ({
      health: {
        providerId: provider.id,
        isReachable: false,
        status: "stopped",
        message: "Runtime is stopped.",
        checkedAt: new Date().toISOString(),
      },
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
    conversationPage: { items: [conversation], nextCursor: null, searchSnippets: {} },
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
    deviceSettings: { theme: "dark", builtInModelPath: null, crashCaptureEnabled: false },
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
      health: {
        providerId: provider.id,
        isReachable: false,
        status: "unavailable",
        message: "Fixture only.",
        checkedAt: new Date().toISOString(),
      },
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
    conversationPage: { items: [conversation], nextCursor: null, searchSnippets: {} },
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
    deviceSettings: { theme: "dark", builtInModelPath: null, crashCaptureEnabled: false },
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
    modelId: "fixture-model.gguf",
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
    conversationPage: { items: [conversation], nextCursor: null, searchSnippets: {} },
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
    deviceSettings: { theme: "dark", builtInModelPath: null, crashCaptureEnabled: false },
    workspaceOpenError: null,
  };

  return createFakeArkClient({
    getAppBootstrap: async () => bootstrap,
    getConversationMessages: async () => messages,
    refreshModels: async () => ({
      health: {
        providerId: provider.id,
        isReachable: true,
        status: "running",
        message: "Runtime is running.",
        checkedAt: new Date().toISOString(),
      },
      models: [model],
      provider,
    }),
    updateDeviceSettings: async (settings) => settings,
    // FTR-004: every other fixture's updateConversationSettings is unimplemented — this is the
    // only way to exercise the conversation-settings panel's save flow live.
    updateConversationSettings: async (input) => ({
      ...conversation,
      systemPrompt: input.systemPrompt ?? null,
      temperature: input.temperature ?? null,
      maxTokens: input.maxTokens ?? null,
      updatedAt: new Date().toISOString(),
    }),
    // OPS-001: a static but realistic-looking bundle — every other fixture's diagnostics-bundle
    // methods are unimplemented, so this is the only way to exercise the review/save UI live.
    exportDiagnosticsBundle: async () => ({
      generatedAt: "2026-08-14T00:00:00Z",
      previewText:
        "Ark diagnostics bundle — generated 2026-08-14T00:00:00Z\n" +
        "App version: 0.1.0\nOS: Windows 11\nCPU: Fixture CPU (8 cores)\n" +
        "Memory: 8000000000 / 16000000000 bytes available\n" +
        "Workspace location: [REDACTED_PATH]\n\n" +
        "-- Managed runtime --\nState: Healthy\nPID present: true\nPort: 51234\nFailure: none\n\n" +
        "-- Recent runtime log lines --\n(none)\n\n-- Recent app log lines --\n" +
        "1755100000000 [info] runtime (-) managed runtime became healthy\n",
    }),
    saveDiagnosticsBundle: async () => undefined,
    // UX-004: minimal import overrides so the terminal-summary toast and its auto-dismiss timer
    // can be exercised live — every other fixture's import methods are unimplemented.
    previewConversationImport: async () => ({
      conversationCount: 1,
      messageCount: 3,
      maximumBranchDepth: 1,
      normalizedMessageCount: 0,
      conflicts: [],
      providerMappings: [],
      estimatedStorageBytes: 2048,
    }),
    importConversationJson: async () => ({
      conversation: { ...conversation, id: "fixture-imported-conversation", title: "Imported fixture conversation" },
      normalizedMessageCount: 0,
    }),
    // UX-010: a failed-benchmark diagnostics result — every other fixture's runDiagnostics is
    // unimplemented, so this is the only way to exercise the benchmarkFailure UI live.
    runDiagnostics: async () => ({
      os: "Windows 11",
      cpu: "Fixture CPU",
      cpuCores: 8,
      totalMemoryBytes: 34_359_738_368,
      availableMemoryBytes: 17_179_869_184,
      totalDiskBytes: 512_110_190_592,
      availableDiskBytes: 128_027_547_648,
      gpu: "GPU/accelerator detection is not available in the MVP diagnostics.",
      providerHealth: {
        providerId: provider.id,
        isReachable: true,
        status: "running",
        message: "Runtime is running.",
        checkedAt: new Date().toISOString(),
      },
      modelAvailable: true,
      benchmark: null,
      benchmarkFailure: { code: "stream_incomplete", message: "The provider closed the connection early." },
      guidance: "The benchmark failed (stream_incomplete): The provider closed the connection early.",
      runtime: {
        state: "healthy",
        pid: 4242,
        port: 49152,
        modelConfigured: true,
        failure: null,
        recentLogs: [],
      },
    }),
    // FTR-001: exercises BackupRestorePanel's UI wiring live — the real filesystem/SQLite
    // behavior itself is covered by src-tauri/src/backup.rs's own integration tests, not by this
    // fixture, which only proves the React side calls these methods and renders their results.
    createWorkspaceBackup: async (destinationDir) => ({
      backupPath: `${destinationDir}\\ark.sqlite3`,
      manifest: {
        appVersion: "0.1.0",
        createdAt: timestamp,
        databaseSha256: "b".repeat(64),
        databaseSizeBytes: 2_500_000,
      },
    }),
    previewWorkspaceRestore: async (backupPath) =>
      backupPath.includes("future")
        ? {
            manifest: {
              appVersion: "9.9.9",
              createdAt: timestamp,
              databaseSha256: "c".repeat(64),
              databaseSizeBytes: 1000,
            },
            detectedSchemaVersion: 999,
            schemaSupported: false,
            conversationCount: 3,
            messageCount: 40,
          }
        : {
            manifest: {
              appVersion: "0.1.0",
              createdAt: timestamp,
              databaseSha256: "b".repeat(64),
              databaseSizeBytes: 2_500_000,
            },
            detectedSchemaVersion: 5,
            schemaSupported: true,
            conversationCount: 3,
            messageCount: 40,
          },
    restoreWorkspaceBackup: async () => undefined,
  });
}

/**
 * FTR-002 browser fixture: a stateful in-memory catalog of several conversations — some pinned,
 * some archived, distinct enough titles/content for search to meaningfully match — so the
 * sidebar's search-snippet, archive/unarchive, pin/unpin, and "show archived" behaviors can all
 * be exercised live. Every other fixture's `listConversations`/`setConversationArchived`/
 * `setConversationPinned` are either unimplemented or return a static empty page; this is the
 * only fixture where those calls actually mutate and re-filter a list.
 */
export function createConversationOrganizationFixtureClient(): ArkClient {
  const timestamp = "2026-08-14T06:24:26Z";
  const provider: ProviderConfig = {
    id: "built_in",
    name: "Built-in llama.cpp",
    providerType: "built_in",
    baseUrl: "http://127.0.0.1:49152",
    defaultModelId: "fixture-model",
    defaultTemperature: 0.7,
    defaultMaxTokens: 2048,
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

  const searchableContent: Record<string, string> = {
    "fixture-conv-budget": "line items for the marketing spend and the infrastructure budget for next quarter",
    "fixture-conv-recipe": "a recipe for weekend pancakes with a note about doubling the vanilla",
    "fixture-conv-search-bug": "the FTS5 snippet function was returning an empty match for short queries",
    "fixture-conv-old-notes": "archived notes from the previous marketing campaign retrospective",
    "fixture-conv-vacation": "a draft itinerary for the coastal trip including flight and hotel links",
  };

  const conversations: Conversation[] = [
    {
      id: "fixture-conv-budget",
      title: "Quarterly budget planning",
      createdAt: timestamp,
      updatedAt: "2026-08-14T05:00:00Z",
      providerId: provider.id,
      modelId: model.name,
      archived: false,
      pinnedAt: "2026-08-14T05:30:00Z",
    },
    {
      id: "fixture-conv-recipe",
      title: "Weekend pancake recipe",
      createdAt: timestamp,
      updatedAt: "2026-08-14T04:00:00Z",
      providerId: provider.id,
      modelId: model.name,
      archived: false,
      pinnedAt: "2026-08-14T05:15:00Z",
    },
    {
      id: "fixture-conv-search-bug",
      title: "Debugging the search index",
      createdAt: timestamp,
      updatedAt: "2026-08-14T03:00:00Z",
      providerId: provider.id,
      modelId: model.name,
      archived: false,
      pinnedAt: null,
    },
    {
      id: "fixture-conv-old-notes",
      title: "Old marketing notes",
      createdAt: timestamp,
      updatedAt: "2026-08-14T02:00:00Z",
      providerId: provider.id,
      modelId: model.name,
      archived: true,
      pinnedAt: null,
    },
    {
      id: "fixture-conv-vacation",
      title: "Vacation itinerary draft",
      createdAt: timestamp,
      updatedAt: "2026-08-14T01:00:00Z",
      providerId: provider.id,
      modelId: model.name,
      archived: false,
      pinnedAt: null,
    },
  ];

  function buildSnippet(source: string, query: string): string {
    const index = source.toLowerCase().indexOf(query.toLowerCase());
    if (index === -1) return source.slice(0, 60);
    const start = Math.max(0, index - 20);
    const end = Math.min(source.length, index + query.length + 20);
    return `${start > 0 ? "…" : ""}${source.slice(start, end)}${end < source.length ? "…" : ""}`;
  }

  const bootstrap: AppBootstrap = {
    conversationPage: {
      items: conversations.filter((conversation) => !conversation.archived),
      nextCursor: null,
      searchSnippets: {},
    },
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
    deviceSettings: { theme: "dark", builtInModelPath: null, crashCaptureEnabled: false },
    workspaceOpenError: null,
  };

  return createFakeArkClient({
    getAppBootstrap: async () => bootstrap,
    getConversationMessages: async () => [],
    refreshModels: async () => ({
      health: {
        providerId: provider.id,
        isReachable: true,
        status: "running",
        message: "Runtime is running.",
        checkedAt: new Date().toISOString(),
      },
      models: [model],
      provider,
    }),
    listConversations: async (input) => {
      const query = (input.query ?? "").trim().toLowerCase();
      let items = conversations.filter((conversation) => {
        if (input.archived === false) return !conversation.archived;
        if (input.archived === true) return conversation.archived;
        return true;
      });
      const searchSnippets: Record<string, string> = {};
      if (query) {
        items = items.filter((conversation) => {
          const titleMatch = conversation.title.toLowerCase().includes(query);
          const content = searchableContent[conversation.id] ?? "";
          const contentMatch = content.toLowerCase().includes(query);
          if (!titleMatch && !contentMatch) return false;
          searchSnippets[conversation.id] = buildSnippet(titleMatch ? conversation.title : content, query);
          return true;
        });
      }
      items = [...items].sort((a, b) => b.updatedAt.localeCompare(a.updatedAt));
      return { items, nextCursor: null, searchSnippets };
    },
    setConversationArchived: async (id, archived) => {
      const conversation = conversations.find((item) => item.id === id);
      if (!conversation) throw new Error(`fixture: conversation ${id} not found`);
      conversation.archived = archived;
      return { ...conversation };
    },
    setConversationPinned: async (id, pinned) => {
      const conversation = conversations.find((item) => item.id === id);
      if (!conversation) throw new Error(`fixture: conversation ${id} not found`);
      conversation.pinnedAt = pinned ? new Date().toISOString() : null;
      return { ...conversation };
    },
    createConversation: async () => {
      const created: Conversation = {
        id: `fixture-conv-created-${conversations.length}`,
        title: "New conversation",
        createdAt: new Date().toISOString(),
        updatedAt: new Date().toISOString(),
        providerId: provider.id,
        modelId: model.name,
        archived: false,
        pinnedAt: null,
      };
      conversations.unshift(created);
      return created;
    },
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
