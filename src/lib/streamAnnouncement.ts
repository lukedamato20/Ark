/**
 * UX-006: pure delta logic for the streaming-response live region — kept separate from the
 * `setInterval` wiring in `useThrottledStreamAnnouncement` (`ChatMessageList.tsx`) so it's
 * unit-testable without a DOM/timers. Announcing the accumulated content on every token would
 * violate this task's own acceptance criterion ("do not read every token"); announcing the
 * *entire* accumulated text on every throttle tick would be just as bad for a long response (the
 * screen reader would re-read from the start each time). This computes only the new slice since
 * the last announcement.
 */
export function computeAnnouncementDelta(
  content: string,
  previousAnnouncedLength: number,
): { delta: string; nextLength: number } {
  if (content.length <= previousAnnouncedLength) {
    return { delta: "", nextLength: previousAnnouncedLength };
  }
  return { delta: content.slice(previousAnnouncedLength).trim(), nextLength: content.length };
}
