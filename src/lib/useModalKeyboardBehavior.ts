import * as React from "react";

const FOCUSABLE_SELECTOR =
  'a[href], button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

/**
 * UX-007: shared Escape-to-close-with-focus-restore and Tab-containment behavior for any
 * `role="dialog" aria-modal="true"` surface (`Drawer.tsx`, `ShortcutsDialog.tsx`) — extracted
 * rather than duplicated so both actually honor their `aria-modal="true"` claim identically.
 * Also moves focus into `panelRef` on open, matching the pattern both callers already needed.
 */
export function useModalKeyboardBehavior(
  open: boolean,
  panelRef: React.RefObject<HTMLElement | null>,
  onClose: () => void,
  triggerRef: React.RefObject<HTMLElement | null>,
) {
  React.useEffect(() => {
    if (!open) return;
    panelRef.current?.focus();

    function handleKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape") {
        onClose();
        triggerRef.current?.focus();
        return;
      }

      if (event.key !== "Tab" || !panelRef.current) return;
      const focusable = Array.from(panelRef.current.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR));
      if (focusable.length === 0) {
        event.preventDefault();
        return;
      }
      const first = focusable[0];
      const last = focusable[focusable.length - 1];
      if (event.shiftKey && document.activeElement === first) {
        event.preventDefault();
        last.focus();
      } else if (!event.shiftKey && document.activeElement === last) {
        event.preventDefault();
        first.focus();
      }
    }
    document.addEventListener("keydown", handleKeyDown);
    return () => document.removeEventListener("keydown", handleKeyDown);
    // eslint-disable-next-line react-hooks/exhaustive-deps -- panelRef/triggerRef are stable refs
  }, [open, onClose]);
}
