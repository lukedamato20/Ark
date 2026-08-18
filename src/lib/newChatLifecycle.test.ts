import assert from "node:assert/strict";
import test from "node:test";
import { needsNewChatConfirmation } from "./newChatLifecycle.ts";

test("blank drafts never block deliberate New Chat actions", () => {
  assert.equal(needsNewChatConfirmation("", false), false);
  assert.equal(needsNewChatConfirmation("   ", false), false);
});

test("unsent text requires confirmation unless discard was explicitly confirmed", () => {
  assert.equal(needsNewChatConfirmation("draft", false), true);
  assert.equal(needsNewChatConfirmation("draft", true), false);
});
