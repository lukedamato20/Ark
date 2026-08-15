import type {
  AppBootstrap,
  BuiltInRuntimeStatus,
  CompanionApiStatus,
  Conversation,
  Message,
  ModelInfo,
  OllamaPullProgress,
  Project,
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
    projects: [],
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
    projects: [],
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
    projects: [],
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

  // FTR-005: a second revision of the first assistant response, so the branch switcher,
  // renaming, and comparison view all have something real to exercise live — every other
  // fixture's getAssistantAlternatives/switchActiveBranch/getMessage/setBranchName are
  // unimplemented.
  const alternateMessage: Message = {
    id: "fixture-message-1-alt",
    conversationId: conversation.id,
    parentMessageId: "fixture-message-0",
    revisionOfMessageId: "fixture-message-1",
    pathIndex: 2,
    role: "assistant",
    content: "Response 1 (alternate). A more concise phrasing of the same explanation, without the code sample.",
    status: "complete",
    createdAt: timestamp,
    updatedAt: timestamp,
    providerId: provider.id,
    modelId: model.name,
    branchName: null,
  };
  const allMessages = [...messages, alternateMessage];
  const activeOverrides: Record<string, string> = {};
  const branchNames: Record<string, string | null> = {};

  function computeActivePath(): Message[] {
    const byParent = new Map<string | null, Message[]>();
    for (const item of allMessages) {
      const key = item.parentMessageId ?? null;
      const bucket = byParent.get(key) ?? [];
      bucket.push(item);
      byParent.set(key, bucket);
    }
    const path: Message[] = [];
    let currentParent: string | null = null;
    while (true) {
      const children: Message[] = byParent.get(currentParent) ?? [];
      if (children.length === 0) break;
      const overrideId = currentParent ? activeOverrides[currentParent] : undefined;
      const next: Message = children.find((child) => child.id === overrideId) ?? children[0];
      path.push({ ...next, branchName: branchNames[next.id] ?? next.branchName ?? null });
      currentParent = next.id;
    }
    return path;
  }

  const bootstrap: AppBootstrap = {
    conversationPage: { items: [conversation], nextCursor: null, searchSnippets: {} },
    providers: [provider],
    models: [model],
    projects: [],
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
    getConversationMessages: async () => computeActivePath(),
    getMessage: async (id) => {
      const found = allMessages.find((item) => item.id === id);
      if (!found) throw new Error(`fixture: message ${id} not found`);
      return { ...found, branchName: branchNames[id] ?? found.branchName ?? null };
    },
    getAssistantAlternatives: async (_conversationId, messageId) => {
      const target = allMessages.find((item) => item.id === messageId);
      if (!target?.parentMessageId) return [];
      const activeIds = new Set(computeActivePath().map((item) => item.id));
      return allMessages
        .filter((item) => item.parentMessageId === target.parentMessageId && item.role === "assistant")
        .map((sibling) => ({
          messageId: sibling.id,
          revisionOfMessageId: sibling.revisionOfMessageId ?? null,
          createdAt: sibling.createdAt,
          status: sibling.status,
          contentPreview: sibling.content.slice(0, 140),
          isActive: activeIds.has(sibling.id),
          hasDescendants: allMessages.some((item) => item.parentMessageId === sibling.id),
          branchName: branchNames[sibling.id] ?? sibling.branchName ?? null,
        }));
    },
    switchActiveBranch: async (_conversationId, messageId) => {
      const target = allMessages.find((item) => item.id === messageId);
      if (!target?.parentMessageId) throw new Error("fixture: cannot switch to a root message");
      activeOverrides[target.parentMessageId] = messageId;
      return computeActivePath();
    },
    setBranchName: async (messageId, name) => {
      const target = allMessages.find((item) => item.id === messageId);
      if (!target) throw new Error(`fixture: message ${messageId} not found`);
      if (target.role !== "assistant") throw new Error("fixture: only assistant messages can be named");
      branchNames[messageId] = name;
      return { ...target, branchName: name };
    },
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

  // FTR-003: one active project already assigned to a conversation so the picker/precedence UI
  // has something real to show on load, not just an empty-state.
  const projects: Project[] = [
    {
      id: "fixture-project-research",
      name: "Research",
      instructions: "Cite sources for every claim.",
      defaultProviderId: provider.id,
      defaultModelId: model.name,
      defaultTemperature: 0.2,
      defaultMaxTokens: 4096,
      archivedAt: null,
      createdAt: timestamp,
      updatedAt: timestamp,
    },
  ];
  conversations[2].projectId = "fixture-project-research";

  const companionApiState: CompanionApiStatus = {
    enabled: false,
    running: false,
    port: null,
    tokenConfigured: false,
  };

  const bootstrap: AppBootstrap = {
    conversationPage: {
      items: conversations.filter((conversation) => !conversation.archived),
      nextCursor: null,
      searchSnippets: {},
    },
    providers: [provider],
    models: [model],
    projects,
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
    setConversationProject: async (id, projectId) => {
      const conversation = conversations.find((item) => item.id === id);
      if (!conversation) throw new Error(`fixture: conversation ${id} not found`);
      if (projectId && !projects.some((project) => project.id === projectId)) {
        throw new Error(`fixture: project ${projectId} not found`);
      }
      conversation.projectId = projectId;
      return { ...conversation };
    },
    listProjects: async () => [...projects],
    createProject: async (name) => {
      const created: Project = {
        id: `fixture-project-${projects.length}`,
        name,
        instructions: null,
        defaultProviderId: null,
        defaultModelId: null,
        defaultTemperature: null,
        defaultMaxTokens: null,
        archivedAt: null,
        createdAt: new Date().toISOString(),
        updatedAt: new Date().toISOString(),
      };
      projects.push(created);
      return created;
    },
    updateProject: async (input) => {
      const project = projects.find((item) => item.id === input.id);
      if (!project) throw new Error(`fixture: project ${input.id} not found`);
      project.name = input.name;
      project.instructions = input.instructions ?? null;
      project.defaultProviderId = input.defaultProviderId ?? null;
      project.defaultModelId = input.defaultModelId ?? null;
      project.defaultTemperature = input.defaultTemperature ?? null;
      project.defaultMaxTokens = input.defaultMaxTokens ?? null;
      project.updatedAt = new Date().toISOString();
      return { ...project };
    },
    setProjectArchived: async (id, archived) => {
      const project = projects.find((item) => item.id === id);
      if (!project) throw new Error(`fixture: project ${id} not found`);
      project.archivedAt = archived ? new Date().toISOString() : null;
      return { ...project };
    },
    previewProjectDeletion: async (id) => {
      const project = projects.find((item) => item.id === id);
      if (!project) throw new Error(`fixture: project ${id} not found`);
      const conversationCount = conversations.filter((item) => item.projectId === id).length;
      return { project: { ...project }, conversationCount };
    },
    deleteProject: async (id) => {
      const index = projects.findIndex((item) => item.id === id);
      if (index === -1) throw new Error(`fixture: project ${id} not found`);
      projects.splice(index, 1);
      for (const conversation of conversations) {
        if (conversation.projectId === id) conversation.projectId = null;
      }
    },

    // FTR-008: exports the fixture's own in-memory conversations as a workspace bundle, and
    // treats any conversation ID already present locally as a duplicate on preview — enough to
    // exercise export → preview → duplicate-flagged → selective-import live, without a real
    // content hash (the fixture has no message bodies to hash).
    exportWorkspaceJson: async (projectId) => {
      const scoped = projectId ? conversations.filter((item) => item.projectId === projectId) : conversations;
      const manifest = {
        schemaVersion: 1,
        exportedAt: new Date().toISOString(),
        scope: projectId ? `project:${projectId}` : "workspace",
        entries: scoped.map((item) => ({
          conversationId: item.id,
          title: item.title,
          messageCount: 0,
          sha256: `fixture-hash-${item.id}`,
        })),
      };
      return JSON.stringify(
        { manifest, conversations: scoped.map((item) => ({ conversation: item, messages: [] })) },
        null,
        2,
      );
    },
    exportWorkspaceMarkdown: async (projectId) => {
      const scoped = projectId ? conversations.filter((item) => item.projectId === projectId) : conversations;
      const label = projectId ? (projects.find((item) => item.id === projectId)?.name ?? "project") : "workspace";
      return (
        `# Ark export — ${label}\n\n` +
        scoped.map((item) => `## ${item.title}\n\n(fixture export — no message bodies)`).join("\n\n---\n\n")
      );
    },
    previewWorkspaceImport: async (json) => {
      const parsed = JSON.parse(json) as {
        manifest: { scope: string; entries: { conversationId: string; title: string; messageCount: number }[] };
      };
      return {
        scope: parsed.manifest.scope,
        entries: parsed.manifest.entries.map((entry) => ({
          conversationId: entry.conversationId,
          title: entry.title,
          messageCount: entry.messageCount,
          duplicateOfLocalId: conversations.some((item) => item.id === entry.conversationId)
            ? entry.conversationId
            : null,
        })),
        providerMappings: [
          {
            sourceProviderId: provider.id,
            targetProviderId: provider.id,
            reason: "Matched an existing provider by stable ID.",
          },
        ],
      };
    },
    importWorkspaceJson: async (json, includeConversationIds) => {
      const parsed = JSON.parse(json) as { conversations: { conversation: Conversation }[] };
      let importedCount = 0;
      let skippedCount = 0;
      for (const entry of parsed.conversations) {
        if (!includeConversationIds.includes(entry.conversation.id)) {
          skippedCount += 1;
          continue;
        }
        importedCount += 1;
        conversations.unshift({
          ...entry.conversation,
          id: `fixture-imported-${conversations.length}-${entry.conversation.id}`,
          createdAt: new Date().toISOString(),
          updatedAt: new Date().toISOString(),
        });
      }
      return { importedCount, skippedCount };
    },

    // FTR-010: in-memory companion API state — enabling generates a token if none exists yet
    // (matching the real backend's `set_enabled`), disabling stops the "server" but keeps the
    // token configured for next time, and regenerating always reveals a fresh value once.
    getCompanionApiStatus: async () => ({ ...companionApiState }),
    setCompanionApiEnabled: async (enabled) => {
      if (enabled && !companionApiState.tokenConfigured) {
        companionApiState.tokenConfigured = true;
      }
      companionApiState.enabled = enabled;
      companionApiState.running = enabled;
      companionApiState.port = enabled ? 52341 : null;
      return { ...companionApiState };
    },
    regenerateCompanionApiToken: async () => {
      companionApiState.tokenConfigured = true;
      return {
        token: `fixture-token-${Math.random().toString(36).slice(2)}`,
        status: { ...companionApiState },
      };
    },
  });
}

/**
 * FTR-006 browser fixture: a reachable Ollama provider with two installed models carrying
 * realistic `/api/tags` `details` metadata (family/parameter size/quantization) — every other
 * fixture's pull/delete/cancel are unimplemented, so this is the only way to exercise the
 * Ollama model-management panel (metadata display, pull progress, cancellation, delete-with-
 * disk-footprint confirmation) live.
 */
export function createOllamaModelsFixtureClient(): ArkClient {
  const timestamp = "2026-08-15T06:00:00Z";
  const conversation: Conversation = {
    id: "fixture-ollama-conversation",
    title: "Ollama model management review",
    createdAt: timestamp,
    updatedAt: timestamp,
    providerId: "ollama",
    modelId: "llama3.2:8b",
    archived: false,
  };
  const provider: ProviderConfig = {
    id: "ollama",
    name: "Ollama",
    providerType: "ollama",
    baseUrl: "http://127.0.0.1:11434",
    defaultModelId: "llama3.2:8b",
    defaultTemperature: 0.7,
    defaultMaxTokens: 2048,
    isLocal: true,
    allowInsecureRemote: false,
    destinationClass: "loopback",
    capabilities: {
      streaming: true,
      modelListing: true,
      modelPull: true,
      modelDelete: true,
      modelUnload: false,
      requiresAuth: false,
      reportsContextWindow: false,
      vision: false,
      embeddings: false,
      tools: false,
    },
    isEnabled: true,
    createdAt: timestamp,
    updatedAt: timestamp,
  };
  const models: ModelInfo[] = [
    {
      id: "ollama:llama3.2:8b",
      providerId: provider.id,
      name: "llama3.2:8b",
      displayName: "llama3.2:8b",
      contextWindow: null,
      supportsStreaming: true,
      supportsTools: false,
      supportsVision: false,
      supportsEmbeddings: false,
      isAvailable: true,
      lastSeenAt: timestamp,
      metadataJson: JSON.stringify({
        size: 4_700_000_000,
        details: { family: "llama", parameter_size: "8B", quantization_level: "Q4_0" },
      }),
      createdAt: timestamp,
      updatedAt: timestamp,
    },
    {
      id: "ollama:mistral:7b",
      providerId: provider.id,
      name: "mistral:7b",
      displayName: "mistral:7b",
      contextWindow: null,
      supportsStreaming: true,
      supportsTools: false,
      supportsVision: false,
      supportsEmbeddings: false,
      isAvailable: true,
      lastSeenAt: timestamp,
      metadataJson: JSON.stringify({
        size: 4_100_000_000,
        details: { family: "mistral", parameter_size: "7B", quantization_level: "Q4_K_M" },
      }),
      createdAt: timestamp,
      updatedAt: timestamp,
    },
  ];
  let installedModels = [...models];

  const bootstrap: AppBootstrap = {
    conversationPage: { items: [conversation], nextCursor: null, searchSnippets: {} },
    providers: [provider],
    models: installedModels,
    projects: [],
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

  let pullProgressHandler: ((event: OllamaPullProgress) => void) | null = null;
  let pullCancelled = false;

  return createFakeArkClient({
    getAppBootstrap: async () => bootstrap,
    getConversationMessages: async () => [],
    refreshModels: async () => ({
      health: {
        providerId: provider.id,
        isReachable: true,
        status: "running",
        message: "Ollama is reachable.",
        checkedAt: new Date().toISOString(),
      },
      models: installedModels,
      provider,
    }),
    onOllamaPullProgress: async (handler) => {
      pullProgressHandler = handler;
      return () => {
        pullProgressHandler = null;
      };
    },
    pullOllamaModel: async (providerId, modelName) => {
      pullCancelled = false;
      const steps = [
        { status: "pulling manifest", total: undefined, completed: undefined },
        { status: "downloading", total: 1000, completed: 250 },
        { status: "downloading", total: 1000, completed: 650 },
        { status: "downloading", total: 1000, completed: 1000 },
        { status: "success", total: 1000, completed: 1000 },
      ];
      for (const step of steps) {
        if (pullCancelled) {
          throw { code: "pull_cancelled", message: "Model pull was cancelled." };
        }
        await new Promise((resolve) => setTimeout(resolve, 200));
        pullProgressHandler?.({
          providerId,
          modelName,
          status: step.status,
          total: step.total ?? null,
          completed: step.completed ?? null,
          digest: null,
          error: null,
        });
      }
      installedModels = [
        ...installedModels,
        {
          id: `ollama:${modelName}`,
          providerId,
          name: modelName,
          displayName: modelName,
          contextWindow: null,
          supportsStreaming: true,
          supportsTools: false,
          supportsVision: false,
          supportsEmbeddings: false,
          isAvailable: true,
          lastSeenAt: new Date().toISOString(),
          metadataJson: JSON.stringify({
            size: 3_800_000_000,
            details: { family: "unknown", parameter_size: "?", quantization_level: "Q4_0" },
          }),
          createdAt: new Date().toISOString(),
          updatedAt: new Date().toISOString(),
        },
      ];
    },
    cancelOllamaPull: async () => {
      pullCancelled = true;
    },
    deleteOllamaModel: async (_providerId, modelName) => {
      installedModels = installedModels.filter((model) => model.name !== modelName);
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
