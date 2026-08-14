import { AnimatePresence, motion, useReducedMotion } from "framer-motion";
import { Keyboard, MessageSquare, PanelLeftClose, PanelLeftOpen, Plus, Search, Settings } from "lucide-react";
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
  onToggleCollapsed: () => void;
  onCreate: () => void;
  onSelect: (id: string) => void;
  onSearch: (query: string) => void;
  onLoadMore: () => void;
  onOpenSettings: () => void;
  onOpenShortcuts: () => void;
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
  onToggleCollapsed,
  onCreate,
  onSelect,
  onSearch,
  onLoadMore,
  onOpenSettings,
  onOpenShortcuts,
  shortcutsTriggerRef,
}: ConversationSidebarProps) {
  // UX-008: this AnimatePresence enter/exit previously ignored prefers-reduced-motion — only the
  // rail/expanded width transition (plain CSS, `motion-reduce:transition-none`) was covered.
  const reducedMotion = useReducedMotion();
  const [query, setQuery] = React.useState("");
  const searchInputRef = React.useRef<HTMLInputElement | null>(null);
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
          <div className="relative">
            <Search className="pointer-events-none absolute left-2.5 top-2.5 h-4 w-4 text-muted-foreground" />
            <Input
              ref={searchInputRef}
              value={query}
              onChange={(event) => setQuery(event.target.value)}
              placeholder="Search conversations"
              maxLength={256}
              className="pl-8"
            />
          </div>
        )}
      </div>

      <nav aria-label="Conversation list" className="min-h-0 flex-1 overflow-y-auto px-2 pb-2">
        <AnimatePresence initial={false}>
          {conversations.map((conversation) => {
            const active = conversation.id === activeConversationId;
            return (
              <motion.button
                key={conversation.id}
                layout={!reducedMotion}
                initial={{ opacity: 0, y: 4 }}
                animate={{ opacity: 1, y: 0 }}
                exit={{ opacity: 0, y: -4 }}
                transition={reducedMotion ? { duration: 0 } : { duration: MOTION_FAST_SECONDS }}
                aria-label={conversation.title}
                aria-current={active ? "true" : undefined}
                className={cn(
                  "mb-1 flex w-full items-center gap-2 rounded-md px-2 py-2 text-left text-sm outline-none transition-colors",
                  "focus-visible:ring-2 focus-visible:ring-ring",
                  active
                    ? "bg-accent text-accent-foreground"
                    : "text-muted-foreground hover:bg-muted hover:text-foreground",
                )}
                onClick={() => onSelect(conversation.id)}
              >
                <MessageSquare className="h-4 w-4 shrink-0" />
                {!collapsed && (
                  <span className="min-w-0 flex-1">
                    <span className="block truncate">{conversation.title}</span>
                    <span className="block text-xs text-muted-foreground">{formatDate(conversation.updatedAt)}</span>
                  </span>
                )}
              </motion.button>
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
