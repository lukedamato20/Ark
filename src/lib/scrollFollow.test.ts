import assert from "node:assert/strict";
import test from "node:test";
import { AUTO_FOLLOW_THRESHOLD_PX, isNearBottom } from "./scrollFollow.ts";

test("reports near-bottom when the remaining scroll distance is within the threshold", () => {
  // scrollHeight 1000, clientHeight 800 -> 200px of scrollable distance total.
  assert.equal(isNearBottom(200, 1000, 800), true); // 0px remaining
  assert.equal(isNearBottom(80, 1000, 800), true); // 120px remaining, exactly at the threshold
  assert.equal(isNearBottom(79, 1000, 800), false); // 121px remaining, just past it
});

test("reports near-bottom for content that does not overflow the viewport at all", () => {
  assert.equal(isNearBottom(0, 400, 800), true);
});

test("respects a custom threshold", () => {
  assert.equal(isNearBottom(0, 1000, 800, 300), true); // 200px remaining, under a 300px threshold
  assert.equal(isNearBottom(0, 1000, 800, 100), false); // 200px remaining, over a 100px threshold
});

test("defaults to the exported threshold constant", () => {
  assert.equal(isNearBottom(1000 - 800 - AUTO_FOLLOW_THRESHOLD_PX, 1000, 800), true);
  assert.equal(isNearBottom(1000 - 800 - AUTO_FOLLOW_THRESHOLD_PX - 1, 1000, 800), false);
});
