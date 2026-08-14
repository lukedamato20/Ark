import { AnimatePresence, motion } from "framer-motion";
import { ChevronLeft, ChevronRight, FileText, MemoryStick, Wrench } from "lucide-react";
import { Button } from "../ui/button";
import { Badge } from "../ui/badge";

interface RightPanelProps {
  collapsed: boolean;
  onToggle: () => void;
}

export function RightPanel({ collapsed, onToggle }: RightPanelProps) {
  return (
    // NOTE (UX-001): plain CSS width transition, not framer-motion's `animate` prop — see
    // `Drawer.tsx` and `ConversationSidebar.tsx` for the investigation of why `animate` on a
    // persistently-mounted element unreliably commits to the DOM in this app/environment. The
    // opacity fade below is unaffected: it goes through `AnimatePresence`'s enter/exit path.
    <aside
      aria-label="Context"
      style={{ width: collapsed ? 48 : 260 }}
      className="flex h-screen shrink-0 flex-col border-l border-border bg-card/70 transition-[width] duration-200 ease-out motion-reduce:transition-none"
    >
      <div className="flex h-14 items-center justify-between border-b border-border px-2">
        {!collapsed && <div className="px-2 text-sm font-semibold">Context</div>}
        <Button size="icon" variant="ghost" onClick={onToggle} aria-label="Toggle context panel">
          {collapsed ? <ChevronLeft className="h-4 w-4" /> : <ChevronRight className="h-4 w-4" />}
        </Button>
      </div>

      <AnimatePresence initial={false}>
        {!collapsed && (
          <motion.div
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            transition={{ duration: 0.14 }}
            className="space-y-3 p-3"
          >
            <div className="rounded-lg border border-border bg-background p-3">
              <div className="mb-2 flex items-center gap-2 text-sm font-medium">
                <FileText className="h-4 w-4" />
                Documents
              </div>
              <p className="text-xs text-muted-foreground">Reserved for local document chat in a later phase.</p>
            </div>
            <div className="rounded-lg border border-border bg-background p-3">
              <div className="mb-2 flex items-center gap-2 text-sm font-medium">
                <MemoryStick className="h-4 w-4" />
                Memory
              </div>
              <p className="text-xs text-muted-foreground">Reserved for explicit local memory controls.</p>
            </div>
            <div className="rounded-lg border border-border bg-background p-3">
              <div className="mb-2 flex items-center gap-2 text-sm font-medium">
                <Wrench className="h-4 w-4" />
                Tools
              </div>
              <p className="text-xs text-muted-foreground">Reserved for approved local tools.</p>
            </div>
            <Badge tone="muted">Future panels only</Badge>
          </motion.div>
        )}
      </AnimatePresence>
    </aside>
  );
}
