import type {
  AppErrorShape,
  BuiltInRuntimeStatus,
  Conversation,
  Message,
  ModelInfo,
  Persona,
  Project,
  ProviderConfig,
  ProviderHealth,
  StreamEvent,
  ThemeMode,
  WorkspaceInfo,
} from "../types/ark";
import { createExternalStore, type ExternalStore } from "./externalStore.ts";

export type ActiveView = "chat" | "settings";

export interface EntityCollection<T> {
  ids: string[];
  byId: Record<string, T>;
}

export interface ConversationCatalogState {
  conversations: EntityCollection<Conversation>;
  nextCursor: string | null;
  search: string;
  isLoading: boolean;
  activeId?: string;
  /** FTR-002: conversation id -> matched-text excerpt, from the most recent search page's
   * `ConversationPage.searchSnippets`. Empty whenever `search` is empty. */
  searchSnippets: Record<string, string>;
  /** FTR-002: when true, archived conversations are included in the fetched page alongside
   * active ones, rather than the default active-only view. */
  showArchived: boolean;
}

export interface TranscriptState {
  conversationId?: string;
  messages: Message[];
  isLoading: boolean;
}

export interface GenerationOverlay {
  conversationId: string;
  content: string;
  status: string;
  errorMessage?: string | null;
  revision: number;
}

export interface GenerationState {
  byMessageId: Record<string, GenerationOverlay>;
  activeMessageIdByConversation: Record<string, string | undefined>;
}

export interface ProviderState {
  providers: EntityCollection<ProviderConfig>;
  models: EntityCollection<ModelInfo>;
  health: Record<string, ProviderHealth>;
}

/** FTR-003: expected to stay small (unlike conversations), so this is a plain unpaginated
 * collection with no search/cursor concept — matching the plan's own scope decision. */
export interface ProjectState {
  projects: EntityCollection<Project>;
}

/** FTR-003: mirrors `ProjectState` exactly — personas are independent of projects, each their
 * own small unpaginated collection. */
export interface PersonaState {
  personas: EntityCollection<Persona>;
}

export interface SettingsState {
  workspacePath: string;
  workspace: WorkspaceInfo | null;
  theme: ThemeMode;
  builtInStatus: BuiltInRuntimeStatus;
  builtInModelPath: string | null;
  crashCaptureEnabled: boolean;
  workspaceOpenError: AppErrorShape | null;
  retryingWorkspace: boolean;
}

export interface ShellState {
  booting: boolean;
  /** UX-004: a total bootstrap failure (`getAppBootstrap`/`getBuiltInRuntimeStatus` itself
   * rejecting), distinct from `SettingsState.workspaceOpenError` — which is a *partial* failure
   * inside an otherwise-successful bootstrap response. Nothing else in the app has loaded when
   * this is set, so it drives a dedicated full-screen recovery state rather than the global
   * toast, which would otherwise strand the user on an empty chat view with no explanation. */
  bootstrapError: AppErrorShape | null;
  view: ActiveView;
  sidebarCollapsed: boolean;
  rightPanelCollapsed: boolean;
  focusSearchSignal: number;
  /** UX-007: bumped by an explicit "New Chat" or conversation-select action so `ChatView` can
   * focus the composer — never on a passive background update (a reconciliation refetch, a
   * provider health poll), which would steal focus from whatever the user is actually doing. */
  focusComposerSignal: number;
  shortcutsOpen: boolean;
  error: string | null;
  info: string | null;
}

export interface ArkStores {
  catalog: ExternalStore<ConversationCatalogState>;
  transcript: ExternalStore<TranscriptState>;
  generation: ExternalStore<GenerationState>;
  providers: ExternalStore<ProviderState>;
  projects: ExternalStore<ProjectState>;
  personas: ExternalStore<PersonaState>;
  settings: ExternalStore<SettingsState>;
  shell: ExternalStore<ShellState>;
}

export function createArkStores(initial?: {
  theme?: ThemeMode;
  sidebarCollapsed?: boolean;
  rightPanelCollapsed?: boolean;
}): ArkStores {
  return {
    catalog: createExternalStore<ConversationCatalogState>({
      conversations: emptyEntityCollection(),
      nextCursor: null,
      search: "",
      isLoading: false,
      searchSnippets: {},
      showArchived: false,
    }),
    transcript: createExternalStore<TranscriptState>({ messages: [], isLoading: false }),
    generation: createExternalStore<GenerationState>({ byMessageId: {}, activeMessageIdByConversation: {} }),
    providers: createExternalStore<ProviderState>({
      providers: emptyEntityCollection(),
      models: emptyEntityCollection(),
      health: {},
    }),
    projects: createExternalStore<ProjectState>({
      projects: emptyEntityCollection(),
    }),
    personas: createExternalStore<PersonaState>({
      personas: emptyEntityCollection(),
    }),
    settings: createExternalStore<SettingsState>({
      workspacePath: "",
      workspace: null,
      theme: initial?.theme ?? "dark",
      builtInStatus: {
        running: false,
        binaryInstalled: false,
        binaryVerified: false,
        state: "unavailable_binary",
        failure: null,
      },
      builtInModelPath: null,
      crashCaptureEnabled: false,
      workspaceOpenError: null,
      retryingWorkspace: false,
    }),
    shell: createExternalStore<ShellState>({
      booting: true,
      bootstrapError: null,
      view: "chat",
      sidebarCollapsed: initial?.sidebarCollapsed ?? false,
      rightPanelCollapsed: initial?.rightPanelCollapsed ?? false,
      focusSearchSignal: 0,
      focusComposerSignal: 0,
      shortcutsOpen: false,
      error: null,
      info: null,
    }),
  };
}

export function emptyEntityCollection<T>(): EntityCollection<T> {
  return { ids: [], byId: {} };
}

export function entityCollection<T extends { id: string }>(items: T[]): EntityCollection<T> {
  return {
    ids: items.map((item) => item.id),
    byId: Object.fromEntries(items.map((item) => [item.id, item])),
  };
}

export function entityList<T>(collection: EntityCollection<T>): T[] {
  return collection.ids.flatMap((id) => {
    const entity = collection.byId[id];
    return entity === undefined ? [] : [entity];
  });
}

export function upsertEntity<T extends { id: string }>(
  collection: EntityCollection<T>,
  entity: T,
): EntityCollection<T> {
  return {
    ids: collection.byId[entity.id] ? collection.ids : [...collection.ids, entity.id],
    byId: { ...collection.byId, [entity.id]: entity },
  };
}

export function messageWithGenerationOverlay(message: Message, overlay?: GenerationOverlay): Message {
  if (!overlay) return message;
  const merged: Message = {
    ...message,
    content: overlay.content,
    status: overlay.status as Message["status"],
  };
  if (overlay.errorMessage !== undefined) merged.errorMessage = overlay.errorMessage;
  return merged;
}

export function streamOverlayFromEvent(
  current: GenerationOverlay | undefined,
  baseMessage: Message,
  event: StreamEvent,
): GenerationOverlay {
  return {
    conversationId: event.conversationId,
    content:
      event.content ??
      (event.delta
        ? (current?.content ?? baseMessage.content) + event.delta
        : (current?.content ?? baseMessage.content)),
    status: event.status,
    errorMessage: event.error ?? current?.errorMessage ?? baseMessage.errorMessage,
    revision: event.revision ?? current?.revision ?? 0,
  };
}
