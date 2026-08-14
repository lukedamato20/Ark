import { AnimatePresence, motion } from "framer-motion";
import { MessageSquare, PanelLeftClose, PanelLeftOpen, Plus, Search, Settings } from "lucide-react";
import * as React from "react";
import { formatDate } from "../../lib/format";
import type { Conversation } from "../../types/ark";
import { Button } from "../../ui/button";
import { Input } from "../../ui/input";
import { cn } from "../../lib/cn";

interface ConversationSidebarProps {
  conversations: Conversation[];
  activeConversationId?: string;
  collapsed: boolean;
  focusSearchSignal: number;
  hasMore: boolean;
  isLoading: boolean;
  onToggleCollapsed: () => void;
  onCreate: () => void;
  onSelect: (id: string) => void;
  onSearch: (query: string) => void;
  onLoadMore: () => void;
  onOpenSettings: () => void;
}

export function ConversationSidebar({
  conversations,
  activeConversationId,
  collapsed,
  focusSearchSignal,
  hasMore,
  isLoading,
  onToggleCollapsed,
  onCreate,
  onSelect,
  onSearch,
  onLoadMore,
  onOpenSettings,
}: ConversationSidebarProps) {
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
    <motion.aside
      animate={{ width: collapsed ? 72 : 288 }}
      transition={{ duration: 0.18 }}
      className="flex h-screen shrink-0 flex-col border-r border-border bg-card/80"
    >
      <div className="flex h-14 items-center gap-2 border-b border-border px-3">
        <Button size="icon" variant="ghost" onClick={onToggleCollapsed} aria-label="Toggle sidebar">
          {collapsed ? <PanelLeftOpen className="h-4 w-4" /> : <PanelLeftClose className="h-4 w-4" />}
        </Button>
        {!collapsed && <div className="text-sm font-semibold tracking-wide">Ark</div>}
      </div>

      <div className="space-y-2 p-3">
        <Button className="w-full justify-start" variant="primary" onClick={onCreate}>
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

      <div className="min-h-0 flex-1 overflow-y-auto px-2 pb-2">
        <AnimatePresence initial={false}>
          {conversations.map((conversation) => {
            const active = conversation.id === activeConversationId;
            return (
              <motion.button
                key={conversation.id}
                layout
                initial={{ opacity: 0, y: 4 }}
                animate={{ opacity: 1, y: 0 }}
                exit={{ opacity: 0, y: -4 }}
                transition={{ duration: 0.14 }}
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
      </div>

      <div className="border-t border-border p-3">
        <Button className="w-full justify-start" variant="ghost" onClick={onOpenSettings}>
          <Settings className="h-4 w-4" />
          {!collapsed && "Settings"}
        </Button>
      </div>
    </motion.aside>
  );
}
