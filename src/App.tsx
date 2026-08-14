import * as React from "react";
import { RightPanel } from "./components/RightPanel";
import { useArkController, type ArkController } from "./app/useArkController";
import { ConversationSidebar } from "./features/conversations/ConversationSidebar";
import { buildWorkspaceDiagnostics, getWorkspaceRecoveryActions } from "./lib/workspaceRecovery";
import { entityList } from "./state/arkStores";
import { useStore, useStoreSelector } from "./state/externalStore";
import { useArkStores } from "./state/useArkStores";
import { Button } from "./ui/button";

const ChatView = React.lazy(() => import("./features/chat/ChatView").then((module) => ({ default: module.ChatView })));
const SettingsView = React.lazy(() =>
  import("./features/settings/SettingsView").then((module) => ({ default: module.SettingsView })),
);

export default function App() {
  const controller = useArkController();
  const stores = useArkStores();
  const view = useStoreSelector(stores.shell, (state) => state.view);

  return (
    <>
      <WorkspaceRecoveryBanner controller={controller} />
      <div className="flex h-screen overflow-hidden bg-background text-foreground">
        <ConversationSidebarContainer controller={controller} />
        <React.Suspense fallback={<MainViewFallback />}>
          {view === "settings" ? (
            <SettingsContainer controller={controller} />
          ) : (
            <ChatContainer controller={controller} />
          )}
        </React.Suspense>
        <RightPanelContainer controller={controller} />
      </div>
      <AppFeedback controller={controller} />
    </>
  );
}

function ConversationSidebarContainer({ controller }: { controller: ArkController }) {
  const stores = useArkStores();
  const catalog = useStore(stores.catalog);
  const shell = useStore(stores.shell);
  return (
    <ConversationSidebar
      conversations={entityList(catalog.conversations)}
      activeConversationId={catalog.activeId}
      collapsed={shell.sidebarCollapsed}
      focusSearchSignal={shell.focusSearchSignal}
      hasMore={catalog.nextCursor != null}
      isLoading={catalog.isLoading}
      onToggleCollapsed={controller.toggleSidebar}
      onCreate={() => void controller.createConversation()}
      onSelect={controller.selectConversation}
      onSearch={(query) => void controller.searchConversations(query)}
      onLoadMore={() => void controller.loadMoreConversations()}
      onOpenSettings={() => controller.setView("settings")}
    />
  );
}

function ChatContainer({ controller }: { controller: ArkController }) {
  const stores = useArkStores();
  const activeConversation = useStoreSelector(stores.catalog, (state) =>
    state.activeId ? state.conversations.byId[state.activeId] : undefined,
  );
  const transcript = useStore(stores.transcript);
  const providerState = useStore(stores.providers);
  const booting = useStoreSelector(stores.shell, (state) => state.booting);
  return (
    <ChatView
      conversation={activeConversation}
      messages={transcript.messages}
      providers={entityList(providerState.providers)}
      models={entityList(providerState.models)}
      providerHealth={providerState.health}
      isLoading={transcript.isLoading || booting}
      onMessagesChange={controller.setMessages}
      onConversationDeleted={controller.deleteActiveConversation}
      onConversationImported={controller.importConversation}
      onConversationRenamed={controller.renameConversation}
      onModelsRefresh={controller.refreshModels}
      onError={controller.setError}
      onInfo={controller.setInfo}
    />
  );
}

function SettingsContainer({ controller }: { controller: ArkController }) {
  const stores = useArkStores();
  const providerState = useStore(stores.providers);
  const settings = useStore(stores.settings);
  return (
    <SettingsView
      workspacePath={settings.workspacePath}
      providers={entityList(providerState.providers)}
      models={entityList(providerState.models)}
      providerHealth={providerState.health}
      theme={settings.theme}
      workspace={settings.workspace}
      builtInStatus={settings.builtInStatus}
      onBuiltInStatusChange={controller.setBuiltInStatus}
      builtInModelPath={settings.builtInModelPath}
      onBuiltInModelPathChange={controller.changeBuiltInModelPath}
      onThemeChange={controller.changeTheme}
      onWorkspaceChange={controller.setWorkspace}
      onProviderSaved={controller.saveProvider}
      onModelsRefresh={controller.refreshModels}
      onBack={() => controller.setView("chat")}
      onError={controller.setError}
    />
  );
}

function WorkspaceRecoveryBanner({ controller }: { controller: ArkController }) {
  const stores = useArkStores();
  const workspaceError = useStoreSelector(stores.settings, (state) => state.workspaceOpenError);
  const retrying = useStoreSelector(stores.settings, (state) => state.retryingWorkspace);
  const workspace = useStoreSelector(stores.settings, (state) => state.workspace);
  if (!workspaceError) return null;
  const recoveryActions = getWorkspaceRecoveryActions(workspaceError.code ?? "database_error");

  const copyDiagnostics = async () => {
    const diagnostics = buildWorkspaceDiagnostics(workspaceError, workspace, new Date().toISOString());
    try {
      await navigator.clipboard.writeText(diagnostics);
      controller.setInfo(
        "Workspace diagnostics copied. The report contains paths and error details, not chat content.",
      );
    } catch {
      controller.setError("Ark could not copy workspace diagnostics to the clipboard.");
    }
  };

  return (
    <div
      role="alert"
      className="fixed inset-x-0 top-0 z-[60] border-b border-destructive/40 bg-destructive/10 px-4 py-2.5 text-sm"
    >
      <div className="mx-auto flex max-w-4xl flex-wrap items-center justify-between gap-3">
        <div>
          <span className="font-medium text-destructive">Workspace database unavailable.</span>{" "}
          <span className="text-muted-foreground">
            {workspaceError.message ?? "Ark could not open your workspace database."} You're viewing a temporary session
            — nothing here will be saved.
          </span>
        </div>
        <div className="flex shrink-0 items-center gap-2">
          {recoveryActions.includes("retry") && (
            <Button size="sm" variant="secondary" onClick={() => void controller.retryWorkspace()} disabled={retrying}>
              {retrying ? "Retrying…" : "Retry"}
            </Button>
          )}
          {recoveryActions.includes("choose-workspace") && (
            <Button size="sm" variant="ghost" onClick={() => controller.setView("settings")}>
              Choose workspace
            </Button>
          )}
          {recoveryActions.includes("copy-diagnostics") && (
            <Button size="sm" variant="ghost" onClick={() => void copyDiagnostics()}>
              Copy diagnostics
            </Button>
          )}
        </div>
      </div>
    </div>
  );
}

function RightPanelContainer({ controller }: { controller: ArkController }) {
  const stores = useArkStores();
  const collapsed = useStoreSelector(stores.shell, (state) => state.rightPanelCollapsed);
  return <RightPanel collapsed={collapsed} onToggle={controller.toggleRightPanel} />;
}

function AppFeedback({ controller }: { controller: ArkController }) {
  const stores = useArkStores();
  const error = useStoreSelector(stores.shell, (state) => state.error);
  const info = useStoreSelector(stores.shell, (state) => state.info);
  if (error) {
    return (
      <div className="fixed bottom-4 left-1/2 z-50 w-[min(560px,calc(100vw-2rem))] -translate-x-1/2 rounded-lg border border-destructive/30 bg-card p-3 shadow-lg">
        <div className="flex items-start justify-between gap-3">
          <div>
            <div className="text-sm font-medium text-destructive">Ark needs attention</div>
            <div className="mt-1 text-sm text-muted-foreground">{error}</div>
          </div>
          <Button size="sm" variant="ghost" onClick={() => controller.setError(null)}>
            Dismiss
          </Button>
        </div>
      </div>
    );
  }
  if (!info) return null;
  return (
    <div
      role="status"
      className="fixed bottom-4 left-1/2 z-50 w-[min(560px,calc(100vw-2rem))] -translate-x-1/2 rounded-lg border border-border bg-card p-3 shadow-lg"
    >
      <div className="flex items-start justify-between gap-3">
        <div className="text-sm text-foreground">{info}</div>
        <Button size="sm" variant="ghost" onClick={() => controller.setInfo(null)}>
          Dismiss
        </Button>
      </div>
    </div>
  );
}

function MainViewFallback() {
  return (
    <section className="flex min-w-0 flex-1 items-center justify-center text-sm text-muted-foreground">
      Loading Ark
    </section>
  );
}
