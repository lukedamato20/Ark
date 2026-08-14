/**
 * FTR-009: pure formatting/staleness logic for `ProviderHealth.checkedAt` — kept DOM-free so it
 * can be unit-tested directly, the same separation `numberField.ts`/`reconciliation.ts` already
 * use for their own component-adjacent pure logic.
 */

/** Provider/model state older than this is shown as stale rather than trusted at face value. */
export const PROVIDER_HEALTH_STALE_MS = 5 * 60 * 1000;

/** A short, human "checked N ago" string. Never throws on a malformed timestamp — falls back to
 * a neutral label rather than rendering "NaN ago" or crashing the component that calls it. */
export function formatRelativeTime(isoTimestamp: string, now: Date = new Date()): string {
  const then = new Date(isoTimestamp).getTime();
  if (!Number.isFinite(then)) {
    return "unknown time";
  }

  const diffSeconds = Math.max(0, Math.round((now.getTime() - then) / 1000));
  if (diffSeconds < 5) return "just now";
  if (diffSeconds < 60) return `${diffSeconds}s ago`;

  const diffMinutes = Math.round(diffSeconds / 60);
  if (diffMinutes < 60) return `${diffMinutes}m ago`;

  const diffHours = Math.round(diffMinutes / 60);
  if (diffHours < 24) return `${diffHours}h ago`;

  const diffDays = Math.round(diffHours / 24);
  return `${diffDays}d ago`;
}

/** A malformed/missing timestamp is treated as stale — fail toward showing the user a caveat,
 * never toward silently trusting data with no verifiable age. */
export function isProviderHealthStale(isoTimestamp: string, now: Date = new Date()): boolean {
  const then = new Date(isoTimestamp).getTime();
  if (!Number.isFinite(then)) {
    return true;
  }
  return now.getTime() - then > PROVIDER_HEALTH_STALE_MS;
}
