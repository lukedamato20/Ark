/**
 * UX-003: pure decision logic for the message-list auto-follow behavior, kept separate from the
 * DOM/ResizeObserver wiring in `MessageScrollContainer.tsx` so the threshold itself is
 * unit-testable without a DOM.
 */
export const AUTO_FOLLOW_THRESHOLD_PX = 120;

export function isNearBottom(
  scrollTop: number,
  scrollHeight: number,
  clientHeight: number,
  thresholdPx: number = AUTO_FOLLOW_THRESHOLD_PX,
): boolean {
  return scrollHeight - scrollTop - clientHeight <= thresholdPx;
}
