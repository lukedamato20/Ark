import { Menu, PanelRight } from "lucide-react";
import * as React from "react";
import { Drawer } from "./components/Drawer";
import { RightPanel } from "./components/RightPanel";
import { ShortcutsDialog } from "./components/ShortcutsDialog";
import { useArkController, type ArkController } from "./app/useArkController";
import { ConversationSidebar } from "./features/conversations/ConversationSidebar";
import {
  buildBootstrapDiagnostics,
  buildWorkspaceDiagnostics,
  getWorkspaceRecoveryActions,
} from "./lib/workspaceRecovery";
import { useBreakpoint } from "./lib/useBreakpoint";
import { entityList } from "./state/arkStores";
import { useStore, useStoreSelector } from "./state/externalStore";
import { useArkStores } from "./state/useArkStores";
import { Button } from "./ui/button";
import { StatePanel } from "./ui/statePanel";
import type { AppErrorShape } from "./types/ark";

const ChatView = React.lazy(() => import("./features/chat/ChatView").then((module) => ({ default: module.ChatView })));
const SettingsView = React.lazy(() =>
  import("./features/settings/SettingsView").then((module) => ({ default: module.SettingsView })),
);

export default function App() {
  const controller = useArkController();
  const stores = useArkStores();
  const view = useStoreSelector(stores.shell, (state) => state.view);
  const bootstrapError = useStoreSelector(stores.shell, (state) => state.bootstrapError);
  const breakpoint = useBreakpoint();

  // UX-001: the sidebar is a docked column (rail or expanded, per the persisted preference) at
  // compact and desktop widths, and only becomes an overlay drawer at phone width. The context
  // panel is docked only at desktop width and is a drawer at both compact and phone — it has a
  // higher/earlier breakpoint than the sidebar because two permanently-docked side columns at
  // 768–1279px would leave too little room for chat, not because it is less important.
  const sidebarIsDrawer = breakpoint === "phone";
  const contextIsDrawer = breakpoint !== "desktop";

  // Deliberately local, transient state — not the persisted sidebarCollapsed/rightPanelCollapsed
  // preference used for docked mode. A drawer must default *closed* regardless of that
  // preference; reusing the docked-mode boolean directly would mean the sidebar drawer opens
  // covering the whole screen on first phone-width load whenever the persisted preference
  // happens to be "expanded".
  const [sidebarDrawerOpen, setSidebarDrawerOpen] = React.useState(false);
  const [contextDrawerOpen, setContextDrawerOpen] = React.useState(false);
  const sidebarTriggerRef = React.useRef<HTMLButtonElement | null>(null);
  const contextTriggerRef = React.useRef<HTMLButtonElement | null>(null);
  const shortcutsTriggerRef = React.useRef<HTMLButtonElement | null>(null);
  const shortcutsOpen = useStoreSelector(stores.shell, (state) => state.shortcutsOpen);

  // UX-004: a total bootstrap failure means nothing else below loaded — no conversations,
  // providers, or settings — so this replaces the entire shell rather than layering a banner on
  // top of what would otherwise be a confusingly empty chat view. Placed after every hook above
  // so the hook call order stays identical across renders regardless of this condition.
  // "view !== settings": lets the panel's own "Open Settings" action actually reach Settings —
  // otherwise this gate would keep re-showing the failure panel forever, since navigating there
  // doesn't clear `bootstrapError` (the underlying problem is still unresolved; only a
  // successful `bootstrap()` retry does that).
  if (bootstrapError && view !== "settings") {
    return <BootstrapFailurePanel error={bootstrapError} controller={controller} />;
  }

  return (
    <>
      <WorkspaceRecoveryBanner controller={controller} />
      {(sidebarIsDrawer || contextIsDrawer) && (
        <ShellTopBar
          showSidebarTrigger={sidebarIsDrawer}
          showContextTrigger={contextIsDrawer}
          onOpenSidebar={() => setSidebarDrawerOpen(true)}
          onOpenContext={() => setContextDrawerOpen(true)}
          sidebarTriggerRef={sidebarTriggerRef}
          contextTriggerRef={contextTriggerRef}
        />
      )}
      <div className="flex h-screen overflow-hidden bg-background text-foreground">
        {sidebarIsDrawer ? (
          <Drawer
            open={sidebarDrawerOpen}
            onClose={() => setSidebarDrawerOpen(false)}
            side="left"
            label="Conversations"
            triggerRef={sidebarTriggerRef}
            widthPx={288}
          >
            <ConversationSidebarContainer
              controller={controller}
              forceExpanded
              shortcutsTriggerRef={shortcutsTriggerRef}
            />
          </Drawer>
        ) : (
          <ConversationSidebarContainer controller={controller} shortcutsTriggerRef={shortcutsTriggerRef} />
        )}
        <React.Suspense fallback={<MainViewFallback />}>
          {view === "settings" ? (
            <SettingsContainer controller={controller} />
          ) : (
            <ChatContainer controller={controller} />
          )}
        </React.Suspense>
        {contextIsDrawer ? (
          <Drawer
            open={contextDrawerOpen}
            onClose={() => setContextDrawerOpen(false)}
            side="right"
            label="Context"
            triggerRef={contextTriggerRef}
            widthPx={260}
          >
            <RightPanel collapsed={false} onToggle={() => setContextDrawerOpen(false)} />
          </Drawer>
        ) : (
          <RightPanelContainer controller={controller} />
        )}
      </div>
      <AppFeedback controller={controller} />
      <ShortcutsDialog
        open={shortcutsOpen}
        onClose={() => controller.setShortcutsOpen(false)}
        triggerRef={shortcutsTriggerRef}
      />
    </>
  );
}

function ShellTopBar({
  showSidebarTrigger,
  showContextTrigger,
  onOpenSidebar,
  onOpenContext,
  sidebarTriggerRef,
  contextTriggerRef,
}: {
  showSidebarTrigger: boolean;
  showContextTrigger: boolean;
  onOpenSidebar: () => void;
  onOpenContext: () => void;
  sidebarTriggerRef: React.RefObject<HTMLButtonElement | null>;
  contextTriggerRef: React.RefObject<HTMLButtonElement | null>;
}) {
  // A fixed, always-reachable place to open the sidebar/context drawers — necessary because
  // each panel's own internal toggle button lives inside that panel, which is off-canvas and
  // `inert` while its drawer is closed, so it cannot itself be the way back in.
  return (
    // UX-008: h-12 bar (up from h-11) so the buttons below can be a full 44×44px touch target —
    // this bar is the app's most clearly "touch-first" surface, since it only renders at
    // phone/compact breakpoints in the first place.
    <div className="flex h-12 shrink-0 items-center justify-between border-b border-border bg-card/80 px-2">
      {showSidebarTrigger ? (
        <Button
          ref={sidebarTriggerRef}
          variant="ghost"
          onClick={onOpenSidebar}
          aria-label="Open conversations"
          className="h-11 w-11 p-0"
        >
          <Menu className="h-4 w-4" />
        </Button>
      ) : (
        <span />
      )}
      {showContextTrigger && (
        <Button
          ref={contextTriggerRef}
          variant="ghost"
          onClick={onOpenContext}
          aria-label="Open context panel"
          className="h-11 w-11 p-0"
        >
          <PanelRight className="h-4 w-4" />
        </Button>
      )}
    </div>
  );
}

function ConversationSidebarContainer({
  controller,
  forceExpanded = false,
  shortcutsTriggerRef,
}: {
  controller: ArkController;
  /** Inside a phone-width drawer (App.tsx): always show full content, and hide the internal
   * rail/expanded toggle — collapsing to a 72px rail inside an already-narrow drawer doesn't
   * make sense, and this must never mutate the persisted desktop/compact collapse preference. */
  forceExpanded?: boolean;
  shortcutsTriggerRef: React.RefObject<HTMLButtonElement | null>;
}) {
  const stores = useArkStores();
  const catalog = useStore(stores.catalog);
  const shell = useStore(stores.shell);
  return (
    <ConversationSidebar
      conversations={entityList(catalog.conversations)}
      activeConversationId={catalog.activeId}
      collapsed={forceExpanded ? false : shell.sidebarCollapsed}
      hideCollapseToggle={forceExpanded}
      focusSearchSignal={shell.focusSearchSignal}
      hasMore={catalog.nextCursor != null}
      isLoading={catalog.isLoading}
      searchSnippets={catalog.searchSnippets}
      showArchived={catalog.showArchived}
      onToggleCollapsed={controller.toggleSidebar}
      onCreate={() => void controller.createConversation()}
      onSelect={controller.selectConversation}
      onSearch={(query) => void controller.searchConversations(query)}
      onLoadMore={() => void controller.loadMoreConversations()}
      onOpenSettings={() => controller.setView("settings")}
      onOpenShortcuts={() => controller.setShortcutsOpen(true)}
      onShowArchivedChange={(showArchived) => void controller.setShowArchived(showArchived)}
      onArchive={(id, archived) => void controller.changeConversationArchived(id, archived)}
      onPin={(id, pinned) => void controller.changeConversationPinned(id, pinned)}
      shortcutsTriggerRef={shortcutsTriggerRef}
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
  const projectState = useStore(stores.projects);
  const booting = useStoreSelector(stores.shell, (state) => state.booting);
  const focusComposerSignal = useStoreSelector(stores.shell, (state) => state.focusComposerSignal);
  return (
    <ChatView
      conversation={activeConversation}
      messages={transcript.messages}
      providers={entityList(providerState.providers)}
      models={entityList(providerState.models)}
      providerHealth={providerState.health}
      projects={entityList(projectState.projects)}
      isLoading={transcript.isLoading || booting}
      focusComposerSignal={focusComposerSignal}
      onMessagesChange={controller.setMessages}
      onConversationDeleted={controller.deleteActiveConversation}
      onConversationImported={controller.importConversation}
      onConversationRenamed={controller.renameConversation}
      onConversationProjectChange={controller.changeConversationProject}
      onRefreshProviderModels={controller.refreshProviderModels}
      onError={controller.setError}
      onInfo={controller.setInfo}
    />
  );
}

function SettingsContainer({ controller }: { controller: ArkController }) {
  const stores = useArkStores();
  const providerState = useStore(stores.providers);
  const projectState = useStore(stores.projects);
  const settings = useStore(stores.settings);
  return (
    <SettingsView
      workspacePath={settings.workspacePath}
      providers={entityList(providerState.providers)}
      models={entityList(providerState.models)}
      providerHealth={providerState.health}
      projects={entityList(projectState.projects)}
      theme={settings.theme}
      workspace={settings.workspace}
      builtInStatus={settings.builtInStatus}
      onBuiltInStatusChange={controller.setBuiltInStatus}
      builtInModelPath={settings.builtInModelPath}
      onBuiltInModelPathChange={controller.changeBuiltInModelPath}
      crashCaptureEnabled={settings.crashCaptureEnabled}
      onCrashCaptureEnabledChange={controller.changeCrashCaptureEnabled}
      onThemeChange={controller.changeTheme}
      onWorkspaceChange={controller.setWorkspace}
      onProviderSaved={controller.saveProvider}
      onProjectSaved={controller.saveProject}
      onProjectDeleted={controller.removeProject}
      onRefreshProviderModels={controller.refreshProviderModels}
      onBack={() => controller.setView("chat")}
      onError={controller.setError}
    />
  );
}

/**
 * UX-004: a total bootstrap failure — `getAppBootstrap`/`getBuiltInRuntimeStatus` itself
 * rejecting, not the narrower `workspaceOpenError` `WorkspaceRecoveryBanner` below handles —
 * gets Retry, Open Settings (best-effort: Settings tolerates the all-defaults store state a
 * failed bootstrap leaves behind, the same state it already renders during the brief window
 * before a *successful* bootstrap finishes), and Copy diagnostics. There is no "Exit" action:
 * that needs `@tauri-apps/plugin-process`, which is not currently a dependency of this app, and
 * adding a new plugin/capability/permission surface was judged out of scope for this specific
 * gap — Retry plus Settings covers real recovery paths already.
 */
function BootstrapFailurePanel({ error, controller }: { error: AppErrorShape; controller: ArkController }) {
  const [retrying, setRetrying] = React.useState(false);

  async function retry() {
    setRetrying(true);
    try {
      await controller.bootstrap();
    } finally {
      setRetrying(false);
    }
  }

  async function copyDiagnostics() {
    const diagnostics = buildBootstrapDiagnostics(error, new Date().toISOString());
    try {
      await navigator.clipboard.writeText(diagnostics);
      controller.setInfo("Diagnostics copied. The report contains an error code and message, not chat content.");
    } catch {
      controller.setError("Ark could not copy diagnostics to the clipboard.");
    }
  }

  return (
    <>
      <div className="flex h-screen items-center justify-center bg-background text-foreground">
        <StatePanel
          role="alert"
          tone="error"
          title="Ark couldn't start up."
          description={error.message ?? "Something prevented Ark from loading your conversations and settings."}
          detail={error.code ? `Error code: ${error.code}` : undefined}
          actions={
            <>
              <Button size="sm" variant="primary" onClick={() => void retry()} disabled={retrying}>
                {retrying ? "Retrying…" : "Retry"}
              </Button>
              <Button size="sm" variant="secondary" onClick={() => controller.setView("settings")}>
                Open Settings
              </Button>
              <Button size="sm" variant="ghost" onClick={() => void copyDiagnostics()}>
                Copy diagnostics
              </Button>
            </>
          }
        />
      </div>
      {/* This screen replaces the normal shell entirely, so it needs its own feedback surface —
       * `AppFeedback` otherwise only mounts inside the shell `copyDiagnostics` above renders
       * into. */}
      <AppFeedback controller={controller} />
    </>
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

const INFO_TOAST_AUTO_DISMISS_MS = 6000;

function AppFeedback({ controller }: { controller: ArkController }) {
  const stores = useArkStores();
  const error = useStoreSelector(stores.shell, (state) => state.error);
  const info = useStoreSelector(stores.shell, (state) => state.info);

  // UX-004: info toasts auto-dismiss — they're confirmations ("import complete"), not
  // actionable failures, so there's nothing to leave on screen waiting for a response. Errors
  // deliberately do not auto-dismiss: an actionable failure disappearing before it's read would
  // defeat the point of showing it at all.
  React.useEffect(() => {
    if (!info) return;
    const timer = window.setTimeout(() => controller.setInfo(null), INFO_TOAST_AUTO_DISMISS_MS);
    return () => window.clearTimeout(timer);
  }, [info, controller]);

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
