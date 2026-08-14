import assert from "node:assert/strict";
import test from "node:test";
import { computeAnnouncementDelta } from "./streamAnnouncement.ts";

test("announces only the newly added slice since the last announcement", () => {
  const result = computeAnnouncementDelta("Hello world", 5);
  assert.equal(result.delta, "world");
  assert.equal(result.nextLength, 11);
});

test("announces nothing and keeps the previous length when content hasn't grown", () => {
  const result = computeAnnouncementDelta("Hello", 5);
  assert.equal(result.delta, "");
  assert.equal(result.nextLength, 5);
});

test("trims the delta so a mid-word split doesn't announce a stray leading space", () => {
  const result = computeAnnouncementDelta("Hello wonderful world", 5);
  assert.equal(result.delta, "wonderful world");
});

test("announces the full content on the first tick, from a zero previous length", () => {
  const content = "First chunk of the response.";
  const result = computeAnnouncementDelta(content, 0);
  assert.equal(result.delta, content);
  assert.equal(result.nextLength, content.length);
});
