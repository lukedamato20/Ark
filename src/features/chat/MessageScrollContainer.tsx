import { ArrowDown } from "lucide-react";
import * as React from "react";
import { isNearBottom } from "../../lib/scrollFollow";
import { Button } from "../../ui/button";

interface MessageScrollContainerProps {
  children: React.ReactNode;
  /** Changes on conversation load/switch (not on every message append) — triggers an instant,
   * non-animated jump to the latest message, since that is a navigation, not "new content
   * arrived while reading." */
  resetKey: string;
  /** PERF-003: when truthy at the moment a DOM mutation is observed, that one mutation is
   * ignored for auto-follow/"new response" purposes and the flag is cleared. Set by a caller
   * right before triggering a content change that isn't "new content arrived" in the sense this
   * component cares about — specifically, `ChatMessageList`'s "Load earlier messages" prepend,
   * which adds content *above* the current view, not below it. Native CSS scroll-anchoring still
   * keeps the visible content stable for that case; only the auto-follow/new-response signaling
   * needs to be skipped. */
  suppressNextMutationRef?: React.RefObject<boolean>;
}

/**
 * UX-003: near-bottom auto-follow, reading-position preservation, and a "jump to latest"
 * control, all driven by a `MutationObserver` on the message content rather than a subscription
 * to message/generation state. This is deliberate for two reasons: per ARC-008, streaming deltas
 * are scoped to the individual message bubble specifically so a token cannot force a rerender
 * anywhere else — watching the DOM instead of the message data means this component follows
 * streaming output without subscribing to it, and the exact same code path naturally covers
 * every other case that changes content height (a new message being sent, a branch switch, a
 * regenerate) with no special-casing per event type. `MutationObserver` rather than
 * `ResizeObserver` specifically: `ResizeObserver` callbacks are dispatched as part of the
 * browser's rendering/paint steps and were confirmed, by direct testing, to never fire at all
 * for a tab that isn't actively compositing frames; `MutationObserver` callbacks are queued as a
 * microtask off the DOM mutation itself and do not have that dependency.
 *
 * Preserving the reading position when *not* following relies on the browser's native CSS
 * scroll-anchoring (on by default in Chromium/Firefox, not disabled anywhere in this app): when
 * content is inserted above or reflows, the browser adjusts `scrollTop` to keep the same visible
 * content in place. This component only decides *when* to deliberately move the scroll position
 * (follow-to-bottom, or the explicit jump-to-latest action); it never fights the browser over
 * position preservation while the user is reading.
 */
export function MessageScrollContainer({ children, resetKey, suppressNextMutationRef }: MessageScrollContainerProps) {
  const scrollRef = React.useRef<HTMLDivElement | null>(null);
  const contentRef = React.useRef<HTMLDivElement | null>(null);
  const [autoFollow, setAutoFollow] = React.useState(true);
  const [hasNewBelow, setHasNewBelow] = React.useState(false);
  const autoFollowRef = React.useRef(autoFollow);
  autoFollowRef.current = autoFollow;

  React.useEffect(() => {
    const el = scrollRef.current;
    if (!el) return;
    el.scrollTop = el.scrollHeight;
    setAutoFollow(true);
    setHasNewBelow(false);
  }, [resetKey]);

  React.useEffect(() => {
    const scrollEl = scrollRef.current;
    const contentEl = contentRef.current;
    if (!scrollEl || !contentEl) return;

    const observer = new MutationObserver(() => {
      if (suppressNextMutationRef?.current) {
        suppressNextMutationRef.current = false;
        return;
      }
      if (autoFollowRef.current) {
        scrollEl.scrollTop = scrollEl.scrollHeight;
      } else {
        setHasNewBelow(true);
      }
    });
    observer.observe(contentEl, { childList: true, subtree: true, characterData: true });
    return () => observer.disconnect();
  }, [suppressNextMutationRef]);

  function handleScroll() {
    const el = scrollRef.current;
    if (!el) return;
    const nearBottom = isNearBottom(el.scrollTop, el.scrollHeight, el.clientHeight);
    setAutoFollow(nearBottom);
    if (nearBottom) setHasNewBelow(false);
  }

  function jumpToLatest() {
    const el = scrollRef.current;
    if (!el) return;
    el.scrollTop = el.scrollHeight;
    setAutoFollow(true);
    setHasNewBelow(false);
  }

  return (
    <div className="relative min-h-0 flex-1">
      <div ref={scrollRef} onScroll={handleScroll} className="h-full overflow-y-auto px-4 py-5">
        <div ref={contentRef}>{children}</div>
      </div>
      {hasNewBelow && (
        <div className="pointer-events-none absolute inset-x-0 bottom-4 flex justify-center">
          <Button size="sm" variant="secondary" onClick={jumpToLatest} className="pointer-events-auto shadow-lg">
            <ArrowDown className="h-3.5 w-3.5" />
            New response — jump to latest
          </Button>
        </div>
      )}
    </div>
  );
}
