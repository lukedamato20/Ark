import { useVirtualizer } from "@tanstack/react-virtual";
import {
  Archive,
  ArchiveRestore,
  ChevronDown,
  ChevronRight,
  MessageSquare,
  PanelLeftClose,
  PanelLeftOpen,
  Pin,
  PinOff,
  Plus,
  Search,
  Settings,
  X,
} from "lucide-react";
import * as React from "react";
import { ArkBrand } from "../../ui/arkBrand";
import { cn } from "../../lib/cn";
import type { Conversation, Project } from "../../types/ark";
import { Button } from "../../ui/button";
import { Input } from "../../ui/input";

type SectionId = "pinned" | "projects" | "chats";

interface ConversationSidebarProps {
  conversations: Conversation[];
  pinnedConversations: Conversation[];
  projects: Project[];
  activeConversationId?: string;
  activeMode: "chat" | "code";
  collapsed: boolean;
  hideCollapseToggle?: boolean;
  focusSearchSignal: number;
  hasMore: boolean;
  isLoading: boolean;
  searchSnippets: Record<string, string>;
  showArchived: boolean;
  onToggleCollapsed: () => void;
  onCreate: (projectId?: string | null) => void;
  onCreateProject: (name: string) => Promise<void>;
  onSelect: (id: string) => void;
  onSearch: (query: string) => void;
  onProjectFilter: (projectId: string | null) => void;
  onLoadMore: () => void;
  onOpenSettings: () => void;
  onModeChange: (mode: "chat" | "code") => void;
  onShowArchivedChange: (showArchived: boolean) => void;
  onArchive: (id: string, archived: boolean) => void;
  onPin: (id: string, pinned: boolean) => void;
}

function readSectionState(): Record<SectionId, boolean> {
  try {
    const value = JSON.parse(localStorage.getItem("ark.sidebar.sections") ?? "null") as Partial<
      Record<SectionId, boolean>
    > | null;
    return { pinned: value?.pinned ?? true, projects: value?.projects ?? true, chats: value?.chats ?? true };
  } catch {
    return { pinned: true, projects: true, chats: true };
  }
}

export function ConversationSidebar(props: ConversationSidebarProps) {
  const {
    conversations,
    pinnedConversations,
    projects,
    activeConversationId,
    activeMode,
    collapsed,
    hideCollapseToggle = false,
    focusSearchSignal,
    hasMore,
    isLoading,
    searchSnippets,
    showArchived,
    onToggleCollapsed,
    onCreate,
    onCreateProject,
    onSelect,
    onSearch,
    onProjectFilter,
    onLoadMore,
    onOpenSettings,
    onModeChange,
    onShowArchivedChange,
    onArchive,
    onPin,
  } = props;
  const [query, setQuery] = React.useState("");
  const [searchOpen, setSearchOpen] = React.useState(false);
  const [sections, setSections] = React.useState(readSectionState);
  const [selectedProjectId, setSelectedProjectId] = React.useState<string | null>(null);
  const [creatingProject, setCreatingProject] = React.useState(false);
  const [projectName, setProjectName] = React.useState("");
  const searchInputRef = React.useRef<HTMLInputElement | null>(null);
  const searchTriggerRef = React.useRef<HTMLButtonElement | null>(null);
  const itemRefs = React.useRef<Array<HTMLButtonElement | null>>([]);
  const scrollElementRef = React.useRef<HTMLElement | null>(null);
  const onSearchRef = React.useRef(onSearch);

  React.useEffect(() => {
    onSearchRef.current = onSearch;
  }, [onSearch]);
  React.useEffect(() => {
    const timer = window.setTimeout(() => onSearchRef.current(query), 200);
    return () => window.clearTimeout(timer);
  }, [query]);
  React.useEffect(() => {
    if (!collapsed && focusSearchSignal > 0) {
      setSearchOpen(true);
      requestAnimationFrame(() => searchInputRef.current?.focus());
    }
  }, [collapsed, focusSearchSignal]);
  React.useEffect(() => {
    localStorage.setItem("ark.sidebar.sections", JSON.stringify(sections));
  }, [sections]);

  const pinned = React.useMemo(
    () => [...pinnedConversations].sort((a, b) => (b.pinnedAt ?? "").localeCompare(a.pinnedAt ?? "")),
    [pinnedConversations],
  );
  const chats = React.useMemo(
    () =>
      conversations.filter((item) => !item.pinnedAt && (!selectedProjectId || item.projectId === selectedProjectId)),
    [conversations, selectedProjectId],
  );
  const virtualizer = useVirtualizer({
    count: chats.length,
    getScrollElement: () => scrollElementRef.current,
    estimateSize: () => (collapsed ? 44 : 48),
    overscan: 8,
  });

  function toggleSection(section: SectionId) {
    setSections((current) => ({ ...current, [section]: !current[section] }));
  }

  async function createProject() {
    const name = projectName.trim();
    if (!name) return;
    try {
      await onCreateProject(name);
      setProjectName("");
      setCreatingProject(false);
    } catch {
      // The application callback owns user-visible error reporting; keep the draft open for retry.
    }
  }

  function closeSearch() {
    setSearchOpen(false);
    requestAnimationFrame(() => searchTriggerRef.current?.focus());
  }

  return (
    <aside
      aria-label="Ark navigation"
      style={{ width: collapsed ? 72 : 288 }}
      className="flex h-screen shrink-0 flex-col border-r border-border bg-card transition-[width] duration-standard motion-reduce:transition-none"
    >
      <div className="flex min-h-16 items-center gap-2 border-b border-border px-3">
        <ArkBrand compact={collapsed} className="min-w-0 flex-1" />
        {!hideCollapseToggle && (
          <Button
            size="icon"
            variant="ghost"
            onClick={onToggleCollapsed}
            aria-label={collapsed ? "Expand sidebar" : "Collapse sidebar"}
          >
            {collapsed ? <PanelLeftOpen className="h-4 w-4" /> : <PanelLeftClose className="h-4 w-4" />}
          </Button>
        )}
      </div>

      <div className="border-b border-border p-2">
        {collapsed ? (
          <div className="grid gap-1">
            <Button
              size="icon"
              variant={activeMode === "chat" ? "secondary" : "ghost"}
              onClick={() => onModeChange("chat")}
              aria-label="Ark Chat"
            >
              <MessageSquare className="h-4 w-4" />
            </Button>
            <Button
              size="icon"
              variant={activeMode === "code" ? "secondary" : "ghost"}
              onClick={() => onModeChange("code")}
              aria-label="Ark Code"
            >
              <span className="font-mono text-xs">&lt;/&gt;</span>
            </Button>
          </div>
        ) : (
          <div className="relative grid grid-cols-2 rounded-lg bg-muted p-1" role="group" aria-label="Ark mode">
            <div
              aria-hidden="true"
              className={cn(
                "pointer-events-none absolute inset-y-1 left-1 w-[calc(50%-4px)] rounded-md bg-card shadow-sm",
                "transition-transform duration-200 ease-in-out motion-reduce:transition-none",
                activeMode === "code" && "translate-x-full",
              )}
            />
            <Button
              size="sm"
              variant="ghost"
              aria-label="Ark Chat"
              aria-pressed={activeMode === "chat"}
              onClick={() => onModeChange("chat")}
              className="relative z-10"
            >
              Chat
            </Button>
            <Button
              size="sm"
              variant="ghost"
              aria-label="Ark Code"
              aria-pressed={activeMode === "code"}
              onClick={() => onModeChange("code")}
              className="relative z-10"
            >
              Code
            </Button>
          </div>
        )}
      </div>

      {!collapsed && (
        <div className="flex min-h-12 items-center gap-1 border-b border-border px-2">
          {searchOpen ? (
            <div className="relative min-w-0 flex-1">
              <Search className="pointer-events-none absolute left-2.5 top-2.5 h-4 w-4 text-muted-foreground" />
              <Input
                ref={searchInputRef}
                value={query}
                onChange={(event) => setQuery(event.target.value)}
                onKeyDown={(event) => {
                  if (event.key === "Escape") closeSearch();
                }}
                placeholder="Search conversations"
                maxLength={256}
                className="pl-8 pr-8"
              />
              <button
                type="button"
                aria-label="Collapse search"
                onClick={closeSearch}
                className="absolute right-2 top-2 rounded p-0.5 text-muted-foreground focus-visible:ring-2 focus-visible:ring-ring"
              >
                <X className="h-4 w-4" />
              </button>
            </div>
          ) : (
            <Button
              ref={searchTriggerRef}
              variant={query ? "secondary" : "ghost"}
              size="icon"
              onClick={() => {
                setSearchOpen(true);
                requestAnimationFrame(() => searchInputRef.current?.focus());
              }}
              aria-label="Search conversations"
            >
              <Search className="h-4 w-4" />
            </Button>
          )}
          <label className="ml-auto flex items-center gap-1 text-xs text-muted-foreground">
            <input
              type="checkbox"
              checked={showArchived}
              onChange={(event) => onShowArchivedChange(event.target.checked)}
            />
            Archived
          </label>
        </div>
      )}

      <div className="flex min-h-0 flex-1 flex-col overflow-hidden">
        {!collapsed && (
          <>
            <SidebarSection title="Pinned" open={sections.pinned} onToggle={() => toggleSection("pinned")}>
              {pinned.map((conversation) => (
                <ConversationRow
                  key={conversation.id}
                  conversation={conversation}
                  active={conversation.id === activeConversationId}
                  snippet={searchSnippets[conversation.id]}
                  projectName={projects.find((project) => project.id === conversation.projectId)?.name}
                  onSelect={onSelect}
                  onArchive={onArchive}
                  onPin={onPin}
                />
              ))}
              {pinned.length === 0 && <EmptyRow>No pinned chats</EmptyRow>}
            </SidebarSection>
            <SidebarSection
              title="Projects"
              open={sections.projects}
              onToggle={() => toggleSection("projects")}
              actionLabel="Create project"
              onAction={() => setCreatingProject(true)}
            >
              <button
                type="button"
                onClick={() => {
                  setSelectedProjectId(null);
                  onProjectFilter(null);
                }}
                aria-pressed={!selectedProjectId}
                className={cn(
                  "w-full rounded px-3 py-1.5 text-left text-sm",
                  !selectedProjectId ? "bg-accent text-accent-foreground" : "text-muted-foreground hover:bg-muted",
                )}
              >
                All chats
              </button>
              {projects
                .filter((project) => !project.archivedAt)
                .map((project) => (
                  <button
                    key={project.id}
                    type="button"
                    onClick={() => {
                      setSelectedProjectId(project.id);
                      onProjectFilter(project.id);
                    }}
                    aria-pressed={selectedProjectId === project.id}
                    className={cn(
                      "w-full truncate rounded px-3 py-1.5 text-left text-sm",
                      selectedProjectId === project.id
                        ? "bg-accent text-accent-foreground"
                        : "text-muted-foreground hover:bg-muted",
                    )}
                  >
                    {project.name}
                  </button>
                ))}
              {creatingProject && (
                <div className="flex gap-1 px-2 py-1">
                  <Input
                    autoFocus
                    value={projectName}
                    maxLength={120}
                    onChange={(event) => setProjectName(event.target.value)}
                    onKeyDown={(event) => {
                      if (event.key === "Enter") void createProject();
                      if (event.key === "Escape") setCreatingProject(false);
                    }}
                    placeholder="Project name"
                  />
                  <Button size="icon" onClick={() => void createProject()} aria-label="Save project">
                    <Plus className="h-4 w-4" />
                  </Button>
                </div>
              )}
            </SidebarSection>
          </>
        )}

        <section className="flex min-h-0 flex-1 flex-col" aria-labelledby="sidebar-chats-heading">
          {!collapsed && (
            <SectionHeader
              id="sidebar-chats-heading"
              title="Chats"
              open={sections.chats}
              contentId="sidebar-chats-content"
              onToggle={() => toggleSection("chats")}
              actionLabel="New Chat"
              onAction={() => onCreate(selectedProjectId)}
            />
          )}
          {(collapsed || sections.chats) && (
            <nav
              id="sidebar-chats-content"
              aria-label="Chats"
              ref={scrollElementRef}
              className="min-h-0 flex-1 overflow-y-auto px-2 pb-2"
            >
              <div style={{ height: virtualizer.getTotalSize(), position: "relative", width: "100%" }}>
                {virtualizer.getVirtualItems().map((virtualRow) => {
                  const conversation = chats[virtualRow.index];
                  return (
                    <div
                      key={conversation.id}
                      ref={virtualizer.measureElement}
                      data-index={virtualRow.index}
                      style={{
                        position: "absolute",
                        top: 0,
                        left: 0,
                        width: "100%",
                        transform: `translateY(${virtualRow.start}px)`,
                      }}
                    >
                      <ConversationRow
                        ref={(element) => {
                          itemRefs.current[virtualRow.index] = element;
                        }}
                        collapsed={collapsed}
                        conversation={conversation}
                        active={conversation.id === activeConversationId}
                        snippet={searchSnippets[conversation.id]}
                        projectName={projects.find((project) => project.id === conversation.projectId)?.name}
                        onSelect={onSelect}
                        onArchive={onArchive}
                        onPin={onPin}
                      />
                    </div>
                  );
                })}
              </div>
              {!collapsed && chats.length === 0 && <EmptyRow>{isLoading ? "Searching…" : "No chats found"}</EmptyRow>}
              {!collapsed && hasMore && (
                <Button className="mt-2 w-full" size="sm" variant="ghost" disabled={isLoading} onClick={onLoadMore}>
                  {isLoading ? "Loading…" : "Load more"}
                </Button>
              )}
            </nav>
          )}
        </section>
      </div>

      <div className="border-t border-border p-2">
        <Button
          className="w-full justify-start"
          variant="ghost"
          onClick={onOpenSettings}
          aria-label={collapsed ? "Settings" : undefined}
        >
          <Settings className="h-4 w-4" />
          {!collapsed && "Settings"}
        </Button>
      </div>
    </aside>
  );
}

function SectionHeader({
  id,
  title,
  open,
  contentId,
  onToggle,
  actionLabel,
  onAction,
}: {
  id?: string;
  title: string;
  open: boolean;
  contentId?: string;
  onToggle: () => void;
  actionLabel?: string;
  onAction?: () => void;
}) {
  return (
    <div className="flex items-center gap-1 px-2 py-1">
      <button
        id={id}
        type="button"
        aria-expanded={open}
        aria-controls={contentId}
        onClick={onToggle}
        className="flex min-w-0 flex-1 items-center gap-1 rounded px-1 py-1 text-left text-xs font-semibold uppercase tracking-wide text-muted-foreground focus-visible:ring-2 focus-visible:ring-ring"
      >
        {open ? <ChevronDown className="h-3.5 w-3.5" /> : <ChevronRight className="h-3.5 w-3.5" />}
        {title}
      </button>
      {onAction && (
        <button
          type="button"
          aria-label={actionLabel}
          onClick={onAction}
          className="rounded p-1 text-muted-foreground hover:bg-muted focus-visible:ring-2 focus-visible:ring-ring"
        >
          <Plus className="h-3.5 w-3.5" />
        </button>
      )}
    </div>
  );
}

function SidebarSection({
  title,
  open,
  onToggle,
  actionLabel,
  onAction,
  children,
}: React.PropsWithChildren<{
  title: string;
  open: boolean;
  onToggle: () => void;
  actionLabel?: string;
  onAction?: () => void;
}>) {
  const contentId = React.useId();
  return (
    <section className="border-b border-border/70">
      <SectionHeader
        title={title}
        open={open}
        contentId={contentId}
        onToggle={onToggle}
        actionLabel={actionLabel}
        onAction={onAction}
      />
      {open && (
        <div id={contentId} className="max-h-40 overflow-y-auto px-2 pb-2">
          {children}
        </div>
      )}
    </section>
  );
}

const ConversationRow = React.forwardRef<
  HTMLButtonElement,
  {
    conversation: Conversation;
    active: boolean;
    collapsed?: boolean;
    snippet?: string;
    projectName?: string;
    onSelect: (id: string) => void;
    onArchive: (id: string, archived: boolean) => void;
    onPin: (id: string, pinned: boolean) => void;
  }
>(function ConversationRow(
  { conversation, active, collapsed = false, snippet, projectName, onSelect, onArchive, onPin },
  ref,
) {
  return (
    <div className="group relative mb-1">
      <button
        ref={ref}
        type="button"
        aria-label={conversation.title}
        aria-current={active ? "page" : undefined}
        title={conversation.updatedAt}
        onClick={() => onSelect(conversation.id)}
        className={cn(
          "flex w-full items-center gap-2 rounded-md px-2 py-2 text-left text-sm outline-none focus-visible:ring-2 focus-visible:ring-ring",
          active ? "bg-accent text-accent-foreground" : "text-muted-foreground hover:bg-muted hover:text-foreground",
        )}
      >
        <MessageSquare className="h-4 w-4 shrink-0" />
        {!collapsed && (
          <span className="min-w-0 flex-1 pr-10">
            <span className="block truncate">{conversation.title}</span>
            {snippet && (
              <span className="block truncate text-xs text-muted-foreground">
                {projectName ? `${projectName} · ` : ""}
                {snippet}
              </span>
            )}
          </span>
        )}
      </button>
      {!collapsed && (
        <div className="absolute right-1 top-1/2 flex -translate-y-1/2 opacity-0 focus-within:opacity-100 group-hover:opacity-100">
          <button
            type="button"
            aria-label={conversation.pinnedAt ? `Unpin ${conversation.title}` : `Pin ${conversation.title}`}
            onClick={(event) => {
              event.stopPropagation();
              onPin(conversation.id, !conversation.pinnedAt);
            }}
            className="rounded p-1 focus-visible:ring-2 focus-visible:ring-ring"
          >
            {conversation.pinnedAt ? <PinOff className="h-3.5 w-3.5" /> : <Pin className="h-3.5 w-3.5" />}
          </button>
          <button
            type="button"
            aria-label={conversation.archived ? `Unarchive ${conversation.title}` : `Archive ${conversation.title}`}
            onClick={(event) => {
              event.stopPropagation();
              onArchive(conversation.id, !conversation.archived);
            }}
            className="rounded p-1 focus-visible:ring-2 focus-visible:ring-ring"
          >
            {conversation.archived ? <ArchiveRestore className="h-3.5 w-3.5" /> : <Archive className="h-3.5 w-3.5" />}
          </button>
        </div>
      )}
    </div>
  );
});

function EmptyRow({ children }: React.PropsWithChildren) {
  return <div className="px-3 py-2 text-xs text-muted-foreground">{children}</div>;
}
