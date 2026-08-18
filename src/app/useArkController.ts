import * as React from "react";
import { getErrorMessage, normalizeError } from "../lib/arkErrors";
import type { SettingsSectionId } from "../lib/settingsSections";
import { findShortcut, matchesShortcut } from "../lib/shortcuts";
import { needsNewChatConfirmation } from "../lib/newChatLifecycle";
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
  AccentPalette,
  BuiltInRuntimeStatus,
  Conversation,
  Message,
  ModelInfo,
  Persona,
  Project,
  ProviderConfig,
  RefreshModelsResult,
  StreamEvent,
  ThemeMode,
  WorkspaceInfo,
} from "../types/ark";

/** PERF-003: the initial bounded page a conversation loads with — see `loadConversation`. */
const INITIAL_MESSAGE_PAGE_SIZE = 50;
/** PERF-003: each "Load earlier messages" click asks for this many more, from the same leaf —
 * see `loadOlderMessages`'s own doc comment for why re-requesting from the leaf (rather than a
 * cursor) is both simpler and correct here. */
const MESSAGE_PAGE_INCREMENT = 50;

export interface ArkController {
  bootstrap: () => Promise<void>;
  createConversation: (discardDraft?: boolean, projectId?: string | null) => Promise<void>;
  setChatComposerDraft: (draft: string) => void;
  dismissNewChatConfirmation: () => void;
  selectConversation: (id: string) => void;
  deleteActiveConversation: () => void;
  importConversation: (conversation: Conversation) => void;
  renameConversation: (conversation: Conversation) => void;
  setMessages: (messages: Message[]) => void;
  /** PERF-003: fetches the next page of older messages for the active conversation — a no-op
   * if there's nothing more to load or a page is already in flight. */
  loadOlderMessages: () => Promise<void>;
  searchConversations: (query: string) => Promise<void>;
  filterConversationsByProject: (projectId: string | null) => Promise<void>;
  loadMoreConversations: () => Promise<void>;
  /** FTR-002: undo is calling this again with the opposite value. */
  changeConversationArchived: (id: string, archived: boolean) => Promise<void>;
  changeConversationPinned: (id: string, pinned: boolean) => Promise<void>;
  /** FTR-003: `projectId: null` unassigns. */
  changeConversationProject: (id: string, projectId: string | null) => Promise<void>;
  /** FTR-003: `personaId: null` unassigns — independent of `changeConversationProject`. */
  changeConversationPersona: (id: string, personaId: string | null) => Promise<void>;
  setShowArchived: (showArchived: boolean) => void;
  refreshProviderModels: (providerId: string) => Promise<void>;
  cancelProviderRefresh: (providerId: string) => Promise<void>;
  saveProvider: (provider: ProviderConfig) => void;
  removeProvider: (id: string) => void;
  /** FTR-003: the project CRUD/archive mutations themselves go straight from the settings UI
   * through `useArkClient()`, matching `saveProvider`'s existing pattern — these two just sync
   * an already-server-confirmed result into the store. */
  saveProject: (project: Project) => void;
  removeProject: (id: string) => void;
  /** FTR-003: mirrors `saveProject`/`removeProject` exactly. */
  savePersona: (persona: Persona) => void;
  removePersona: (id: string) => void;
  changeApplicationInstructions: (instructions: string | null) => Promise<void>;
  changeTheme: (theme: ThemeMode) => Promise<void>;
  changeAccentPalette: (palette: AccentPalette) => Promise<void>;
  changeBuiltInModelPath: (path: string) => Promise<void>;
  changeManagedModelDirectory: (path: string | null) => Promise<void>;
  changeCrashCaptureEnabled: (enabled: boolean) => Promise<void>;
  changeCompletionNotificationsEnabled: (enabled: boolean) => Promise<void>;
  changePerfMetricsEnabled: (enabled: boolean) => Promise<void>;
  retryWorkspace: () => Promise<void>;
  setBuiltInStatus: (status: BuiltInRuntimeStatus) => void;
  setWorkspace: (workspace: WorkspaceInfo) => void;
  setView: (view: ActiveView) => void;
  setSettingsSection: (section: SettingsSectionId) => void;
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

function removeEntity<T>(collection: EntityCollection<T>, id: string): EntityCollection<T> {
  const { [id]: _removed, ...byId } = collection.byId as Record<string, T>;
  return { ids: collection.ids.filter((candidate) => candidate !== id), byId };
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
  const createConversationInFlightRef = React.useRef(false);
  const transcriptSequenceRef = React.useRef(0);
  const reconciliationSequenceByConversationRef = React.useRef(new Map<string, number>());
  // FTR-009: per-provider sequencing so a slower, earlier refresh can never overwrite a
  // faster, later one's result — the same `isLatestRequest` pattern already used for
  // conversation history/transcript loads above, applied to provider/model refresh instead.
  const providerRefreshSequenceRef = React.useRef(new Map<string, number>());
  // FTR-009: a refresh already in flight for a provider absorbs any additional trigger for
  // that same provider rather than starting a second redundant network/IPC round trip —
  // callers (bootstrap, ChatView's auto-refresh, Settings' Refresh buttons) can all safely
  // call this without coordinating with each other.
  const inFlightProviderRefreshesRef = React.useRef(new Set<string>());
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
      stores.transcript.set({ conversationId, messages, isLoading: false, hasMoreOlder: false, isLoadingOlder: false });
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
        hasMoreOlder: changingConversation ? false : currentTranscript.hasMoreOlder,
        isLoadingOlder: false,
      });
      clearConversationGeneration(stores, conversationId);
      try {
        // PERF-003: a bounded initial page, not the conversation's full history — see
        // `loadOlderMessages` for how the rest is fetched, on demand, if the user asks for it.
        const { messages, hasMoreOlder } = await client.getConversationMessages(
          conversationId,
          INITIAL_MESSAGE_PAGE_SIZE,
        );
        if (
          !isLatestRequest(sequence, transcriptSequenceRef.current) ||
          stores.catalog.getSnapshot().activeId !== conversationId
        ) {
          return;
        }
        stores.transcript.set({ conversationId, messages, isLoading: false, hasMoreOlder, isLoadingOlder: false });
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
          stores.transcript.set({
            conversationId,
            messages: [],
            isLoading: false,
            hasMoreOlder: false,
            isLoadingOlder: false,
          });
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

  /** PERF-003: re-requests the active path from the same leaf with a larger depth limit, rather
   * than tracking cursor/continuation state — see `Database::get_active_messages_page`'s own
   * doc comment for why this is both simpler and correct (the recursive query is cheap even at
   * a few hundred/thousand messages; ARC-007's own tests already prove sub-100ms at 250 nodes).
   * Because `MessageBubble`s are keyed by `message.id`, replacing `transcript.messages` with the
   * new (strictly larger) result leaves already-mounted bubbles alone — React only mounts the
   * newly-revealed older ones. Shares `transcriptSequenceRef` with `loadConversation` so
   * switching conversations mid-fetch correctly discards a stale response. */
  const loadOlderMessages = React.useCallback(async () => {
    const transcript = stores.transcript.getSnapshot();
    const { conversationId, hasMoreOlder, isLoadingOlder, messages } = transcript;
    if (!conversationId || !hasMoreOlder || isLoadingOlder) return;
    const sequence = ++transcriptSequenceRef.current;
    patchStore(stores.transcript, { isLoadingOlder: true });
    try {
      const nextDepthLimit = messages.length + MESSAGE_PAGE_INCREMENT;
      const { messages: olderMessages, hasMoreOlder: nextHasMoreOlder } = await client.getConversationMessages(
        conversationId,
        nextDepthLimit,
      );
      if (
        !isLatestRequest(sequence, transcriptSequenceRef.current) ||
        stores.transcript.getSnapshot().conversationId !== conversationId
      ) {
        return;
      }
      stores.transcript.set({
        conversationId,
        messages: olderMessages,
        isLoading: false,
        hasMoreOlder: nextHasMoreOlder,
        isLoadingOlder: false,
      });
    } catch (error) {
      if (isLatestRequest(sequence, transcriptSequenceRef.current)) {
        patchStore(stores.transcript, { isLoadingOlder: false });
        setError(getErrorMessage(error));
      }
    }
  }, [client, setError, stores]);

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

  const applyRefreshedModels = React.useCallback(
    (result: RefreshModelsResult) => {
      stores.providers.set((current) => ({
        health: { ...current.health, [result.health.providerId]: result.health },
        models: replaceModelsForProvider(current.models, result.models, result.provider.id),
        providers: replaceProvider(current.providers, result.provider),
      }));
    },
    [stores],
  );

  /**
   * FTR-009: the single path every caller (bootstrap, ChatView's auto-refresh, every "Refresh"
   * button in Settings) goes through to refresh a provider's health/model list. Centralizing it
   * here — rather than each caller fetching via `client.refreshModels` and applying the result
   * itself — is what makes the sequencing/dedup guarantees actually hold everywhere, not just at
   * whichever call site happened to add them: a second trigger for a provider already being
   * refreshed is absorbed rather than starting a redundant request, and a response is only ever
   * applied if no newer request for that same provider has started since.
   */
  const refreshProviderModels = React.useCallback(
    async (providerId: string) => {
      if (!providerId || inFlightProviderRefreshesRef.current.has(providerId)) {
        return;
      }
      const sequence = (providerRefreshSequenceRef.current.get(providerId) ?? 0) + 1;
      providerRefreshSequenceRef.current.set(providerId, sequence);
      inFlightProviderRefreshesRef.current.add(providerId);
      try {
        const result = await client.refreshModels(providerId);
        if (isLatestRequest(sequence, providerRefreshSequenceRef.current.get(providerId) ?? 0)) {
          applyRefreshedModels(result);
        }
      } catch (error) {
        if (
          normalizeError(error).code !== "provider_refresh_cancelled" &&
          isLatestRequest(sequence, providerRefreshSequenceRef.current.get(providerId) ?? 0)
        ) {
          setError(getErrorMessage(error));
        }
      } finally {
        inFlightProviderRefreshesRef.current.delete(providerId);
      }
    },
    [applyRefreshedModels, client, setError],
  );

  const cancelProviderRefresh = React.useCallback(
    async (providerId: string) => {
      if (!providerId || !inFlightProviderRefreshesRef.current.has(providerId)) return;
      try {
        await client.cancelProviderRefresh(providerId);
      } catch (error) {
        setError(getErrorMessage(error));
      }
    },
    [client, setError],
  );

  const bootstrap = React.useCallback(async () => {
    patchStore(stores.shell, { booting: true, bootstrapError: null });
    try {
      const [data, sidecarStatus, pinnedConversations] = await Promise.all([
        client.getAppBootstrap(),
        client.getBuiltInRuntimeStatus(),
        client.listPinnedConversations(50),
      ]);
      let conversations = data.conversationPage.items;
      if (conversations.length === 0) {
        conversations = [await client.createConversation()];
      }
      stores.catalog.set({
        conversations: entityCollection(conversations),
        pinnedConversations: entityCollection(pinnedConversations),
        nextCursor: data.conversationPage.nextCursor ?? null,
        search: "",
        isLoading: false,
        activeId: conversations[0]?.id,
        searchSnippets: data.conversationPage.searchSnippets,
        showArchived: false,
        selectedProjectId: null,
      });
      stores.providers.set({
        providers: entityCollection(data.providers),
        models: entityCollection(data.models),
        health: {},
      });
      stores.projects.set({ projects: entityCollection(data.projects) });
      stores.personas.set({ personas: entityCollection(data.personas) });
      stores.settings.set({
        workspacePath: data.workspacePath,
        workspace: data.workspace,
        applicationInstructions: data.applicationInstructions,
        theme: data.deviceSettings.theme,
        accentPalette: data.deviceSettings.accentPalette,
        builtInStatus: sidecarStatus,
        builtInModelPath: data.deviceSettings.builtInModelPath ?? null,
        managedModelDirectory: data.deviceSettings.managedModelDirectory ?? null,
        crashCaptureEnabled: data.deviceSettings.crashCaptureEnabled,
        completionNotificationsEnabled: data.deviceSettings.completionNotificationsEnabled,
        perfMetricsEnabled: data.deviceSettings.perfMetricsEnabled,
        workspaceOpenError: data.workspaceOpenError ?? null,
        retryingWorkspace: false,
      });
      if (conversations[0]) void loadConversation(conversations[0].id);

      // FTR-009: fire-and-forget — shell/history/settings are already loaded and set above, so
      // the UI must render now rather than wait on this network round trip. The result still
      // reaches the store via refreshProviderModels' own sequencing once it resolves, the same
      // path every other refresh trigger (ChatView's auto-refresh, Settings' Refresh buttons)
      // goes through.
      const providerToRefresh =
        data.providers.find((provider) => provider.id === conversations[0]?.providerId) ?? data.providers[0];
      if (providerToRefresh) {
        void refreshProviderModels(providerToRefresh.id);
      }
    } catch (error) {
      // A total bootstrap failure gets its own dedicated recovery state (App.tsx's
      // BootstrapFailurePanel), not just the dismissible global toast: nothing else loaded, so
      // dismissing the toast would strand the user on an unexplained empty chat view.
      patchStore(stores.shell, { bootstrapError: normalizeError(error) });
    } finally {
      patchStore(stores.shell, { booting: false });
      // PERF-001: `performance.now()` is already relative to navigation start, so this is a
      // direct "cached shell" proxy with no extra module-scope timestamp plumbing needed.
      // Fire-and-forget — the backend no-ops when the opt-in setting is off, and a failed
      // metric recording must never affect the app's actual bootstrap.
      void client.recordFrontendPerfMetric("cached_shell_ms", performance.now());
    }
  }, [client, loadConversation, refreshProviderModels, stores]);

  const createConversation = React.useCallback(
    async (discardDraft = false, projectId?: string | null) => {
      const initialShell = stores.shell.getSnapshot();
      if (needsNewChatConfirmation(initialShell.chatComposerDraft, discardDraft)) {
        patchStore(stores.shell, { newChatConfirmationRequested: true });
        return;
      }
      if (createConversationInFlightRef.current) return;
      createConversationInFlightRef.current = true;
      try {
        const conversation = await client.createConversation(undefined, projectId);
        const catalog = stores.catalog.getSnapshot();
        const conversations = entityList(catalog.conversations);
        stores.catalog.set({
          ...catalog,
          conversations: entityCollection([
            conversation,
            ...conversations.filter((item) => item.id !== conversation.id),
          ]),
          activeId: conversation.id,
        });
        stores.transcript.set({
          conversationId: conversation.id,
          messages: [],
          isLoading: false,
          hasMoreOlder: false,
          isLoadingOlder: false,
        });
        const shell = stores.shell.getSnapshot();
        patchStore(stores.shell, {
          view: "chat",
          focusComposerSignal: shell.focusComposerSignal + 1,
          chatComposerDraft: "",
          newChatConfirmationRequested: false,
        });
      } catch (error) {
        setError(getErrorMessage(error));
      } finally {
        createConversationInFlightRef.current = false;
      }
    },
    [client, setError, stores],
  );

  const setChatComposerDraft = React.useCallback(
    (chatComposerDraft: string) => patchStore(stores.shell, { chatComposerDraft }),
    [stores],
  );
  const dismissNewChatConfirmation = React.useCallback(
    () => patchStore(stores.shell, { newChatConfirmationRequested: false }),
    [stores],
  );

  const deleteActiveConversation = React.useCallback(() => {
    const catalog = stores.catalog.getSnapshot();
    const remaining = entityList(catalog.conversations).filter((conversation) => conversation.id !== catalog.activeId);
    const active = remaining[0];
    stores.catalog.set({
      ...catalog,
      conversations: entityCollection(remaining),
      activeId: active?.id,
    });
    stores.transcript.set({
      conversationId: active?.id,
      messages: [],
      isLoading: Boolean(active),
      hasMoreOlder: false,
      isLoadingOlder: false,
    });
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
      const showArchived = stores.catalog.getSnapshot().showArchived;
      const selectedProjectId = stores.catalog.getSnapshot().selectedProjectId;
      patchStore(stores.catalog, { search: normalizedQuery, isLoading: true });
      try {
        const page = await client.listConversations({
          limit: 50,
          query: normalizedQuery || null,
          // FTR-002: `null` includes archived conversations alongside active ones; `false`
          // (the default) excludes them, matching the existing pre-toggle behavior.
          archived: showArchived ? null : false,
          projectId: selectedProjectId,
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
          searchSnippets: page.searchSnippets,
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
        archived: catalog.showArchived ? null : false,
        projectId: catalog.selectedProjectId,
      });
      if (!isLatestRequest(sequence, historySequenceRef.current)) return;
      const current = stores.catalog.getSnapshot();
      stores.catalog.set({
        ...current,
        conversations: entityCollection(mergeConversationPage(entityList(current.conversations), page.items)),
        nextCursor: page.nextCursor ?? null,
        isLoading: false,
        searchSnippets: { ...current.searchSnippets, ...page.searchSnippets },
      });
    } catch (error) {
      if (isLatestRequest(sequence, historySequenceRef.current)) {
        patchStore(stores.catalog, { isLoading: false });
        setError(getErrorMessage(error));
      }
    }
  }, [client, setError, stores]);

  const filterConversationsByProject = React.useCallback(
    async (selectedProjectId: string | null) => {
      patchStore(stores.catalog, { selectedProjectId });
      await searchConversations(stores.catalog.getSnapshot().search);
    },
    [searchConversations, stores],
  );

  /** FTR-002: re-runs the current search/archived-visibility combination — the same fetch
   * `searchConversations`/`loadMoreConversations` already know how to do, just re-triggered
   * after a mutation (archive/pin) or a "show archived" toggle rather than a new query string. */
  const refreshConversationList = React.useCallback(async () => {
    await searchConversations(stores.catalog.getSnapshot().search);
  }, [searchConversations, stores]);

  const setShowArchived = React.useCallback(
    (showArchived: boolean) => {
      patchStore(stores.catalog, { showArchived });
      void refreshConversationList();
    },
    [refreshConversationList, stores],
  );

  const changeConversationArchived = React.useCallback(
    async (id: string, archived: boolean) => {
      try {
        const updated = await client.setConversationArchived(id, archived);
        if (!stores.catalog.getSnapshot().showArchived && archived) {
          // Archiving a conversation while the archived view is hidden removes it from the
          // visible list immediately, rather than waiting for the next full refetch.
          patchStore(stores.catalog, {
            conversations: entityCollection(
              entityList(stores.catalog.getSnapshot().conversations).filter((item) => item.id !== id),
            ),
            pinnedConversations: removeEntity(stores.catalog.getSnapshot().pinnedConversations, id),
          });
        } else {
          patchStore(stores.catalog, {
            conversations: upsertEntity(stores.catalog.getSnapshot().conversations, updated),
            pinnedConversations:
              updated.pinnedAt && !updated.archived
                ? upsertEntity(stores.catalog.getSnapshot().pinnedConversations, updated)
                : removeEntity(stores.catalog.getSnapshot().pinnedConversations, id),
          });
        }
      } catch (error) {
        setError(getErrorMessage(error));
      }
    },
    [client, setError, stores],
  );

  const changeConversationPinned = React.useCallback(
    async (id: string, pinned: boolean) => {
      try {
        const updated = await client.setConversationPinned(id, pinned);
        patchStore(stores.catalog, {
          conversations: upsertEntity(stores.catalog.getSnapshot().conversations, updated),
          pinnedConversations: pinned
            ? upsertEntity(stores.catalog.getSnapshot().pinnedConversations, updated)
            : removeEntity(stores.catalog.getSnapshot().pinnedConversations, id),
        });
      } catch (error) {
        setError(getErrorMessage(error));
      }
    },
    [client, setError, stores],
  );

  const changeConversationProject = React.useCallback(
    async (id: string, projectId: string | null) => {
      try {
        const updated = await client.setConversationProject(id, projectId);
        patchStore(stores.catalog, {
          conversations: upsertEntity(stores.catalog.getSnapshot().conversations, updated),
        });
      } catch (error) {
        setError(getErrorMessage(error));
      }
    },
    [client, setError, stores],
  );

  const saveProvider = React.useCallback(
    (provider: ProviderConfig) =>
      stores.providers.set((current) => ({ ...current, providers: replaceProvider(current.providers, provider) })),
    [stores],
  );

  const removeProvider = React.useCallback(
    (id: string) => {
      stores.providers.set((current) => {
        const health = { ...current.health };
        delete health[id];
        return {
          ...current,
          providers: removeEntity(current.providers, id),
          models: entityCollection(entityList(current.models).filter((model) => model.providerId !== id)),
          health,
        };
      });
      patchStore(stores.catalog, {
        conversations: entityCollection(
          entityList(stores.catalog.getSnapshot().conversations).map((conversation) =>
            conversation.providerId === id ? { ...conversation, providerId: null, modelId: null } : conversation,
          ),
        ),
      });
      patchStore(stores.projects, {
        projects: entityCollection(
          entityList(stores.projects.getSnapshot().projects).map((project) =>
            project.defaultProviderId === id ? { ...project, defaultProviderId: null, defaultModelId: null } : project,
          ),
        ),
      });
    },
    [stores],
  );

  const saveProject = React.useCallback(
    (project: Project) =>
      patchStore(stores.projects, { projects: upsertEntity(stores.projects.getSnapshot().projects, project) }),
    [stores],
  );

  const removeProject = React.useCallback(
    (id: string) => patchStore(stores.projects, { projects: removeEntity(stores.projects.getSnapshot().projects, id) }),
    [stores],
  );

  const changeConversationPersona = React.useCallback(
    async (id: string, personaId: string | null) => {
      try {
        const updated = await client.setConversationPersona(id, personaId);
        patchStore(stores.catalog, {
          conversations: upsertEntity(stores.catalog.getSnapshot().conversations, updated),
        });
      } catch (error) {
        setError(getErrorMessage(error));
      }
    },
    [client, setError, stores],
  );

  const savePersona = React.useCallback(
    (persona: Persona) =>
      patchStore(stores.personas, { personas: upsertEntity(stores.personas.getSnapshot().personas, persona) }),
    [stores],
  );

  const removePersona = React.useCallback(
    (id: string) => patchStore(stores.personas, { personas: removeEntity(stores.personas.getSnapshot().personas, id) }),
    [stores],
  );

  const changeApplicationInstructions = React.useCallback(
    async (instructions: string | null) => {
      const previous = stores.settings.getSnapshot().applicationInstructions;
      patchStore(stores.settings, { applicationInstructions: instructions });
      try {
        const saved = await client.updateApplicationInstructions(instructions);
        patchStore(stores.settings, { applicationInstructions: saved });
      } catch (error) {
        if (stores.settings.getSnapshot().applicationInstructions === instructions) {
          patchStore(stores.settings, { applicationInstructions: previous });
        }
        setError(getErrorMessage(error));
      }
    },
    [client, setError, stores],
  );

  const changeTheme = React.useCallback(
    async (theme: ThemeMode) => {
      const settings = stores.settings.getSnapshot();
      const sequence = ++settingsMutationSequenceRef.current;
      patchStore(stores.settings, { theme });
      const operation = settingsWriteQueueRef.current.then(() =>
        client.updateDeviceSettings({
          theme,
          accentPalette: settings.accentPalette,
          builtInModelPath: settings.builtInModelPath,
          managedModelDirectory: settings.managedModelDirectory,
          crashCaptureEnabled: settings.crashCaptureEnabled,
          completionNotificationsEnabled: settings.completionNotificationsEnabled,
          perfMetricsEnabled: settings.perfMetricsEnabled,
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
          accentPalette: settings.accentPalette,
          builtInModelPath: path,
          managedModelDirectory: settings.managedModelDirectory,
          crashCaptureEnabled: settings.crashCaptureEnabled,
          completionNotificationsEnabled: settings.completionNotificationsEnabled,
          perfMetricsEnabled: settings.perfMetricsEnabled,
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

  const changeAccentPalette = React.useCallback(
    async (accentPalette: AccentPalette) => {
      const settings = stores.settings.getSnapshot();
      const sequence = ++settingsMutationSequenceRef.current;
      patchStore(stores.settings, { accentPalette });
      const operation = settingsWriteQueueRef.current.then(() =>
        client.updateDeviceSettings({
          theme: settings.theme,
          accentPalette,
          builtInModelPath: settings.builtInModelPath,
          managedModelDirectory: settings.managedModelDirectory,
          crashCaptureEnabled: settings.crashCaptureEnabled,
          completionNotificationsEnabled: settings.completionNotificationsEnabled,
          perfMetricsEnabled: settings.perfMetricsEnabled,
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
          stores.settings.getSnapshot().accentPalette === accentPalette
        ) {
          patchStore(stores.settings, { accentPalette: settings.accentPalette });
        }
        setError(getErrorMessage(error));
      }
    },
    [client, setError, stores],
  );

  const changeManagedModelDirectory = React.useCallback(
    async (path: string | null) => {
      const settings = stores.settings.getSnapshot();
      const sequence = ++settingsMutationSequenceRef.current;
      patchStore(stores.settings, { managedModelDirectory: path });
      const operation = settingsWriteQueueRef.current.then(() =>
        client.updateDeviceSettings({
          theme: settings.theme,
          accentPalette: settings.accentPalette,
          builtInModelPath: settings.builtInModelPath,
          managedModelDirectory: path,
          crashCaptureEnabled: settings.crashCaptureEnabled,
          completionNotificationsEnabled: settings.completionNotificationsEnabled,
          perfMetricsEnabled: settings.perfMetricsEnabled,
        }),
      );
      settingsWriteQueueRef.current = operation.then(
        () => undefined,
        () => undefined,
      );
      try {
        const saved = await operation;
        patchStore(stores.settings, { managedModelDirectory: saved.managedModelDirectory ?? null });
      } catch (error) {
        if (
          sequence === settingsMutationSequenceRef.current &&
          stores.settings.getSnapshot().managedModelDirectory === path
        ) {
          patchStore(stores.settings, { managedModelDirectory: settings.managedModelDirectory });
        }
        setError(getErrorMessage(error));
        throw error;
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
          accentPalette: settings.accentPalette,
          builtInModelPath: settings.builtInModelPath,
          managedModelDirectory: settings.managedModelDirectory,
          crashCaptureEnabled: enabled,
          completionNotificationsEnabled: settings.completionNotificationsEnabled,
          perfMetricsEnabled: settings.perfMetricsEnabled,
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

  /** CMP-006: mirrors `changeCrashCaptureEnabled`'s exact optimistic-update/write-queue/rollback
   * shape, with one addition — enabling requires the OS notification permission first. A denial
   * (or the user simply not granting it) leaves the setting off and surfaces a message, rather
   * than persisting `true` and letting every future notification silently no-op. */
  const changeCompletionNotificationsEnabled = React.useCallback(
    async (enabled: boolean) => {
      if (enabled) {
        const granted = await client.requestNotificationPermission();
        if (!granted) {
          setError("Notification permission was not granted. Enable it in your OS settings to use this.");
          return;
        }
      }
      const settings = stores.settings.getSnapshot();
      const sequence = ++settingsMutationSequenceRef.current;
      patchStore(stores.settings, { completionNotificationsEnabled: enabled });
      const operation = settingsWriteQueueRef.current.then(() =>
        client.updateDeviceSettings({
          theme: settings.theme,
          accentPalette: settings.accentPalette,
          builtInModelPath: settings.builtInModelPath,
          managedModelDirectory: settings.managedModelDirectory,
          crashCaptureEnabled: settings.crashCaptureEnabled,
          completionNotificationsEnabled: enabled,
          perfMetricsEnabled: settings.perfMetricsEnabled,
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
          stores.settings.getSnapshot().completionNotificationsEnabled === enabled
        ) {
          patchStore(stores.settings, { completionNotificationsEnabled: settings.completionNotificationsEnabled });
        }
        setError(getErrorMessage(error));
      }
    },
    [client, setError, stores],
  );

  /** PERF-001: mirrors `changeCrashCaptureEnabled`'s exact optimistic-update/write-queue/rollback
   * shape — no permission step needed (unlike notifications), since this only gates writes into
   * the existing local diagnostics log. */
  const changePerfMetricsEnabled = React.useCallback(
    async (enabled: boolean) => {
      const settings = stores.settings.getSnapshot();
      const sequence = ++settingsMutationSequenceRef.current;
      patchStore(stores.settings, { perfMetricsEnabled: enabled });
      const operation = settingsWriteQueueRef.current.then(() =>
        client.updateDeviceSettings({
          theme: settings.theme,
          accentPalette: settings.accentPalette,
          builtInModelPath: settings.builtInModelPath,
          managedModelDirectory: settings.managedModelDirectory,
          crashCaptureEnabled: settings.crashCaptureEnabled,
          completionNotificationsEnabled: settings.completionNotificationsEnabled,
          perfMetricsEnabled: enabled,
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
          stores.settings.getSnapshot().perfMetricsEnabled === enabled
        ) {
          patchStore(stores.settings, { perfMetricsEnabled: settings.perfMetricsEnabled });
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
      document.documentElement.dataset.accent = settings.accentPalette;
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
  const setSettingsSection = React.useCallback(
    (settingsSection: SettingsSectionId) => patchStore(stores.shell, { settingsSection }),
    [stores],
  );
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
      if (matchesShortcut(event, findShortcut("showShortcuts").keys) && !isEditableTarget(event.target)) {
        event.preventDefault();
        setShortcutsOpen(true);
        return;
      }

      if (event.isComposing) return;
      if (matchesShortcut(event, findShortcut("newChat").keys)) {
        event.preventDefault();
        void createConversation();
      } else if (matchesShortcut(event, findShortcut("search").keys)) {
        event.preventDefault();
        openSearch();
      } else if (matchesShortcut(event, findShortcut("settings").keys)) {
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
      setChatComposerDraft,
      dismissNewChatConfirmation,
      selectConversation,
      deleteActiveConversation,
      importConversation,
      renameConversation,
      setMessages,
      loadOlderMessages,
      searchConversations,
      filterConversationsByProject,
      loadMoreConversations,
      changeConversationArchived,
      changeConversationPinned,
      changeConversationProject,
      changeConversationPersona,
      setShowArchived,
      refreshProviderModels,
      cancelProviderRefresh,
      saveProvider,
      removeProvider,
      saveProject,
      removeProject,
      savePersona,
      removePersona,
      changeApplicationInstructions,
      changeTheme,
      changeAccentPalette,
      changeBuiltInModelPath,
      changeManagedModelDirectory,
      changeCrashCaptureEnabled,
      changeCompletionNotificationsEnabled,
      changePerfMetricsEnabled,
      retryWorkspace,
      setBuiltInStatus,
      setWorkspace,
      setView,
      setSettingsSection,
      toggleSidebar,
      toggleRightPanel,
      openSearch,
      setShortcutsOpen,
      setError,
      setInfo,
    }),
    [
      bootstrap,
      changeApplicationInstructions,
      changeBuiltInModelPath,
      changeManagedModelDirectory,
      changeConversationArchived,
      changeConversationPinned,
      changeConversationProject,
      changeConversationPersona,
      changeCrashCaptureEnabled,
      changeCompletionNotificationsEnabled,
      changePerfMetricsEnabled,
      changeTheme,
      changeAccentPalette,
      createConversation,
      setChatComposerDraft,
      dismissNewChatConfirmation,
      deleteActiveConversation,
      importConversation,
      loadMoreConversations,
      loadOlderMessages,
      openSearch,
      refreshProviderModels,
      cancelProviderRefresh,
      removeProject,
      removePersona,
      renameConversation,
      retryWorkspace,
      saveProject,
      savePersona,
      saveProvider,
      removeProvider,
      searchConversations,
      filterConversationsByProject,
      selectConversation,
      setBuiltInStatus,
      setShowArchived,
      setError,
      setInfo,
      setMessages,
      setSettingsSection,
      setShortcutsOpen,
      setView,
      setWorkspace,
      toggleRightPanel,
      toggleSidebar,
    ],
  );
}
