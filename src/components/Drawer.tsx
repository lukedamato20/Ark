import { AnimatePresence, motion, useReducedMotion } from "framer-motion";
import * as React from "react";

interface DrawerProps {
  open: boolean;
  onClose: () => void;
  side: "left" | "right";
  label: string;
  /** Focus returns here on close — standard menu/dialog keyboard pattern, matching the
   * conversation-actions overflow menu's existing behavior (`ChatView.tsx`'s `HeaderOverflowMenu`). */
  triggerRef: React.RefObject<HTMLElement | null>;
  widthPx: number;
  children: React.ReactNode;
}

/**
 * UX-001: the overlay-drawer presentation used for the sidebar and context panel at phone width
 * (and the context panel at compact width). Deliberately never unmounts `children` — only its
 * visual position/visibility changes — so a conversation list's scroll position, search text,
 * and selection survive opening and closing the drawer, per this task's own acceptance
 * criterion. `inert` (not `display: none`) keeps the content out of the tab order and hidden
 * from assistive tech while closed without removing it from the DOM.
 */
export function Drawer({ open, onClose, side, label, triggerRef, widthPx, children }: DrawerProps) {
  const panelRef = React.useRef<HTMLDivElement | null>(null);
  const reducedMotion = useReducedMotion();

  React.useEffect(() => {
    if (!open) return;
    // Move focus into the drawer on open — the panel itself is a valid landing spot; content
    // inside (search input, list items) remains reachable via normal tab order from there.
    panelRef.current?.focus();

    function handleKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape") {
        onClose();
        triggerRef.current?.focus();
      }
    }
    document.addEventListener("keydown", handleKeyDown);
    return () => document.removeEventListener("keydown", handleKeyDown);
  }, [open, onClose, triggerRef]);

  const offscreenX = side === "left" ? -widthPx : widthPx;

  return (
    <>
      <AnimatePresence>
        {open && (
          <motion.div
            aria-hidden="true"
            onClick={() => {
              onClose();
              triggerRef.current?.focus();
            }}
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            transition={reducedMotion ? { duration: 0 } : { duration: 0.15 }}
            className="fixed inset-0 z-40 bg-background/70 backdrop-blur-sm"
          />
        )}
      </AnimatePresence>
      {/*
        The plain outer element owns `inert`/positioning/dialog semantics. The slide itself is a
        plain CSS transform transition, not framer-motion's `animate` prop: in this framer-motion
        12.x setup, `animate={{x}}` on this element reliably failed to ever commit a transform to
        the DOM — not on mount (fixable with an explicit `initial`) and, more fundamentally, not
        on subsequent `open` toggles either (confirmed by direct DOM inspection: the computed
        transform stayed at its mount-time value long after the transition duration had elapsed,
        even though the updated `animate` prop was verifiably reaching this render). CSS avoids
        that failure mode entirely. `motion-reduce:transition-none` covers prefers-reduced-motion
        the same way `useReducedMotion()` does for the framer-motion-driven backdrop above.
      */}
      <div
        ref={panelRef}
        role="dialog"
        aria-modal="true"
        aria-label={label}
        tabIndex={-1}
        inert={!open}
        style={{ width: widthPx, [side]: 0, transform: `translateX(${open ? 0 : offscreenX}px)` }}
        className="fixed inset-y-0 z-50 flex flex-col bg-card shadow-2xl outline-none transition-transform duration-200 ease-out motion-reduce:transition-none"
      >
        {children}
      </div>
    </>
  );
}
