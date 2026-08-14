import * as React from "react";

/**
 * UX-001: three layout tiers, not a continuous range — every layout decision in the app shell
 * branches on exactly one of these, never on a raw pixel value, so there is one place that
 * defines what "phone"/"compact"/"desktop" mean.
 *
 * - phone: below Tailwind's default `md` (768px) — single full-width main stack; sidebar and
 *   context panel are overlay drawers, not docked columns.
 * - compact: `md` to just below `xl` (768–1279px) — covers the declared 980×720 minimum.
 *   Sidebar defaults to its existing rail-collapsed (72px) state; context panel is a drawer.
 * - desktop: `xl` (1280px) and above — current docked three-column behavior.
 *
 * Pure and exported separately from the hook so the boundary logic itself is unit-testable
 * without mocking `window`/`matchMedia`.
 */
export type Breakpoint = "phone" | "compact" | "desktop";

export const BREAKPOINT_COMPACT_MIN_PX = 768;
export const BREAKPOINT_DESKTOP_MIN_PX = 1280;

export function classifyWidth(widthPx: number): Breakpoint {
  if (widthPx >= BREAKPOINT_DESKTOP_MIN_PX) return "desktop";
  if (widthPx >= BREAKPOINT_COMPACT_MIN_PX) return "compact";
  return "phone";
}

/**
 * `window.matchMedia` (not a `resize` listener) so the browser's own compositor coalesces
 * updates and this only re-renders on an actual tier change, never on every intermediate pixel
 * during a drag-resize.
 */
export function useBreakpoint(): Breakpoint {
  const [breakpoint, setBreakpoint] = React.useState<Breakpoint>(() =>
    typeof window === "undefined" ? "desktop" : classifyWidth(window.innerWidth),
  );

  React.useEffect(() => {
    const compactQuery = window.matchMedia(`(min-width: ${BREAKPOINT_COMPACT_MIN_PX}px)`);
    const desktopQuery = window.matchMedia(`(min-width: ${BREAKPOINT_DESKTOP_MIN_PX}px)`);

    function update() {
      setBreakpoint(desktopQuery.matches ? "desktop" : compactQuery.matches ? "compact" : "phone");
    }

    update();
    compactQuery.addEventListener("change", update);
    desktopQuery.addEventListener("change", update);
    return () => {
      compactQuery.removeEventListener("change", update);
      desktopQuery.removeEventListener("change", update);
    };
  }, []);

  return breakpoint;
}
