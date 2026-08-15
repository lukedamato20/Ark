import * as React from "react";
import { detectIsMacPlatform, formatShortcutKeys } from "../lib/platform";
import { SHORTCUTS } from "../lib/shortcuts";
import { useModalKeyboardBehavior } from "../lib/useModalKeyboardBehavior";
import { Button } from "../ui/button";

interface ShortcutsDialogProps {
  open: boolean;
  onClose: () => void;
  triggerRef: React.RefObject<HTMLElement | null>;
}

/**
 * UX-007: makes the global shortcuts already wired in `useArkController.ts` (Mod+N/F/,) and the
 * composer's own Mod+Enter actually discoverable — previously the only on-screen hint was
 * "Ctrl/Cmd + Enter to send" under the composer, with no reference for the rest. OS-specific key
 * labels come from `formatShortcutKeys`/`detectIsMacPlatform` (pure, unit-tested); this component
 * only reads `navigator.userAgent` once and reuses the shared modal focus-trap/Escape hook so it
 * behaves identically to `Drawer.tsx`'s dialogs.
 */
export function ShortcutsDialog({ open, onClose, triggerRef }: ShortcutsDialogProps) {
  const panelRef = React.useRef<HTMLDivElement | null>(null);
  const isMac = React.useMemo(() => detectIsMacPlatform(navigator.userAgent), []);
  useModalKeyboardBehavior(open, panelRef, onClose, triggerRef);

  if (!open) return null;

  return (
    <>
      <div aria-hidden="true" onClick={onClose} className="fixed inset-0 z-40 bg-background/70 backdrop-blur-sm" />
      <div
        ref={panelRef}
        role="dialog"
        aria-modal="true"
        aria-label="Keyboard shortcuts"
        tabIndex={-1}
        className="fixed left-1/2 top-1/2 z-50 w-[min(420px,calc(100vw-2rem))] -translate-x-1/2 -translate-y-1/2 rounded-lg border border-border bg-card p-4 shadow-2xl outline-none"
      >
        <div className="mb-3 flex items-center justify-between gap-3">
          <h2 className="text-sm font-semibold">Keyboard shortcuts</h2>
          <Button size="sm" variant="ghost" onClick={onClose} aria-label="Close keyboard shortcuts">
            Close
          </Button>
        </div>
        <dl className="grid gap-2">
          {SHORTCUTS.map((shortcut) => (
            <div key={shortcut.description} className="flex items-center justify-between gap-3 text-sm">
              <dt className="text-muted-foreground">{shortcut.description}</dt>
              <dd>
                <kbd className="rounded border border-border bg-muted/60 px-1.5 py-0.5 font-mono text-xs">
                  {formatShortcutKeys(shortcut.keys, isMac)}
                </kbd>
              </dd>
            </div>
          ))}
        </dl>
      </div>
    </>
  );
}
