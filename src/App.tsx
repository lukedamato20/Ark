import { listen } from "@tauri-apps/api/event";
import * as React from "react";
import { ConversationSidebar } from "./features/conversations/ConversationSidebar";
import {
  createConversation,
  getAppBootstrap,
  getBuiltInRuntimeStatus,
  getConversationMessages,
  getErrorMessage,
  refreshModels,
  setThemePreference,
} from "./lib/api";
import type {
  AppBootstrap,
  BuiltInRuntimeStatus,
  Conversation,
  Message,
  ModelInfo,
  ProviderConfig,
  ProviderHealth,
  StreamEvent,
  ThemeMode,
  WorkspaceInfo,
} from "./types/ark";
import { RightPanel } from "./components/RightPanel";
import { Button } from "./ui/button";

const ChatView = React.lazy(() => import("./features/chat/ChatView").then((m) => ({ default: m.ChatView })));
const SettingsView = React.lazy(() =>
  import("./features/settings/SettingsView").then((m) => ({ default: m.SettingsView })),
);

type ActiveView = "chat" | "settings";

export default function App() {
  const [booting, setBooting] = React.useState(true);
  const [view, setView] = React.useState<ActiveView>("chat");
  const [conversations, setConversations] = React.useState<Conversation[]>([]);
  const [activeConversationId, setActiveConversationId] = React.useState<string | undefined>();
  const [messages, setMessages] = React.useState<Message[]>([]);
  const [providers, setProviders] = React.useState<ProviderConfig[]>([]);
  const [models, setModels] = React.useState<ModelInfo[]>([]);
  const [providerHealth, setProviderHealth] = React.useState<Record<string, ProviderHealth>>({});
  const [workspacePath, setWorkspacePath] = React.useState("");
  const [workspace, setWorkspace] = React.useState<WorkspaceInfo | null>(null);
  const [theme, setTheme] = React.useState<ThemeMode>(() => getStoredTheme());
  const [sidebarCollapsed, setSidebarCollapsed] = React.useState(
    () => localStorage.getItem("ark.sidebar") === "collapsed",
  );
  const [rightPanelCollapsed, setRightPanelCollapsed] = React.useState(
    () => localStorage.getItem("ark.rightPanel") === "collapsed",
  );
  const [focusSearchSignal, setFocusSearchSignal] = React.useState(0);
  const [loadingMessages, setLoadingMessages] = React.useState(false);
  const [error, setError] = React.useState<string | null>(null);
  const [builtInStatus, setBuiltInStatus] = React.useState<BuiltInRuntimeStatus>({ running: false });

  const activeConversation = conversations.find((c) => c.id === activeConversationId);

  React.useEffect(() => {
    document.documentElement.classList.toggle("dark", theme === "dark");
    localStorage.setItem("ark.theme", theme);
  }, [theme]);

  React.useEffect(() => {
    localStorage.setItem("ark.sidebar", sidebarCollapsed ? "collapsed" : "expanded");
  }, [sidebarCollapsed]);

  React.useEffect(() => {
    localStorage.setItem("ark.rightPanel", rightPanelCollapsed ? "collapsed" : "expanded");
  }, [rightPanelCollapsed]);

  React.useEffect(() => {
    void bootstrap();
  }, []);

  React.useEffect(() => {
    if (!activeConversationId) {
      setMessages([]);
      return;
    }

    setLoadingMessages(true);
    getConversationMessages(activeConversationId)
      .then(setMessages)
      .catch((err) => setError(getErrorMessage(err)))
      .finally(() => setLoadingMessages(false));
  }, [activeConversationId]);

  React.useEffect(() => {
    const unlisteners: Array<() => void> = [];

    const applyEvent = (payload: StreamEvent) => {
      setMessages((current) =>
        current.map((m) =>
          m.id === payload.messageId
            ? {
                ...m,
                content: payload.content ?? m.content,
                status: payload.status,
                errorMessage: payload.error ?? m.errorMessage,
                updatedAt: new Date().toISOString(),
              }
            : m,
        ),
      );
    };

    void listen<StreamEvent>("chat:stream-delta", (e) => applyEvent(e.payload)).then((u) => unlisteners.push(u));
    void listen<StreamEvent>("chat:stream-complete", (e) => applyEvent(e.payload)).then((u) => unlisteners.push(u));
    void listen<StreamEvent>("chat:stream-error", (e) => {
      applyEvent(e.payload);
      if (e.payload.error) setError(e.payload.error);
    }).then((u) => unlisteners.push(u));
    void listen<StreamEvent>("chat:stream-cancelled", (e) => applyEvent(e.payload)).then((u) => unlisteners.push(u));

    return () => {
      unlisteners.forEach((u) => u());
    };
  }, []);

  React.useEffect(() => {
    function handleKeyDown(event: KeyboardEvent) {
      const modifier = event.metaKey || event.ctrlKey;
      if (!modifier || event.altKey || event.shiftKey || event.isComposing) return;

      if (event.key.toLowerCase() === "n") {
        event.preventDefault();
        void handleCreateConversation();
        return;
      }
      if (event.key.toLowerCase() === "f") {
        event.preventDefault();
        setSidebarCollapsed(false);
        setFocusSearchSignal((v) => v + 1);
        return;
      }
      if (event.key === ",") {
        event.preventDefault();
        setView("settings");
      }
    }

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, []);

  async function bootstrap() {
    setBooting(true);
    try {
      const [data, sidecarStatus] = await Promise.all([getAppBootstrap(), getBuiltInRuntimeStatus()]);
      setBuiltInStatus(sidecarStatus);
      setConversations(data.conversations);
      setProviders(data.providers);
      setModels(data.models); // all providers' models from DB cache
      setWorkspacePath(data.workspacePath);
      setWorkspace(data.workspace);
      setTheme(getStoredTheme(data.theme));

      let nextConversations = data.conversations;
      if (nextConversations.length === 0) {
        const initial = await createConversation();
        nextConversations = [initial];
        setConversations(nextConversations);
      }
      setActiveConversationId(nextConversations[0]?.id);

      const activeProviderId = nextConversations[0]?.providerId;
      const providerToRefresh = data.providers.find((provider) => provider.id === activeProviderId) ?? data.providers[0];
      if (providerToRefresh) {
        const result = await refreshModels(providerToRefresh.id);
        setProviderHealth((current) => ({ ...current, [result.health.providerId]: result.health }));
        setModels((current) => replaceModelsForProvider(current, result.models, result.provider.id));
        setProviders((current) => replaceProvider(current, result.provider));
      }
    } catch (bootstrapError) {
      setError(getErrorMessage(bootstrapError));
    } finally {
      setBooting(false);
    }
  }

  async function handleCreateConversation() {
    try {
      const conversation = await createConversation();
      setConversations((current) => [conversation, ...current]);
      setActiveConversationId(conversation.id);
      setMessages([]);
      setView("chat");
    } catch (err) {
      setError(getErrorMessage(err));
    }
  }

  function handleConversationDeleted() {
    setConversations((current) => {
      const remaining = current.filter((c) => c.id !== activeConversationId);
      setActiveConversationId(remaining[0]?.id);
      return remaining;
    });
    setMessages([]);
  }

  function handleConversationImported(conversation: Conversation) {
    setConversations((current) => [conversation, ...current]);
    setActiveConversationId(conversation.id);
    setView("chat");
  }

  function handleConversationRenamed(conversation: Conversation) {
    setConversations((current) => current.map((c) => (c.id === conversation.id ? conversation : c)));
  }

  function handleMessagesChange(nextMessages: Message[]) {
    setMessages(nextMessages);
    const lastMessage = nextMessages[nextMessages.length - 1];
    if (!activeConversationId || !lastMessage) return;

    setConversations((current) =>
      current.map((conversation) =>
        conversation.id === activeConversationId
          ? {
              ...conversation,
              currentMessageId: lastMessage.id,
              providerId: lastMessage.providerId ?? conversation.providerId,
              modelId: lastMessage.modelId ?? conversation.modelId,
              updatedAt: lastMessage.updatedAt,
            }
          : conversation,
      ),
    );
  }

  function handleModelsRefresh(result: { health: ProviderHealth; models: ModelInfo[]; provider: ProviderConfig }) {
    setProviderHealth((current) => ({ ...current, [result.health.providerId]: result.health }));
    setModels((current) => replaceModelsForProvider(current, result.models, result.provider.id));
    setProviders((current) => replaceProvider(current, result.provider));
  }

  function handleProviderSaved(provider: ProviderConfig) {
    setProviders((current) => replaceProvider(current, provider));
  }

  async function handleThemeChange(nextTheme: ThemeMode) {
    setTheme(nextTheme);
    try {
      await setThemePreference(nextTheme);
    } catch (err) {
      setError(getErrorMessage(err));
    }
  }

  return (
    <>
      <div className="flex h-screen overflow-hidden bg-background text-foreground">
        <ConversationSidebar
          conversations={conversations}
          activeConversationId={activeConversationId}
          collapsed={sidebarCollapsed}
          focusSearchSignal={focusSearchSignal}
          onToggleCollapsed={() => setSidebarCollapsed((v) => !v)}
          onCreate={handleCreateConversation}
          onSelect={(id) => {
            setActiveConversationId(id);
            setView("chat");
          }}
          onOpenSettings={() => setView("settings")}
        />

        <React.Suspense fallback={<MainViewFallback />}>
          {view === "settings" ? (
            <SettingsView
              workspacePath={workspacePath}
              providers={providers}
              models={models}
              providerHealth={providerHealth}
              theme={theme}
              workspace={workspace}
              builtInStatus={builtInStatus}
              onBuiltInStatusChange={setBuiltInStatus}
              onThemeChange={handleThemeChange}
              onWorkspaceChange={setWorkspace}
              onProviderSaved={handleProviderSaved}
              onModelsRefresh={handleModelsRefresh}
              onBack={() => setView("chat")}
              onError={setError}
            />
          ) : (
            <ChatView
              conversation={activeConversation}
              messages={messages}
              providers={providers}
              models={models}
              providerHealth={providerHealth}
              isLoading={loadingMessages || booting}
              onMessagesChange={handleMessagesChange}
              onConversationDeleted={handleConversationDeleted}
              onConversationImported={handleConversationImported}
              onConversationRenamed={handleConversationRenamed}
              onModelsRefresh={handleModelsRefresh}
              onError={setError}
            />
          )}
        </React.Suspense>

        <RightPanel collapsed={rightPanelCollapsed} onToggle={() => setRightPanelCollapsed((v) => !v)} />
      </div>

      {error && (
        <div className="fixed bottom-4 left-1/2 z-50 w-[min(560px,calc(100vw-2rem))] -translate-x-1/2 rounded-lg border border-destructive/30 bg-card p-3 shadow-lg">
          <div className="flex items-start justify-between gap-3">
            <div>
              <div className="text-sm font-medium text-destructive">Ark needs attention</div>
              <div className="mt-1 text-sm text-muted-foreground">{error}</div>
            </div>
            <Button size="sm" variant="ghost" onClick={() => setError(null)}>
              Dismiss
            </Button>
          </div>
        </div>
      )}
    </>
  );
}

function replaceProvider(providers: ProviderConfig[], provider: ProviderConfig): ProviderConfig[] {
  const exists = providers.some((p) => p.id === provider.id);
  return exists ? providers.map((p) => (p.id === provider.id ? provider : p)) : [...providers, provider];
}

function replaceModelsForProvider(current: ModelInfo[], models: ModelInfo[], providerId: string): ModelInfo[] {
  return [...current.filter((m) => m.providerId !== providerId), ...models];
}

function MainViewFallback() {
  return (
    <section className="flex min-w-0 flex-1 items-center justify-center text-sm text-muted-foreground">
      Loading Ark
    </section>
  );
}

function getStoredTheme(fallback: ThemeMode = "dark"): ThemeMode {
  const stored = localStorage.getItem("ark.theme");
  return stored === "light" || stored === "dark" ? stored : fallback;
}
