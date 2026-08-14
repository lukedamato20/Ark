import assert from "node:assert/strict";
import test from "node:test";
import { formatRelativeTime, isProviderHealthStale, PROVIDER_HEALTH_STALE_MS } from "./relativeTime.ts";

const NOW = new Date("2026-08-14T12:00:00Z");

test("formats sub-5-second differences as just now", () => {
  assert.equal(formatRelativeTime("2026-08-14T11:59:58Z", NOW), "just now");
});

test("formats seconds, minutes, hours, and days at their respective boundaries", () => {
  assert.equal(formatRelativeTime("2026-08-14T11:59:30Z", NOW), "30s ago");
  assert.equal(formatRelativeTime("2026-08-14T11:55:00Z", NOW), "5m ago");
  assert.equal(formatRelativeTime("2026-08-14T09:00:00Z", NOW), "3h ago");
  assert.equal(formatRelativeTime("2026-08-12T12:00:00Z", NOW), "2d ago");
});

test("never produces a negative or NaN duration for a timestamp at or after now", () => {
  assert.equal(formatRelativeTime("2026-08-14T12:00:00Z", NOW), "just now");
  assert.equal(formatRelativeTime("2026-08-14T12:05:00Z", NOW), "just now");
});

test("falls back to a neutral label for a malformed timestamp rather than throwing or printing NaN", () => {
  assert.equal(formatRelativeTime("not-a-timestamp", NOW), "unknown time");
  assert.equal(formatRelativeTime("", NOW), "unknown time");
});

test("is not stale just under the threshold and stale just over it", () => {
  const justUnder = new Date(NOW.getTime() - (PROVIDER_HEALTH_STALE_MS - 1000)).toISOString();
  const justOver = new Date(NOW.getTime() - (PROVIDER_HEALTH_STALE_MS + 1000)).toISOString();
  assert.equal(isProviderHealthStale(justUnder, NOW), false);
  assert.equal(isProviderHealthStale(justOver, NOW), true);
});

test("treats a malformed timestamp as stale, failing toward the safer caveat", () => {
  assert.equal(isProviderHealthStale("not-a-timestamp", NOW), true);
});
