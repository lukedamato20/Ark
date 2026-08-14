import * as React from "react";
import { getErrorMessage, normalizeError } from "../lib/arkErrors";
import { useArkClient } from "../lib/useArkClient";
import {
  entityCollection,
  entityList,
  messageWithGenerationOverlay,
  streamOverlayFromEvent,
  upsertEntity,
  type ActiveView,
  type ArkStores,
  type EntityCollection,
} from "../state/arkStores";
import {
  classifyStreamEvent,
  isLatestRequest,
  mergeConversationPage,
  preserveSelectedConversation,
} from "../state/reconciliation";
import { useArkStores } from "../state/useArkStores";
import type {
  BuiltInRuntimeStatus,
  Conversation,
  Message,
  ModelInfo,
  ProviderConfig,
  RefreshModelsResult,
  StreamEvent,
  ThemeMode,
  WorkspaceInfo,
} from "../types/ark";

export interface ArkController {
  bootstrap: () => Promise<void>;
  createConversation: () => Promise<void>;
  selectConversation: (id: string) => void;
  deleteActiveConversation: () => void;
  importConversation: (conversation: Conversation) => void;
  renameConversation: (conversation: Conversation) => void;
  setMessages: (messages: Message[]) => void;
  searchConversations: (query: string) => Promise<void>;
  loadMoreConversations: () => Promise<void>;
  refreshModels: (result: RefreshModelsResult) => void;
  saveProvider: (provider: ProviderConfig) => void;
  changeTheme: (theme: ThemeMode) => Promise<void>;
  changeBuiltInModelPath: (path: string) => Promise<void>;
  changeCrashCaptureEnabled: (enabled: boolean) => Promise<void>;
  retryWorkspace: () => Promise<void>;
  setBuiltInStatus: (status: BuiltInRuntimeStatus) => void;
  setWorkspace: (workspace: WorkspaceInfo) => void;
  setView: (view: ActiveView) => void;
  toggleSidebar: () => void;
  toggleRightPanel: () => void;
  openSearch: () => void;
  setShortcutsOpen: (open: boolean) => void;
  setError: (message: string | null) => void;
  setInfo: (message: string | null) => void;
}

function patchStore<T extends object>(store: { getSnapshot: () => T; set: (next: T) => void }, patch: Partial<T>) {
  store.set({ ...store.getSnapshot(), ...patch });
}

function isEditableTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  return target.tagName === "INPUT" || target.tagName === "TEXTAREA" || target.isContentEditable;
}

function replaceProvider(
  providers: EntityCollection<ProviderConfig>,
  provider: ProviderConfig,
): EntityCollection<ProviderConfig> {
  return upsertEntity(providers, provider);
}

function replaceModelsForProvider(
  current: EntityCollection<ModelInfo>,
  models: ModelInfo[],
  providerId: string,
): EntityCollection<ModelInfo> {
  return entityCollection([...entityList(current).filter((model) => model.providerId !== providerId), ...models]);
}

function clearConversationGeneration(stores: ArkStores, conversationId: string) {
  stores.generation.set((current) => {
    const byMessageId = Object.fromEntries(
      Object.entries(current.byMessageId).filter(([, overlay]) => overlay.conversationId !== conversationId),
    );
    const activeMessageIdByConversation = { ...current.activeMessageIdByConversation };
    delete activeMessageIdByConversation[conversationId];
    return { byMessageId, activeMessageIdByConversation };
  });
}

/**
 * ARC-008 application/UI coordinator. It owns effects and server-state invalidation but no
 * rendered state: domain snapshots live in scoped external stores, while components subscribe
 * only to the store/entity they render. This keeps App as composition and prevents stream deltas
 * from rerendering the sidebar, Settings, or completed sibling messages.
 */
export function useArkController(): ArkController {
  const client = useArkClient();
  const stores = useArkStores();
  const historySequenceRef = React.useRef(0);
  const transcriptSequenceRef = React.useRef(0);
  const reconciliationSequenceByConversationRef = React.useRef(new Map<string, number>());
  const settingsWriteQueueRef = React.useRef<Promise<unknown>>(Promise.resolve());
  const settingsMutationSequenceRef = React.useRef(0);
  const bootstrappedRef = React.useRef(false);

  const setError = React.useCallback(
    (message: string | null) => patchStore(stores.shell, { error: message }),
    [stores],
  );
  const setInfo = React.useCallback((message: string | null) => patchStore(stores.shell, { info: message }), [stores]);

  const setMessages = React.useCallback(
    (messages: Message[]) => {
      const catalog = stores.catalog.getSnapshot();
      const conversationId = catalog.activeId;
      stores.transcript.set({ conversationId, messages, isLoading: false });
      if (!conversationId) return;

      const activeMessage = [...messages]
        .reverse()
        .find(
          (message) => message.role === "assistant" && (message.status === "pending" || message.status === "streaming"),
        );
      stores.generation.set((current) => ({
        ...current,
        activeMessageIdByConversation: {
          ...current.activeMessageIdByConversation,
          [conversationId]: activeMessage?.id,
        },
      }));

      const lastMessage = messages[messages.length - 1];
      const currentConversation = conversationId ? catalog.conversations.byId[conversationId] : undefined;
      if (!lastMessage || !currentConversation) return;
      const active = {
        ...currentConversation,
        currentMessageId: lastMessage.id,
        providerId: lastMessage.providerId ?? currentConversation.providerId,
        modelId: lastMessage.modelId ?? currentConversation.modelId,
        updatedAt: lastMessage.updatedAt,
      };
      stores.catalog.set({
        ...catalog,
        conversations: upsertEntity(catalog.conversations, active),
      });
    },
    [stores],
  );

  const loadConversation = React.useCallback(
    async (conversationId: string) => {
      const sequence = ++transcriptSequenceRef.current;
      reconciliationSequenceByConversationRef.current.set(conversationId, sequence);
      const currentTranscript = stores.transcript.getSnapshot();
      const changingConversation = currentTranscript.conversationId !== conversationId;
      stores.transcript.set({
        conversationId,
        messages: changingConversation ? [] : currentTranscript.messages,
        isLoading: changingConversation,
      });
      clearConversationGeneration(stores, conversationId);
      try {
        const messages = await client.getConversationMessages(conversationId);
        if (
          !isLatestRequest(sequence, transcriptSequenceRef.current) ||
          stores.catalog.getSnapshot().activeId !== conversationId
        ) {
          return;
        }
        stores.transcript.set({ conversationId, messages, isLoading: false });
        const activeMessage = [...messages]
          .reverse()
          .find(
            (message) =>
              message.role === "assistant" && (message.status === "pending" || message.status === "streaming"),
          );
        stores.generation.set((current) => ({
          ...current,
          activeMessageIdByConversation: {
            ...current.activeMessageIdByConversation,
            [conversationId]: activeMessage?.id,
          },
        }));
      } catch (error) {
        if (isLatestRequest(sequence, transcriptSequenceRef.current)) {
          stores.transcript.set({ conversationId, messages: [], isLoading: false });
          setError(getErrorMessage(error));
        }
      } finally {
        if (reconciliationSequenceByConversationRef.current.get(conversationId) === sequence) {
          reconciliationSequenceByConversationRef.current.delete(conversationId);
        }
      }
    },
    [client, setError, stores],
  );

  const selectConversation = React.useCallback(
    (id: string) => {
      patchStore(stores.catalog, {
        activeId: id,
      });
      const shell = stores.shell.getSnapshot();
      patchStore(stores.shell, { view: "chat", focusComposerSignal: shell.focusComposerSignal + 1 });
      void loadConversation(id);
    },
    [loadConversation, stores],
  );

  const refreshModels = React.useCallback(
    (result: RefreshModelsResult) => {
      stores.providers.set((current) => ({
        health: { ...current.health, [result.health.providerId]: result.health },
        models: replaceModelsForProvider(current.models, result.models, result.provider.id),
        providers: replaceProvider(current.providers, result.provider),
      }));
    },
    [stores],
  );

  const bootstrap = React.useCallback(async () => {
    patchStore(stores.shell, { booting: true, bootstrapError: null });
    try {
      const [data, sidecarStatus] = await Promise.all([client.getAppBootstrap(), client.getBuiltInRuntimeStatus()]);
      let conversations = data.conversationPage.items;
      if (conversations.length === 0) {
        conversations = [await client.createConversation()];
      }
      stores.catalog.set({
        conversations: entityCollection(conversations),
        nextCursor: data.conversationPage.nextCursor ?? null,
        search: "",
        isLoading: false,
        activeId: conversations[0]?.id,
      });
      stores.providers.set({
        providers: entityCollection(data.providers),
        models: entityCollection(data.models),
        health: {},
      });
      stores.settings.set({
        workspacePath: data.workspacePath,
        workspace: data.workspace,
        theme: data.deviceSettings.theme,
        builtInStatus: sidecarStatus,
        builtInModelPath: data.deviceSettings.builtInModelPath ?? null,
        crashCaptureEnabled: data.deviceSettings.crashCaptureEnabled,
        workspaceOpenError: data.workspaceOpenError ?? null,
        retryingWorkspace: false,
      });
      if (conversations[0]) void loadConversation(conversations[0].id);

      const providerToRefresh =
        data.providers.find((provider) => provider.id === conversations[0]?.providerId) ?? data.providers[0];
      if (providerToRefresh) {
        refreshModels(await client.refreshModels(providerToRefresh.id));
      }
    } catch (error) {
      // A total bootstrap failure gets its own dedicated recovery state (App.tsx's
      // BootstrapFailurePanel), not just the dismissible global toast: nothing else loaded, so
      // dismissing the toast would strand the user on an unexplained empty chat view.
      patchStore(stores.shell, { bootstrapError: normalizeError(error) });
    } finally {
      patchStore(stores.shell, { booting: false });
    }
  }, [client, loadConversation, refreshModels, stores]);

  const createConversation = React.useCallback(async () => {
    try {
      const conversation = await client.createConversation();
      const catalog = stores.catalog.getSnapshot();
      const conversations = entityList(catalog.conversations);
      stores.catalog.set({
        ...catalog,
        conversations: entityCollection([conversation, ...conversations.filter((item) => item.id !== conversation.id)]),
        activeId: conversation.id,
      });
      stores.transcript.set({ conversationId: conversation.id, messages: [], isLoading: false });
      const shell = stores.shell.getSnapshot();
      patchStore(stores.shell, { view: "chat", focusComposerSignal: shell.focusComposerSignal + 1 });
    } catch (error) {
      setError(getErrorMessage(error));
    }
  }, [client, setError, stores]);

  const deleteActiveConversation = React.useCallback(() => {
    const catalog = stores.catalog.getSnapshot();
    const remaining = entityList(catalog.conversations).filter((conversation) => conversation.id !== catalog.activeId);
    const active = remaining[0];
    stores.catalog.set({
      ...catalog,
      conversations: entityCollection(remaining),
      activeId: active?.id,
    });
    stores.transcript.set({ conversationId: active?.id, messages: [], isLoading: Boolean(active) });
    if (active) void loadConversation(active.id);
  }, [loadConversation, stores]);

  const importConversation = React.useCallback(
    (conversation: Conversation) => {
      const catalog = stores.catalog.getSnapshot();
      const conversations = entityList(catalog.conversations);
      stores.catalog.set({
        ...catalog,
        conversations: entityCollection([conversation, ...conversations.filter((item) => item.id !== conversation.id)]),
        activeId: conversation.id,
      });
      patchStore(stores.shell, { view: "chat" });
      void loadConversation(conversation.id);
    },
    [loadConversation, stores],
  );

  const renameConversation = React.useCallback(
    (conversation: Conversation) => {
      const catalog = stores.catalog.getSnapshot();
      stores.catalog.set({
        ...catalog,
        conversations: upsertEntity(catalog.conversations, conversation),
      });
    },
    [stores],
  );

  const searchConversations = React.useCallback(
    async (query: string) => {
      const normalizedQuery = query.trim();
      const sequence = ++historySequenceRef.current;
      patchStore(stores.catalog, { search: normalizedQuery, isLoading: true });
      try {
        const page = await client.listConversations({
          limit: 50,
          query: normalizedQuery || null,
          archived: false,
        });
        if (!isLatestRequest(sequence, historySequenceRef.current)) return;
        const catalog = stores.catalog.getSnapshot();
        const selected = catalog.activeId ? catalog.conversations.byId[catalog.activeId] : undefined;
        const preserved = preserveSelectedConversation(selected, page.items);
        const pageItems =
          preserved && !page.items.some((conversation) => conversation.id === preserved.id)
            ? [...page.items, preserved]
            : page.items;
        stores.catalog.set({
          ...catalog,
          conversations: entityCollection(pageItems),
          nextCursor: page.nextCursor ?? null,
          isLoading: false,
        });
      } catch (error) {
        if (isLatestRequest(sequence, historySequenceRef.current)) {
          patchStore(stores.catalog, { isLoading: false });
          setError(getErrorMessage(error));
        }
      }
    },
    [client, setError, stores],
  );

  const loadMoreConversations = React.useCallback(async () => {
    const catalog = stores.catalog.getSnapshot();
    if (!catalog.nextCursor || catalog.isLoading) return;
    const sequence = ++historySequenceRef.current;
    patchStore(stores.catalog, { isLoading: true });
    try {
      const page = await client.listConversations({
        limit: 50,
        cursor: catalog.nextCursor,
        query: catalog.search || null,
        archived: false,
      });
      if (!isLatestRequest(sequence, historySequenceRef.current)) return;
      const current = stores.catalog.getSnapshot();
      stores.catalog.set({
        ...current,
        conversations: entityCollection(mergeConversationPage(entityList(current.conversations), page.items)),
        nextCursor: page.nextCursor ?? null,
        isLoading: false,
      });
    } catch (error) {
      if (isLatestRequest(sequence, historySequenceRef.current)) {
        patchStore(stores.catalog, { isLoading: false });
        setError(getErrorMessage(error));
      }
    }
  }, [client, setError, stores]);

  const saveProvider = React.useCallback(
    (provider: ProviderConfig) =>
      stores.providers.set((current) => ({ ...current, providers: replaceProvider(current.providers, provider) })),
    [stores],
  );

  const changeTheme = React.useCallback(
    async (theme: ThemeMode) => {
      const settings = stores.settings.getSnapshot();
      const sequence = ++settingsMutationSequenceRef.current;
      patchStore(stores.settings, { theme });
      const operation = settingsWriteQueueRef.current.then(() =>
        client.updateDeviceSettings({
          theme,
          builtInModelPath: settings.builtInModelPath,
          crashCaptureEnabled: settings.crashCaptureEnabled,
        }),
      );
      settingsWriteQueueRef.current = operation.then(
        () => undefined,
        () => undefined,
      );
      try {
        await operation;
      } catch (error) {
        if (sequence === settingsMutationSequenceRef.current && stores.settings.getSnapshot().theme === theme) {
          patchStore(stores.settings, { theme: settings.theme });
        }
        setError(getErrorMessage(error));
      }
    },
    [client, setError, stores],
  );

  const changeBuiltInModelPath = React.useCallback(
    async (path: string) => {
      const settings = stores.settings.getSnapshot();
      const sequence = ++settingsMutationSequenceRef.current;
      patchStore(stores.settings, { builtInModelPath: path });
      const operation = settingsWriteQueueRef.current.then(() =>
        client.updateDeviceSettings({
          theme: settings.theme,
          builtInModelPath: path,
          crashCaptureEnabled: settings.crashCaptureEnabled,
        }),
      );
      settingsWriteQueueRef.current = operation.then(
        () => undefined,
        () => undefined,
      );
      try {
        await operation;
      } catch (error) {
        if (
          sequence === settingsMutationSequenceRef.current &&
          stores.settings.getSnapshot().builtInModelPath === path
        ) {
          patchStore(stores.settings, { builtInModelPath: settings.builtInModelPath });
        }
        setError(getErrorMessage(error));
      }
    },
    [client, setError, stores],
  );

  const changeCrashCaptureEnabled = React.useCallback(
    async (enabled: boolean) => {
      const settings = stores.settings.getSnapshot();
      const sequence = ++settingsMutationSequenceRef.current;
      patchStore(stores.settings, { crashCaptureEnabled: enabled });
      const operation = settingsWriteQueueRef.current.then(() =>
        client.updateDeviceSettings({
          theme: settings.theme,
          builtInModelPath: settings.builtInModelPath,
          crashCaptureEnabled: enabled,
        }),
      );
      settingsWriteQueueRef.current = operation.then(
        () => undefined,
        () => undefined,
      );
      try {
        await operation;
      } catch (error) {
        if (
          sequence === settingsMutationSequenceRef.current &&
          stores.settings.getSnapshot().crashCaptureEnabled === enabled
        ) {
          patchStore(stores.settings, { crashCaptureEnabled: settings.crashCaptureEnabled });
        }
        setError(getErrorMessage(error));
      }
    },
    [client, setError, stores],
  );

  const retryWorkspace = React.useCallback(async () => {
    patchStore(stores.settings, { retryingWorkspace: true });
    try {
      const data = await client.retryWorkspaceOpen();
      patchStore(stores.settings, { workspaceOpenError: data.workspaceOpenError ?? null });
      if (!data.workspaceOpenError) await bootstrap();
    } catch (error) {
      setError(getErrorMessage(error));
    } finally {
      patchStore(stores.settings, { retryingWorkspace: false });
    }
  }, [bootstrap, client, setError, stores]);

  const applyStreamEvent = React.useCallback(
    (event: StreamEvent) => {
      if (stores.catalog.getSnapshot().activeId !== event.conversationId) return;
      const transcript = stores.transcript.getSnapshot();
      // Once an authoritative reconciliation is in flight, later event deltas are only
      // invalidations. Applying them against the pre-refetch snapshot would create another
      // gap/refetch loop; the single latest request replaces this state from durable data.
      if (reconciliationSequenceByConversationRef.current.has(event.conversationId)) return;
      const baseMessage = transcript.messages.find((message) => message.id === event.messageId);
      if (!baseMessage) {
        void loadConversation(event.conversationId);
        return;
      }
      const generation = stores.generation.getSnapshot();
      const currentOverlay = generation.byMessageId[event.messageId];
      const decision = classifyStreamEvent(currentOverlay?.revision, event);
      if (decision === "ignore-duplicate") return;
      if (decision === "refetch") {
        void loadConversation(event.conversationId);
        return;
      }
      if (decision === "apply-delta") {
        stores.generation.set({
          byMessageId: {
            ...generation.byMessageId,
            [event.messageId]: streamOverlayFromEvent(currentOverlay, baseMessage, event),
          },
          activeMessageIdByConversation: {
            ...generation.activeMessageIdByConversation,
            [event.conversationId]: event.messageId,
          },
        });
        return;
      }

      const reconciled = messageWithGenerationOverlay(
        baseMessage,
        streamOverlayFromEvent(currentOverlay, baseMessage, event),
      );
      const nextMessages = transcript.messages.map((message) => (message.id === reconciled.id ? reconciled : message));
      if (
        baseMessage.content !== reconciled.content ||
        baseMessage.status !== reconciled.status ||
        baseMessage.errorMessage !== reconciled.errorMessage
      ) {
        stores.transcript.set({ ...transcript, messages: nextMessages });
      }
      stores.generation.set((current) => {
        const byMessageId = { ...current.byMessageId };
        delete byMessageId[event.messageId];
        return {
          byMessageId,
          activeMessageIdByConversation: {
            ...current.activeMessageIdByConversation,
            [event.conversationId]: undefined,
          },
        };
      });
    },
    [loadConversation, stores],
  );

  React.useEffect(() => {
    if (!bootstrappedRef.current) {
      bootstrappedRef.current = true;
      void bootstrap();
    }
  }, [bootstrap]);

  React.useEffect(() => {
    const unlisteners: Array<() => void> = [];
    let disposed = false;
    const register = (promise: Promise<() => void>) => {
      void promise
        .then((unlisten) => {
          if (disposed) unlisten();
          else unlisteners.push(unlisten);
        })
        .catch((error) => setError(getErrorMessage(error)));
    };
    register(client.onStreamDelta(applyStreamEvent));
    register(client.onStreamComplete(applyStreamEvent));
    register(
      client.onStreamError((event) => {
        applyStreamEvent(event);
        if (event.error) setError(event.error);
      }),
    );
    register(client.onStreamCancelled(applyStreamEvent));
    register(client.onStreamInterrupted(applyStreamEvent));
    return () => {
      disposed = true;
      unlisteners.forEach((unlisten) => unlisten());
    };
  }, [applyStreamEvent, client, setError]);

  React.useEffect(() => {
    const applySettings = () => {
      const settings = stores.settings.getSnapshot();
      document.documentElement.classList.toggle("dark", settings.theme === "dark");
      localStorage.setItem("ark.theme", settings.theme);
    };
    const applyShell = () => {
      const shell = stores.shell.getSnapshot();
      localStorage.setItem("ark.sidebar", shell.sidebarCollapsed ? "collapsed" : "expanded");
      localStorage.setItem("ark.rightPanel", shell.rightPanelCollapsed ? "collapsed" : "expanded");
    };
    applySettings();
    applyShell();
    const unsubscribeSettings = stores.settings.subscribe(applySettings);
    const unsubscribeShell = stores.shell.subscribe(applyShell);
    return () => {
      unsubscribeSettings();
      unsubscribeShell();
    };
  }, [stores]);

  const setView = React.useCallback((view: ActiveView) => patchStore(stores.shell, { view }), [stores]);
  const toggleSidebar = React.useCallback(
    () => patchStore(stores.shell, { sidebarCollapsed: !stores.shell.getSnapshot().sidebarCollapsed }),
    [stores],
  );
  const toggleRightPanel = React.useCallback(
    () => patchStore(stores.shell, { rightPanelCollapsed: !stores.shell.getSnapshot().rightPanelCollapsed }),
    [stores],
  );
  const openSearch = React.useCallback(() => {
    const shell = stores.shell.getSnapshot();
    patchStore(stores.shell, { sidebarCollapsed: false, focusSearchSignal: shell.focusSearchSignal + 1 });
  }, [stores]);
  const setBuiltInStatus = React.useCallback(
    (builtInStatus: BuiltInRuntimeStatus) => patchStore(stores.settings, { builtInStatus }),
    [stores],
  );
  const setWorkspace = React.useCallback(
    (workspace: WorkspaceInfo) => patchStore(stores.settings, { workspace }),
    [stores],
  );
  const setShortcutsOpen = React.useCallback(
    (shortcutsOpen: boolean) => patchStore(stores.shell, { shortcutsOpen }),
    [stores],
  );

  React.useEffect(() => {
    function handleKeyDown(event: KeyboardEvent) {
      // UX-007: "?" (Shift+/ on most layouts, but browsers already report it as `event.key ===
      // "?"`) is a normal typable character, unlike the Mod-prefixed shortcuts below — it must
      // not fire while the user is typing it into an editable field.
      if (event.key === "?" && !event.metaKey && !event.ctrlKey && !event.altKey && !isEditableTarget(event.target)) {
        event.preventDefault();
        setShortcutsOpen(true);
        return;
      }

      const modifier = event.metaKey || event.ctrlKey;
      if (!modifier || event.altKey || event.shiftKey || event.isComposing) return;
      if (event.key.toLowerCase() === "n") {
        event.preventDefault();
        void createConversation();
      } else if (event.key.toLowerCase() === "f") {
        event.preventDefault();
        openSearch();
      } else if (event.key === ",") {
        event.preventDefault();
        setView("settings");
      }
    }
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [createConversation, openSearch, setShortcutsOpen, setView]);

  return React.useMemo(
    () => ({
      bootstrap,
      createConversation,
      selectConversation,
      deleteActiveConversation,
      importConversation,
      renameConversation,
      setMessages,
      searchConversations,
      loadMoreConversations,
      refreshModels,
      saveProvider,
      changeTheme,
      changeBuiltInModelPath,
      changeCrashCaptureEnabled,
      retryWorkspace,
      setBuiltInStatus,
      setWorkspace,
      setView,
      toggleSidebar,
      toggleRightPanel,
      openSearch,
      setShortcutsOpen,
      setError,
      setInfo,
    }),
    [
      bootstrap,
      changeBuiltInModelPath,
      changeCrashCaptureEnabled,
      changeTheme,
      createConversation,
      deleteActiveConversation,
      importConversation,
      loadMoreConversations,
      openSearch,
      refreshModels,
      renameConversation,
      retryWorkspace,
      saveProvider,
      searchConversations,
      selectConversation,
      setBuiltInStatus,
      setError,
      setInfo,
      setMessages,
      setShortcutsOpen,
      setView,
      setWorkspace,
      toggleRightPanel,
      toggleSidebar,
    ],
  );
}
