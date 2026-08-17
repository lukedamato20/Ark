import type {
  AppBootstrap,
  Attachment,
  AuditEvent,
  BuiltInRuntimeStatus,
  CodeAgentRun,
  CodeAgentStep,
  CodeObservation,
  CodeRunDetail,
  CodeSession,
  CodeSessionDetail,
  CodeToolInvocation,
  CompanionApiStatus,
  Conversation,
  ConversationNote,
  EditFileOutcome,
  EditFilePreview,
  Message,
  ModelInfo,
  NoteWriteAction,
  OllamaPullProgress,
  Persona,
  PersonaVersionSummary,
  Project,
  ProviderConfig,
  SideEffectPreview,
  ToolCapabilityGrant,
  ToolStatus,
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
    isUserManaged: false,
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
    toolCallingMode: "unsupported",
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
    personas: [],
    applicationInstructions: null,
    workspacePath: "C:\\Ark",
    workspace: {
      rootPath: "C:\\Ark",
      databasePath: "C:\\Ark\\ark.sqlite3",
      defaultRootPath: "C:\\Ark",
      configPath: "C:\\Ark\\workspace.json",
      isPortable: false,
      requiresRestart: false,
    },
    deviceSettings: {
      theme: "dark",
      builtInModelPath: status.modelPath,
      crashCaptureEnabled: false,
      completionNotificationsEnabled: false,
      perfMetricsEnabled: false,
    },
    workspaceOpenError: null,
  };

  return createFakeArkClient({
    getAppBootstrap: async () => bootstrap,
    getBuiltInRuntimeStatus: async () => status,
    getConversationMessages: async () => ({ messages: [], hasMoreOlder: false }),
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
    isUserManaged: false,
    isEnabled: true,
    createdAt: timestamp,
    updatedAt: timestamp,
  };
  const bootstrap: AppBootstrap = {
    conversationPage: { items: [conversation], nextCursor: null, searchSnippets: {} },
    providers: [provider],
    models: [],
    projects: [],
    personas: [],
    applicationInstructions: null,
    workspacePath: "C:\\Ark",
    workspace: {
      rootPath: "C:\\Ark",
      databasePath: "C:\\Ark\\ark.sqlite3",
      defaultRootPath: "C:\\Ark",
      configPath: "C:\\Ark\\workspace.json",
      isPortable: false,
      requiresRestart: false,
    },
    deviceSettings: {
      theme: "dark",
      builtInModelPath: null,
      crashCaptureEnabled: false,
      completionNotificationsEnabled: false,
      perfMetricsEnabled: false,
    },
    workspaceOpenError: null,
  };
  let statusChecks = 0;
  let metadata: { id: string; masked: string; available: boolean } | null = null;

  return createFakeArkClient({
    getAppBootstrap: async () => bootstrap,
    getConversationMessages: async () => ({ messages: [], hasMoreOlder: false }),
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
    personas: [],
    applicationInstructions: null,
    workspacePath: "C:\\Ark",
    workspace: {
      rootPath: "C:\\Ark",
      databasePath: "C:\\Ark\\ark.sqlite3",
      defaultRootPath: "C:\\Ark",
      configPath: "C:\\Ark\\workspace.json",
      isPortable: false,
      requiresRestart: false,
    },
    deviceSettings: {
      theme: "dark",
      builtInModelPath: null,
      crashCaptureEnabled: false,
      completionNotificationsEnabled: false,
      perfMetricsEnabled: false,
    },
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
    getConversationMessages: async () => ({ messages: [], hasMoreOlder: false }),
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
    isUserManaged: false,
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
    toolCallingMode: "unsupported",
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
    personas: [],
    applicationInstructions: null,
    workspacePath: "C:\\Ark",
    workspace: {
      rootPath: "C:\\Ark",
      databasePath: "C:\\Ark\\ark.sqlite3",
      defaultRootPath: "C:\\Ark",
      configPath: "C:\\Ark\\workspace.json",
      isPortable: false,
      requiresRestart: false,
    },
    deviceSettings: {
      theme: "dark",
      builtInModelPath: null,
      crashCaptureEnabled: false,
      completionNotificationsEnabled: false,
      perfMetricsEnabled: false,
    },
    workspaceOpenError: null,
  };

  return createFakeArkClient({
    getAppBootstrap: async () => bootstrap,
    getConversationMessages: async () => ({ messages: computeActivePath(), hasMoreOlder: false }),
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
    getConversationBranchTopology: async () => {
      const activeIds = new Set(computeActivePath().map((item) => item.id));
      return allMessages.map((item) => ({
        messageId: item.id,
        parentMessageId: item.parentMessageId ?? null,
        revisionOfMessageId: item.revisionOfMessageId ?? null,
        pathIndex: item.pathIndex,
        role: item.role,
        createdAt: item.createdAt,
        status: item.status,
        contentPreview: item.content.slice(0, 140),
        isActive: activeIds.has(item.id),
        branchName: branchNames[item.id] ?? item.branchName ?? null,
        providerId: item.providerId ?? null,
        modelId: item.modelId ?? null,
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
    updateApplicationInstructions: async (instructions) => instructions,
    // FTR-004: every other fixture's updateConversationSettings is unimplemented — this is the
    // only way to exercise the conversation-settings panel's save flow live.
    updateConversationSettings: async (input) => ({
      ...conversation,
      systemPrompt: input.systemPrompt ?? null,
      temperature: input.temperature ?? null,
      maxTokens: input.maxTokens ?? null,
      responseStyle: input.responseStyle ?? null,
      tone: input.tone ?? null,
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
    isUserManaged: false,
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
    toolCallingMode: "unsupported",
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

  // FTR-003: one active persona, already at version 2 (revised once) so the version-history UI
  // has something real to show, and already assigned to a different conversation than the
  // project above — proving the two are independently assignable.
  const personas: Persona[] = [
    {
      id: "fixture-persona-reviewer",
      name: "Terse reviewer",
      instructions: "Be terse and cite line numbers.",
      defaultTemperature: 0.2,
      defaultMaxTokens: 512,
      versionNumber: 2,
      archivedAt: null,
      createdAt: timestamp,
      updatedAt: timestamp,
    },
  ];
  const personaVersions: Record<string, PersonaVersionSummary[]> = {
    "fixture-persona-reviewer": [
      {
        id: "fixture-persona-reviewer-v2",
        versionNumber: 2,
        instructions: "Be terse and cite line numbers.",
        defaultTemperature: 0.2,
        defaultMaxTokens: 512,
        createdAt: timestamp,
      },
      {
        id: "fixture-persona-reviewer-v1",
        versionNumber: 1,
        instructions: "Be terse.",
        defaultTemperature: 0.2,
        defaultMaxTokens: null,
        createdAt: timestamp,
      },
    ],
  };
  conversations[0].personaId = "fixture-persona-reviewer";

  // CMP-001: in-memory attachment state, independent of the messages array (this fixture never
  // seeds any pre-existing messages, matching how it's only ever used for the Settings-view
  // panels this session built — attachments here exist purely to exercise the compose-time
  // attach/paste/drop flow and, via the minimal `sendChatMessage` below, the "shows under the
  // sent message" display path).
  const attachments: Attachment[] = [];

  // CMP-003: mirrors the real backend's built-in "notes" tool + SEC-009 capability-grant/audit
  // persistence closely enough to live-verify the grant/revoke/preview/approve/audit-trail flow —
  // a single global, hash-chain-free (fixtures don't need tamper-evidence, only the interaction
  // shape) in-memory audit log, one grant slot for the one built-in tool, and per-conversation
  // notes.
  const notesToolDefinition = {
    id: "notes",
    name: "Notes",
    description: "Read and write a short scratch note attached to this conversation.",
    publisher: "Ark (built-in)",
    scope: {
      tier: "chat_safe" as const,
      read: true,
      write: true,
      network: false,
      secret: false,
      data: "This conversation's own notes",
    },
  };
  // CMP-004: the second built-in tool — mirrors `tools::built_in_tools()`'s web_search entry.
  const webSearchToolDefinition = {
    id: "web_search",
    name: "Web Search",
    description: "Search the web via Brave Search and bring back cited results.",
    publisher: "Brave Search (via Ark)",
    scope: {
      tier: "chat_safe" as const,
      read: true,
      write: false,
      network: true,
      secret: true,
      data: "Search query text sent to Brave Search API; result titles/URLs/snippets returned",
    },
  };
  const notes: ConversationNote[] = [];
  const grants: ToolCapabilityGrant[] = [];
  const auditEvents: AuditEvent[] = [];
  let nextAuditSequence = 0;
  // CMP-004: whether a Brave Search API key has been "saved" — mirrors `tool_secrets`'
  // presence/absence, not the key's actual value (never round-tripped, matching the real
  // secret store's own metadata-only IPC surface).
  let webSearchSecretConfigured = false;

  function recordAuditEvent(kind: AuditEvent["kind"], toolId: string, redactedDetail: string) {
    auditEvents.push({
      sequence: nextAuditSequence++,
      timestamp: new Date().toISOString(),
      kind,
      toolId,
      redactedDetail,
      chainHash: `fixture-hash-${nextAuditSequence}`,
    });
  }

  function activeGrant(toolId: string): ToolCapabilityGrant | null {
    const now = new Date().toISOString();
    return grants.find((grant) => grant.toolId === toolId && !grant.revoked && grant.expiresAt > now) ?? null;
  }

  const toolScopeData: Record<string, string> = {
    notes: notesToolDefinition.scope.data,
    web_search: webSearchToolDefinition.scope.data,
  };

  /** Mirrors `tools::authorize_tool_invocation`'s auto-grant: a short-lived (5 min) grant created
   * the moment a previewed action is approved with no already-valid grant for that tool. */
  function createAutoGrant(toolId: string): ToolCapabilityGrant {
    const definition = toolId === "web_search" ? webSearchToolDefinition : notesToolDefinition;
    const grant: ToolCapabilityGrant = {
      id: `fixture-grant-${grants.length}`,
      toolId,
      tier: "chat_safe",
      read: definition.scope.read,
      write: definition.scope.write,
      network: definition.scope.network,
      secret: definition.scope.secret,
      data: toolScopeData[toolId],
      grantedAt: new Date().toISOString(),
      expiresAt: new Date(Date.now() + 5 * 60_000).toISOString(),
      revoked: false,
    };
    grants.push(grant);
    recordAuditEvent("granted", toolId, `granted: ${grant.data} for 5 min`);
    return grant;
  }

  function previewSummary(action: NoteWriteAction, content?: string | null): string {
    const truncated = (content ?? "").trim().slice(0, 80);
    if (action === "create") return `Create a new note in this conversation: "${truncated}"`;
    if (action === "update") return `Replace this note's content with: "${truncated}"`;
    return "Delete this note permanently";
  }

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
    personas,
    applicationInstructions: null,
    workspacePath: "C:\\Ark",
    workspace: {
      rootPath: "C:\\Ark",
      databasePath: "C:\\Ark\\ark.sqlite3",
      defaultRootPath: "C:\\Ark",
      configPath: "C:\\Ark\\workspace.json",
      isPortable: false,
      requiresRestart: false,
    },
    deviceSettings: {
      theme: "dark",
      builtInModelPath: null,
      crashCaptureEnabled: false,
      completionNotificationsEnabled: false,
      perfMetricsEnabled: false,
    },
    workspaceOpenError: null,
  };

  return createFakeArkClient({
    getAppBootstrap: async () => bootstrap,
    getConversationMessages: async () => ({ messages: [], hasMoreOlder: false }),
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
        repositoryPath: null,
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
      project.responseStyle = input.responseStyle ?? null;
      project.tone = input.tone ?? null;
      project.updatedAt = new Date().toISOString();
      return { ...project };
    },
    setProjectRepository: async (id, repositoryPath) => {
      const project = projects.find((item) => item.id === id);
      if (!project) throw new Error(`fixture: project ${id} not found`);
      project.repositoryPath = repositoryPath;
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
      return { project: { ...project }, conversationCount, attachmentCount: 0 };
    },
    deleteProject: async (id) => {
      const index = projects.findIndex((item) => item.id === id);
      if (index === -1) throw new Error(`fixture: project ${id} not found`);
      projects.splice(index, 1);
      for (const conversation of conversations) {
        if (conversation.projectId === id) conversation.projectId = null;
      }
    },

    setConversationPersona: async (id, personaId) => {
      const conversation = conversations.find((item) => item.id === id);
      if (!conversation) throw new Error(`fixture: conversation ${id} not found`);
      if (personaId && !personas.some((persona) => persona.id === personaId)) {
        throw new Error(`fixture: persona ${personaId} not found`);
      }
      conversation.personaId = personaId;
      return { ...conversation };
    },
    listPersonas: async () => [...personas],
    createPersona: async (input) => {
      const created: Persona = {
        id: `fixture-persona-${personas.length}`,
        name: input.name,
        instructions: input.instructions,
        defaultTemperature: input.defaultTemperature ?? null,
        defaultMaxTokens: input.defaultMaxTokens ?? null,
        responseStyle: input.responseStyle ?? null,
        tone: input.tone ?? null,
        versionNumber: 1,
        archivedAt: null,
        createdAt: new Date().toISOString(),
        updatedAt: new Date().toISOString(),
      };
      personas.push(created);
      personaVersions[created.id] = [
        {
          id: `${created.id}-v1`,
          versionNumber: 1,
          instructions: created.instructions,
          defaultTemperature: created.defaultTemperature,
          defaultMaxTokens: created.defaultMaxTokens,
          responseStyle: created.responseStyle,
          tone: created.tone,
          createdAt: created.createdAt,
        },
      ];
      return created;
    },
    updatePersona: async (input) => {
      const persona = personas.find((item) => item.id === input.id);
      if (!persona) throw new Error(`fixture: persona ${input.id} not found`);
      const promptUnchanged =
        persona.instructions === input.instructions &&
        (persona.defaultTemperature ?? null) === (input.defaultTemperature ?? null) &&
        (persona.defaultMaxTokens ?? null) === (input.defaultMaxTokens ?? null) &&
        (persona.responseStyle ?? null) === (input.responseStyle ?? null) &&
        (persona.tone ?? null) === (input.tone ?? null);
      persona.name = input.name;
      persona.updatedAt = new Date().toISOString();
      if (!promptUnchanged) {
        persona.instructions = input.instructions;
        persona.defaultTemperature = input.defaultTemperature ?? null;
        persona.defaultMaxTokens = input.defaultMaxTokens ?? null;
        persona.responseStyle = input.responseStyle ?? null;
        persona.tone = input.tone ?? null;
        persona.versionNumber += 1;
        const versions = personaVersions[persona.id] ?? [];
        versions.unshift({
          id: `${persona.id}-v${persona.versionNumber}`,
          versionNumber: persona.versionNumber,
          instructions: persona.instructions,
          defaultTemperature: persona.defaultTemperature,
          defaultMaxTokens: persona.defaultMaxTokens,
          responseStyle: persona.responseStyle,
          tone: persona.tone,
          createdAt: persona.updatedAt,
        });
        personaVersions[persona.id] = versions;
      }
      return { ...persona };
    },
    listPersonaVersions: async (id) => [...(personaVersions[id] ?? [])],
    exportPersonaJson: async (id) => {
      const persona = personas.find((item) => item.id === id);
      if (!persona) throw new Error(`fixture: persona ${id} not found`);
      return JSON.stringify(
        {
          schemaVersion: 1,
          exportedAt: new Date().toISOString(),
          persona,
          versions: personaVersions[id] ?? [],
        },
        null,
        2,
      );
    },
    importPersonaJson: async (json) => {
      const parsed = JSON.parse(json) as {
        schemaVersion: number;
        persona: Persona;
        versions: PersonaVersionSummary[];
      };
      if (parsed.schemaVersion !== 1 || parsed.versions.length === 0) {
        throw new Error("Invalid persona export.");
      }
      const created: Persona = {
        ...parsed.persona,
        id: `fixture-persona-imported-${personas.length}`,
      };
      personas.push(created);
      personaVersions[created.id] = parsed.versions.map((version) => ({
        ...version,
        id: `${created.id}-v${version.versionNumber}`,
      }));
      return { ...created };
    },
    setPersonaArchived: async (id, archived) => {
      const persona = personas.find((item) => item.id === id);
      if (!persona) throw new Error(`fixture: persona ${id} not found`);
      persona.archivedAt = archived ? new Date().toISOString() : null;
      return { ...persona };
    },
    previewPersonaDeletion: async (id) => {
      const persona = personas.find((item) => item.id === id);
      if (!persona) throw new Error(`fixture: persona ${id} not found`);
      const conversationCount = conversations.filter((item) => item.personaId === id).length;
      return { persona: { ...persona }, conversationCount };
    },
    deletePersona: async (id) => {
      const index = personas.findIndex((item) => item.id === id);
      if (index === -1) throw new Error(`fixture: persona ${id} not found`);
      personas.splice(index, 1);
      delete personaVersions[id];
      for (const conversation of conversations) {
        if (conversation.personaId === id) conversation.personaId = null;
      }
    },

    // FTR-008: exports the fixture's own in-memory conversations as a workspace bundle, and
    // treats any conversation ID already present locally as a duplicate on preview — enough to
    // exercise export → preview → duplicate-flagged → selective-import live, without a real
    // content hash (the fixture has no message bodies to hash).
    exportWorkspaceJson: async (projectId) => {
      const scoped = projectId ? conversations.filter((item) => item.projectId === projectId) : conversations;
      const manifest = {
        schemaVersion: 2,
        exportedAt: new Date().toISOString(),
        scope: projectId ? `project:${projectId}` : "workspace",
        entries: scoped.map((item) => ({
          conversationId: item.id,
          title: item.title,
          messageCount: 0,
          attachmentCount: 0,
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
        manifest: {
          scope: string;
          entries: { conversationId: string; title: string; messageCount: number; attachmentCount?: number }[];
        };
      };
      return {
        scope: parsed.manifest.scope,
        entries: parsed.manifest.entries.map((entry) => ({
          conversationId: entry.conversationId,
          title: entry.title,
          messageCount: entry.messageCount,
          attachmentCount: entry.attachmentCount ?? 0,
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

    // CMP-001: mirrors the real backend's staged-then-linked lifecycle — `attachTextFile` always
    // starts a new attachment unlinked (`messageId: null`); `sendChatMessage` below is what links
    // it, matching `Database::link_attachments_to_message`'s real behavior.
    attachTextFile: async (conversationId, fileName, content) => {
      const conversation = conversations.find((item) => item.id === conversationId);
      if (!conversation) throw new Error(`fixture: conversation ${conversationId} not found`);
      const created: Attachment = {
        id: `fixture-attachment-${attachments.length}`,
        conversationId,
        messageId: null,
        fileName,
        byteSize: content.length,
        sha256: `fixture-sha256-${attachments.length}`,
        createdAt: new Date().toISOString(),
      };
      attachments.push(created);
      return created;
    },
    listConversationAttachments: async (conversationId) =>
      attachments.filter((item) => item.conversationId === conversationId).map((item) => ({ ...item })),
    getAttachmentContent: async (id) => {
      if (!attachments.some((item) => item.id === id)) throw new Error(`fixture: attachment ${id} not found`);
      return "fixture attachment content";
    },
    deleteAttachment: async (id) => {
      const index = attachments.findIndex((item) => item.id === id);
      if (index === -1) throw new Error(`fixture: attachment ${id} not found`);
      if (attachments[index].messageId) {
        throw new Error("fixture: cannot delete an attachment already linked to a sent message");
      }
      attachments.splice(index, 1);
    },

    listTools: async (): Promise<ToolStatus[]> => [
      { definition: notesToolDefinition, activeGrant: activeGrant("notes") },
      { definition: webSearchToolDefinition, activeGrant: activeGrant("web_search") },
    ],
    grantToolCapability: async (toolId, ttlMinutes) => {
      const definition =
        toolId === "web_search" ? webSearchToolDefinition : toolId === "notes" ? notesToolDefinition : null;
      if (!definition) throw { code: "not_found", message: "Tool not found." };
      const grant: ToolCapabilityGrant = {
        id: `fixture-grant-${grants.length}`,
        toolId,
        tier: "chat_safe",
        read: definition.scope.read,
        write: definition.scope.write,
        network: definition.scope.network,
        secret: definition.scope.secret,
        data: definition.scope.data,
        grantedAt: new Date().toISOString(),
        expiresAt: new Date(Date.now() + ttlMinutes * 60_000).toISOString(),
        revoked: false,
      };
      grants.push(grant);
      recordAuditEvent("granted", toolId, `granted: ${grant.data} for ${ttlMinutes} min`);
      return grant;
    },
    revokeToolCapability: async (id) => {
      const grant = grants.find((item) => item.id === id);
      if (!grant) throw { code: "not_found", message: "Capability grant not found." };
      grant.revoked = true;
      recordAuditEvent("revoked", grant.toolId, "revoked by user");
    },
    listToolAuditEvents: async () => auditEvents.map((event) => ({ ...event })),
    verifyToolAuditTrail: async () => true,

    listConversationNotes: async (conversationId) =>
      notes.filter((note) => note.conversationId === conversationId).map((note) => ({ ...note })),
    previewNoteWrite: async (action, content): Promise<SideEffectPreview> => ({
      toolId: "notes",
      summary: previewSummary(action, content),
      idempotency: "requires_fresh_approval",
    }),
    createNote: async (conversationId, content, approve) => {
      if (!activeGrant("notes")) {
        if (!approve)
          throw {
            code: "approval_required",
            message: "This action needs approval — preview it and grant access first.",
          };
        createAutoGrant("notes");
      }
      const note: ConversationNote = {
        id: `fixture-note-${notes.length}`,
        conversationId,
        content,
        createdAt: new Date().toISOString(),
        updatedAt: new Date().toISOString(),
      };
      notes.push(note);
      recordAuditEvent("invoked", "notes", "created a note");
      return note;
    },
    updateNote: async (id, content, approve) => {
      const note = notes.find((item) => item.id === id);
      if (!note) throw { code: "not_found", message: "Note not found." };
      if (!activeGrant("notes")) {
        if (!approve)
          throw {
            code: "approval_required",
            message: "This action needs approval — preview it and grant access first.",
          };
        createAutoGrant("notes");
      }
      note.content = content;
      note.updatedAt = new Date().toISOString();
      recordAuditEvent("invoked", "notes", "updated a note");
      return { ...note };
    },
    deleteNote: async (id, approve) => {
      const index = notes.findIndex((item) => item.id === id);
      if (index === -1) throw { code: "not_found", message: "Note not found." };
      if (!activeGrant("notes")) {
        if (!approve)
          throw {
            code: "approval_required",
            message: "This action needs approval — preview it and grant access first.",
          };
        createAutoGrant("notes");
      }
      notes.splice(index, 1);
      recordAuditEvent("invoked", "notes", "deleted a note");
    },

    previewWebSearch: async (query): Promise<SideEffectPreview> => ({
      toolId: "web_search",
      summary: `Send this query to Brave Search: "${query}"`,
      idempotency: "requires_fresh_approval",
    }),
    searchWeb: async (query, approve) => {
      if (!webSearchSecretConfigured) {
        throw {
          code: "tool_secret_not_configured",
          message: "Add a Brave Search API key in Settings → Tools before using web search.",
        };
      }
      if (!activeGrant("web_search")) {
        if (!approve)
          throw {
            code: "approval_required",
            message: "This action needs approval — preview it and grant access first.",
          };
        createAutoGrant("web_search");
      }
      recordAuditEvent("invoked", "web_search", `query: ${query.length} chars, 2 results`);
      return {
        citations: [
          {
            title: "Rust Release Notes",
            url: "https://example.test/rust-notes",
            snippet: "Recent changes to the language and standard library.",
          },
          {
            title: "Rust Programming Language",
            url: "https://example.test/rust-lang",
            snippet: "The official site for the Rust programming language.",
          },
        ],
      };
    },
    upsertToolSecret: async (toolId, secret) => {
      if (toolId !== "web_search") throw { code: "not_found", message: "Tool not found." };
      if (!secret.trim()) throw { code: "invalid_input", message: "Credential must be non-empty." };
      webSearchSecretConfigured = true;
      return { id: "fixture-tool-secret-web_search", masked: "••••••••", available: true };
    },
    getToolSecretMetadata: async (toolId) => {
      if (toolId !== "web_search" || !webSearchSecretConfigured) return null;
      return { id: "fixture-tool-secret-web_search", masked: "••••••••", available: true };
    },
    deleteToolSecret: async (toolId) => {
      if (toolId === "web_search") webSearchSecretConfigured = false;
    },

    // CMP-001: a minimal send — creates a user message and links any staged attachments to it,
    // enough to live-verify "attach, send, see it under the sent message" end to end. No real
    // generation is simulated (`startPendingStream`/stream event subscriptions fall through to
    // `createFakeArkClient`'s no-op defaults), so the assistant bubble stays in its placeholder
    // "Thinking" state — expected here, not a bug, since this fixture exists to exercise
    // attachments, not streaming.
    sendChatMessage: async (input) => {
      const conversation = conversations.find((item) => item.id === input.conversationId);
      if (!conversation) throw new Error(`fixture: conversation ${input.conversationId} not found`);
      const userMessageId = `fixture-sent-message-${Date.now()}-user`;
      const assistantMessageId = `fixture-sent-message-${Date.now()}-assistant`;
      for (const attachment of attachments) {
        if (input.attachmentIds?.includes(attachment.id)) {
          attachment.messageId = userMessageId;
        }
      }
      return {
        conversationId: input.conversationId,
        userMessageId,
        assistantMessageId,
      };
    },
  });
}

/**
 * FTR-006 browser fixture: a reachable Ollama provider with two installed models carrying
 * realistic `/api/tags` plus bounded `/api/show` metadata — every other fixture's
 * pull/delete/cancel are unimplemented, so this is the only way to exercise the Ollama
 * model-management panel (metadata display, pull progress, cancellation, delete-with-disk-
 * footprint confirmation) live.
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
      reportsContextWindow: true,
      vision: false,
      embeddings: false,
      tools: false,
    },
    isUserManaged: false,
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
      contextWindow: 131_072,
      supportsStreaming: true,
      supportsTools: false,
      toolCallingMode: "unsupported",
      supportsVision: false,
      supportsEmbeddings: false,
      isAvailable: true,
      lastSeenAt: timestamp,
      metadataJson: JSON.stringify({
        size: 4_700_000_000,
        details: { family: "llama", parameter_size: "8B", quantization_level: "Q4_0" },
        arkShow: { contextWindow: 131_072, licenseSummary: "Llama 3.2 Community License" },
      }),
      createdAt: timestamp,
      updatedAt: timestamp,
    },
    {
      id: "ollama:mistral:7b",
      providerId: provider.id,
      name: "mistral:7b",
      displayName: "mistral:7b",
      contextWindow: 32_768,
      supportsStreaming: true,
      supportsTools: false,
      toolCallingMode: "unsupported",
      supportsVision: false,
      supportsEmbeddings: false,
      isAvailable: true,
      lastSeenAt: timestamp,
      metadataJson: JSON.stringify({
        size: 4_100_000_000,
        details: { family: "mistral", parameter_size: "7B", quantization_level: "Q4_K_M" },
        arkShow: { contextWindow: 32_768, licenseSummary: "Apache-2.0" },
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
    personas: [],
    applicationInstructions: null,
    workspacePath: "C:\\Ark",
    workspace: {
      rootPath: "C:\\Ark",
      databasePath: "C:\\Ark\\ark.sqlite3",
      defaultRootPath: "C:\\Ark",
      configPath: "C:\\Ark\\workspace.json",
      isPortable: false,
      requiresRestart: false,
    },
    deviceSettings: {
      theme: "dark",
      builtInModelPath: null,
      crashCaptureEnabled: false,
      completionNotificationsEnabled: false,
      perfMetricsEnabled: false,
    },
    workspaceOpenError: null,
  };

  let pullProgressHandler: ((event: OllamaPullProgress) => void) | null = null;
  let pullCancelled = false;

  return createFakeArkClient({
    getAppBootstrap: async () => bootstrap,
    getConversationMessages: async () => ({ messages: [], hasMoreOlder: false }),
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
          toolCallingMode: "unsupported",
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
    // UX-011: deliberately tight — small enough that pulling any curated suggested model (all
    // several GB+) trips the "may not have enough space" warning, exercising that path without a
    // dedicated fixture.
    checkDiskSpace: async () => ({ totalBytes: 512_000_000_000, availableBytes: 2_000_000_000 }),
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

/** Deterministic, non-cryptographic stand-in for the Rust fixture's SHA-256 hashes — only needs
 * to change when the fixture's fake file content changes, so `codeExecuteEditFile` below can
 * genuinely reject a stale approval the same way the real backend does. */
function fixtureHash(value: string): string {
  let hash = 0;
  for (let index = 0; index < value.length; index += 1) {
    hash = (Math.imul(31, hash) + value.charCodeAt(index)) | 0;
  }
  return `fixture-${(hash >>> 0).toString(16)}`;
}

/**
 * CODE-005 browser fixture: one Ark Code session bound to a fake Repository containing a single
 * file, for live-verifying the `edit_file` diff-preview/approve/reject UI in `CodeView.tsx`
 * without a real Tauri backend. `codePreviewEditFile`/`codeExecuteEditFile` mirror the real
 * backend's approval-hash binding (`code_write_tools::execute_edit_file`): execution recomputes
 * hashes from the fixture's *current* fake file content and refuses if they no longer match what
 * was approved, so the "stale/tampered approval is rejected" behavior is genuinely exercised
 * here, not merely assumed.
 */
export function createCodeEditFixtureClient(): ArkClient {
  const timestamp = "2026-08-17T09:00:00Z";
  const project: Project = {
    id: "fixture-code-project",
    name: "Ark Code Fixture",
    repositoryPath: "C:\\fixtures\\ark-code-demo",
    instructions: null,
    defaultProviderId: null,
    defaultModelId: null,
    defaultTemperature: null,
    defaultMaxTokens: null,
    responseStyle: null,
    tone: null,
    archivedAt: null,
    createdAt: timestamp,
    updatedAt: timestamp,
  };
  const conversation: Conversation = {
    id: "fixture-code-conversation",
    title: "Ark Chat",
    createdAt: timestamp,
    updatedAt: timestamp,
    providerId: "built_in",
    archived: false,
  };
  const provider: ProviderConfig = {
    id: "fixture-code-provider",
    name: "Fixture Ollama",
    providerType: "ollama",
    baseUrl: "http://127.0.0.1:11434",
    defaultModelId: "fixture-model",
    defaultTemperature: 0.7,
    defaultMaxTokens: 4_096,
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
      reportsContextWindow: true,
      vision: false,
      embeddings: false,
      tools: true,
    },
    isUserManaged: false,
    isEnabled: true,
    createdAt: timestamp,
    updatedAt: timestamp,
  };
  const model: ModelInfo = {
    id: "fixture-code-model",
    providerId: provider.id,
    name: "fixture-model",
    displayName: "Fixture Model",
    contextWindow: 32_768,
    supportsStreaming: true,
    supportsTools: true,
    toolCallingMode: "native",
    supportsVision: false,
    supportsEmbeddings: false,
    isAvailable: true,
    lastSeenAt: timestamp,
    metadataJson: null,
    createdAt: timestamp,
    updatedAt: timestamp,
  };
  const bootstrap: AppBootstrap = {
    conversationPage: { items: [conversation], nextCursor: null, searchSnippets: {} },
    providers: [provider],
    models: [model],
    projects: [project],
    personas: [],
    applicationInstructions: null,
    workspacePath: "C:\\Ark",
    workspace: {
      rootPath: "C:\\Ark",
      databasePath: "C:\\Ark\\ark.sqlite3",
      defaultRootPath: "C:\\Ark",
      configPath: "C:\\Ark\\workspace.json",
      isPortable: false,
      requiresRestart: false,
    },
    deviceSettings: {
      theme: "dark",
      builtInModelPath: null,
      crashCaptureEnabled: false,
      completionNotificationsEnabled: false,
      perfMetricsEnabled: false,
    },
    workspaceOpenError: null,
  };

  const sessions = new Map<string, CodeSession>();
  const filePath = "src/greeting.rs";
  let fileContent = 'pub fn greet() -> &\'static str {\n    "Hello, world!"\n}\n';
  let lastPreview: EditFilePreview | null = null;

  interface FixtureStepRecord {
    step: CodeAgentStep;
    invocation?: CodeToolInvocation;
    observations: CodeObservation[];
  }
  let runCounter = 0;
  const runs = new Map<string, CodeAgentRun>();
  const runSteps = new Map<string, FixtureStepRecord[]>();

  function buildRunDetail(runId: string): CodeRunDetail {
    const run = runs.get(runId);
    if (!run) throw { code: "not_found", message: "Ark Code run not found." };
    const records = runSteps.get(runId) ?? [];
    return {
      run,
      steps: records.map((record) => record.step),
      invocations: records.flatMap((record) => (record.invocation ? [record.invocation] : [])),
      observations: records.flatMap((record) => record.observations),
      events: [],
    };
  }

  function buildPreview(path: string, search: string, replace: string): EditFilePreview {
    if (path !== filePath) {
      throw { code: "repository_path_not_found", message: "The requested Repository path was not found." };
    }
    const occurrences = fileContent.split(search).length - 1;
    if (occurrences === 0) {
      throw { code: "edit_search_not_found", message: "Edit block 1 search text was not found in the file." };
    }
    if (occurrences > 1) {
      throw {
        code: "edit_search_ambiguous",
        message: `Edit block 1 search text matches ${occurrences} places; it must match exactly one.`,
      };
    }
    const beforeHash = fixtureHash(fileContent);
    const afterContent = fileContent.replace(search, replace);
    const expectedAfterHash = fixtureHash(afterContent);
    const diff = [
      ...search.split("\n").map((line) => `- ${line}`),
      ...replace.split("\n").map((line) => `+ ${line}`),
    ].join("\n");
    const preview: EditFilePreview = {
      path,
      diff,
      beforeHash,
      expectedAfterHash,
      callHash: fixtureHash(`${path}::${search}::${replace}`),
      previewHash: fixtureHash(diff),
      preconditionHash: fixtureHash(`${path}::${beforeHash}`),
    };
    lastPreview = preview;
    return preview;
  }

  return createFakeArkClient({
    getAppBootstrap: async () => bootstrap,
    getConversationMessages: async () => ({ messages: [], hasMoreOlder: false }),
    listCodeSessions: async () => Array.from(sessions.values()),
    createCodeSession: async (input) => {
      const session: CodeSession = {
        id: `fixture-session-${sessions.size + 1}`,
        projectId: input.projectId,
        title: input.title,
        archived: false,
        createdAt: timestamp,
        updatedAt: timestamp,
      };
      sessions.set(session.id, session);
      return session;
    },
    getCodeSession: async (id) => {
      const session = sessions.get(id);
      if (!session) throw { code: "not_found", message: "Ark Code session not found." };
      const sessionRuns = Array.from(runs.values()).filter((run) => run.sessionId === id);
      const detail: CodeSessionDetail = { session, runs: sessionRuns, events: [] };
      return detail;
    },
    createCodeRun: async (input) => {
      runCounter += 1;
      const run: CodeAgentRun = {
        id: `fixture-run-${runCounter}`,
        sessionId: input.sessionId,
        parentRunId: null,
        providerId: input.providerId,
        modelId: input.modelId,
        task: input.task,
        repositoryPathSnapshot: project.repositoryPath ?? "",
        repositoryIdentityHash: "f".repeat(64),
        state: "queued",
        maxSteps: input.maxSteps ?? 12,
        maxActiveMs: input.maxActiveMs ?? 600_000,
        maxTokens: input.maxTokens ?? 32_768,
        maxCostMicrounits: null,
        stepsUsed: 0,
        activeElapsedMs: 0,
        reservedTokens: 0,
        actualTokens: 0,
        actualCostMicrounits: null,
        cancelRequestedAt: null,
        terminalReason: null,
        recoveryOutcome: null,
        createdAt: timestamp,
        updatedAt: timestamp,
        completedAt: null,
      };
      runs.set(run.id, run);
      runSteps.set(run.id, []);
      return run;
    },
    runCodeAgentStep: async (sessionId, runId) => {
      const run = runs.get(runId);
      if (!run || run.sessionId !== sessionId) {
        throw { code: "not_found", message: "Ark Code run not found." };
      }
      if (run.state !== "queued" && run.state !== "observing") {
        throw {
          code: "code_run_not_ready",
          message: `Ark Code run is '${run.state}' and cannot start a new step.`,
        };
      }
      const records = runSteps.get(runId) ?? [];
      const stepIndex = run.stepsUsed;
      const stepId = `fixture-step-${runId}-${stepIndex}`;
      const step: CodeAgentStep = {
        id: stepId,
        runId,
        stepIndex,
        state: "completed",
        reservedTokens: 512,
        actualTokens: 96,
        createdAt: timestamp,
      };
      let invocation: CodeToolInvocation | undefined;
      const observations: CodeObservation[] = [];
      if (stepIndex === 0) {
        invocation = {
          id: `fixture-invocation-${runId}`,
          runId,
          stepId,
          toolName: "read_file",
          canonicalArgumentsJson: JSON.stringify({ path: filePath }),
          state: "applied",
          createdAt: timestamp,
        };
        observations.push({
          id: `fixture-observation-tool-${runId}`,
          runId,
          stepId,
          kind: "tool_result",
          content: fileContent,
          createdAt: timestamp,
        });
        run.state = "observing";
      } else {
        observations.push({
          id: `fixture-observation-text-${runId}`,
          runId,
          stepId,
          kind: "model_text",
          content: "The greeting function looks correct and returns a static string as expected.",
          createdAt: timestamp,
        });
        run.state = "completed";
        run.completedAt = timestamp;
      }
      run.stepsUsed += 1;
      run.actualTokens += 96;
      run.updatedAt = timestamp;
      records.push({ step, invocation, observations });
      runSteps.set(runId, records);
      return buildRunDetail(runId);
    },
    getCodeRunDetail: async (runId) => buildRunDetail(runId),
    codePreviewEditFile: async (input) => {
      const [edit] = input.edits;
      return buildPreview(input.path, edit.search, edit.replace);
    },
    codeExecuteEditFile: async (input): Promise<EditFileOutcome> => {
      const [edit] = input.edits;
      const fresh = buildPreview(input.path, edit.search, edit.replace);
      if (
        !lastPreview ||
        input.callHash !== fresh.callHash ||
        input.previewHash !== fresh.previewHash ||
        input.preconditionHash !== fresh.preconditionHash
      ) {
        throw {
          code: "edit_approval_stale",
          message: "This edit no longer matches what was approved. Request a new preview.",
        };
      }
      const beforeHash = fixtureHash(fileContent);
      fileContent = fileContent.replace(edit.search, edit.replace);
      const observedAfterHash = fixtureHash(fileContent);
      lastPreview = null;
      return {
        path: input.path,
        beforeHash,
        expectedAfterHash: fresh.expectedAfterHash,
        observedAfterHash,
        outcome: observedAfterHash === fresh.expectedAfterHash ? "applied" : "diverged",
      };
    },
  });
}
