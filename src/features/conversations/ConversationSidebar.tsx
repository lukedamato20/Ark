import { AnimatePresence, motion, useReducedMotion } from "framer-motion";
import {
  Archive,
  ArchiveRestore,
  Keyboard,
  MessageSquare,
  PanelLeftClose,
  PanelLeftOpen,
  Pin,
  PinOff,
  Plus,
  Search,
  Settings,
} from "lucide-react";
import * as React from "react";
import { formatDate } from "../../lib/format";
import { MOTION_FAST_SECONDS } from "../../lib/motionTokens";
import type { Conversation } from "../../types/ark";
import { Button } from "../../ui/button";
import { Input } from "../../ui/input";
import { cn } from "../../lib/cn";

interface ConversationSidebarProps {
  conversations: Conversation[];
  activeConversationId?: string;
  collapsed: boolean;
  /** UX-001: hides the internal rail/expanded toggle when this sidebar is rendered inside a
   * phone-width drawer, where collapsing to a rail doesn't make sense. */
  hideCollapseToggle?: boolean;
  focusSearchSignal: number;
  hasMore: boolean;
  isLoading: boolean;
  /** FTR-002: conversation id -> a short matched-text excerpt, shown instead of the date line
   * while a search query is active and this conversation matched on content, not just title. */
  searchSnippets: Record<string, string>;
  showArchived: boolean;
  onToggleCollapsed: () => void;
  onCreate: () => void;
  onSelect: (id: string) => void;
  onSearch: (query: string) => void;
  onLoadMore: () => void;
  onOpenSettings: () => void;
  onOpenShortcuts: () => void;
  onShowArchivedChange: (showArchived: boolean) => void;
  /** FTR-002: undo is calling this again with the opposite value. */
  onArchive: (id: string, archived: boolean) => void;
  onPin: (id: string, pinned: boolean) => void;
  shortcutsTriggerRef: React.RefObject<HTMLButtonElement | null>;
}

export function ConversationSidebar({
  conversations,
  activeConversationId,
  collapsed,
  hideCollapseToggle = false,
  focusSearchSignal,
  hasMore,
  isLoading,
  searchSnippets,
  showArchived,
  onToggleCollapsed,
  onCreate,
  onSelect,
  onSearch,
  onLoadMore,
  onOpenSettings,
  onOpenShortcuts,
  onShowArchivedChange,
  onArchive,
  onPin,
  shortcutsTriggerRef,
}: ConversationSidebarProps) {
  // UX-008: this AnimatePresence enter/exit previously ignored prefers-reduced-motion — only the
  // rail/expanded width transition (plain CSS, `motion-reduce:transition-none`) was covered.
  const reducedMotion = useReducedMotion();
  const [query, setQuery] = React.useState("");
  const searchInputRef = React.useRef<HTMLInputElement | null>(null);
  const itemRefs = React.useRef<Array<HTMLButtonElement | null>>([]);
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
      searchInputRef.current?.focus();
      searchInputRef.current?.select();
    }
  }, [collapsed, focusSearchSignal]);

  // FTR-002: pinned conversations sort first within the currently-loaded page — a pure
  // client-side re-sort of already-fetched rows, not a change to the backend's keyset-paginated
  // ORDER BY (see `build_conversation_page_query`'s own comment on why that boundary matters for
  // pagination correctness). Stable otherwise: everything else keeps the order the backend gave.
  const sortedConversations = React.useMemo(() => {
    const pinned = conversations.filter((item) => item.pinnedAt);
    pinned.sort((a, b) => (b.pinnedAt ?? "").localeCompare(a.pinnedAt ?? ""));
    const unpinned = conversations.filter((item) => !item.pinnedAt);
    return [...pinned, ...unpinned];
  }, [conversations]);

  /** FTR-002: arrow-key traversal through the visible result list — a roving focus move, not a
   * selection change (selection still happens on click/Enter, which native `<button>` semantics
   * already provide with no extra handling needed). */
  function handleListKeyDown(event: React.KeyboardEvent<HTMLElement>) {
    if (event.key !== "ArrowDown" && event.key !== "ArrowUp") return;
    event.preventDefault();
    const currentIndex = itemRefs.current.findIndex((element) => element === document.activeElement);
    const delta = event.key === "ArrowDown" ? 1 : -1;
    const nextIndex =
      currentIndex === -1 ? 0 : Math.min(Math.max(currentIndex + delta, 0), sortedConversations.length - 1);
    itemRefs.current[nextIndex]?.focus();
  }

  return (
    // NOTE (UX-001): plain CSS width transition, not framer-motion's `animate` prop — `animate`
    // reliably failed to commit a target width to the DOM for this persistently-mounted element
    // in this app/environment (confirmed by DOM inspection; see `Drawer.tsx` for the fuller
    // investigation of the same failure mode). The list items' `motion.button`s below are
    // unaffected since those animate through `AnimatePresence`'s enter/exit path instead.
    <aside
      aria-label="Conversations"
      style={{ width: collapsed ? 72 : 288 }}
      className="flex h-screen shrink-0 flex-col border-r border-border bg-card/80 transition-[width] duration-standard ease-out motion-reduce:transition-none"
    >
      <div className="flex h-14 items-center gap-2 border-b border-border px-3">
        {!hideCollapseToggle && (
          <Button size="icon" variant="ghost" onClick={onToggleCollapsed} aria-label="Toggle sidebar">
            {collapsed ? <PanelLeftOpen className="h-4 w-4" /> : <PanelLeftClose className="h-4 w-4" />}
          </Button>
        )}
        {!collapsed && <div className="text-sm font-semibold tracking-wide">Ark</div>}
      </div>

      <div className="space-y-2 p-3">
        <Button
          className="w-full justify-start"
          variant="primary"
          onClick={onCreate}
          aria-label={collapsed ? "New Chat" : undefined}
        >
          <Plus className="h-4 w-4" />
          {!collapsed && "New Chat"}
        </Button>
        {!collapsed && (
          <>
            <div className="relative">
              <Search className="pointer-events-none absolute left-2.5 top-2.5 h-4 w-4 text-muted-foreground" />
              <Input
                ref={searchInputRef}
                value={query}
                onChange={(event) => setQuery(event.target.value)}
                onKeyDown={(event) => {
                  if (event.key === "ArrowDown" && sortedConversations.length > 0) {
                    event.preventDefault();
                    itemRefs.current[0]?.focus();
                  }
                }}
                placeholder="Search conversations"
                maxLength={256}
                className="pl-8"
              />
            </div>
            <label className="flex items-center gap-1.5 px-0.5 text-xs text-muted-foreground">
              <input
                type="checkbox"
                checked={showArchived}
                onChange={(event) => onShowArchivedChange(event.target.checked)}
                className="h-3.5 w-3.5"
              />
              Show archived
            </label>
          </>
        )}
      </div>

      <nav
        aria-label="Conversation list"
        className="min-h-0 flex-1 overflow-y-auto px-2 pb-2"
        onKeyDown={handleListKeyDown}
      >
        <AnimatePresence initial={false}>
          {sortedConversations.map((conversation, index) => {
            const active = conversation.id === activeConversationId;
            const snippet = searchSnippets[conversation.id];
            return (
              <motion.div
                key={conversation.id}
                layout={!reducedMotion}
                initial={{ opacity: 0, y: 4 }}
                animate={{ opacity: 1, y: 0 }}
                exit={{ opacity: 0, y: -4 }}
                transition={reducedMotion ? { duration: 0 } : { duration: MOTION_FAST_SECONDS }}
                className="group relative mb-1"
              >
                <button
                  ref={(element) => {
                    itemRefs.current[index] = element;
                  }}
                  type="button"
                  aria-label={conversation.title}
                  aria-current={active ? "true" : undefined}
                  className={cn(
                    "flex w-full items-center gap-2 rounded-md px-2 py-2 text-left text-sm outline-none transition-colors",
                    "focus-visible:ring-2 focus-visible:ring-ring",
                    active
                      ? "bg-accent text-accent-foreground"
                      : "text-muted-foreground hover:bg-muted hover:text-foreground",
                  )}
                  onClick={() => onSelect(conversation.id)}
                >
                  <MessageSquare className="h-4 w-4 shrink-0" />
                  {!collapsed && (
                    <span className="min-w-0 flex-1 pr-10">
                      <span className="flex items-center gap-1">
                        {conversation.pinnedAt && <Pin className="h-3 w-3 shrink-0 text-primary" aria-label="Pinned" />}
                        <span className="block truncate">{conversation.title}</span>
                      </span>
                      <span className="block truncate text-xs text-muted-foreground">
                        {snippet ?? formatDate(conversation.updatedAt)}
                      </span>
                    </span>
                  )}
                </button>
                {!collapsed && (
                  <div className="absolute right-1 top-1/2 flex -translate-y-1/2 gap-0.5 opacity-0 focus-within:opacity-100 group-hover:opacity-100">
                    <button
                      type="button"
                      aria-label={conversation.pinnedAt ? "Unpin conversation" : "Pin conversation"}
                      onClick={(event) => {
                        event.stopPropagation();
                        onPin(conversation.id, !conversation.pinnedAt);
                      }}
                      className="rounded p-1 text-muted-foreground outline-none hover:bg-background hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring"
                    >
                      {conversation.pinnedAt ? <PinOff className="h-3.5 w-3.5" /> : <Pin className="h-3.5 w-3.5" />}
                    </button>
                    <button
                      type="button"
                      aria-label={conversation.archived ? "Unarchive conversation" : "Archive conversation"}
                      onClick={(event) => {
                        event.stopPropagation();
                        onArchive(conversation.id, !conversation.archived);
                      }}
                      className="rounded p-1 text-muted-foreground outline-none hover:bg-background hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring"
                    >
                      {conversation.archived ? (
                        <ArchiveRestore className="h-3.5 w-3.5" />
                      ) : (
                        <Archive className="h-3.5 w-3.5" />
                      )}
                    </button>
                  </div>
                )}
              </motion.div>
            );
          })}
        </AnimatePresence>

        {!collapsed && conversations.length === 0 && (
          <div className="px-3 py-6 text-sm text-muted-foreground">
            {isLoading ? "Searching conversations…" : "No conversations found."}
          </div>
        )}
        {!collapsed && hasMore && (
          <Button className="mt-2 w-full" size="sm" variant="ghost" disabled={isLoading} onClick={onLoadMore}>
            {isLoading ? "Loading…" : "Load more"}
          </Button>
        )}
      </nav>

      <div className="border-t border-border p-3 grid gap-1">
        <Button
          ref={shortcutsTriggerRef}
          className="w-full justify-start"
          variant="ghost"
          onClick={onOpenShortcuts}
          aria-label={collapsed ? "Keyboard shortcuts" : undefined}
        >
          <Keyboard className="h-4 w-4" />
          {!collapsed && "Keyboard shortcuts"}
        </Button>
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
