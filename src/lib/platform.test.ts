import assert from "node:assert/strict";
import test from "node:test";
import { detectIsMacPlatform, formatShortcutKeys } from "./platform.ts";

test("formats a modifier + letter on Mac using the compact symbol convention", () => {
  assert.equal(formatShortcutKeys(["Mod", "N"], true), "⌘N");
});

test("formats a modifier + letter on Windows/Linux using the plus-separated convention", () => {
  assert.equal(formatShortcutKeys(["Mod", "N"], false), "Ctrl+N");
});

test("formats a three-key combination on both platforms", () => {
  assert.equal(formatShortcutKeys(["Mod", "Shift", "N"], true), "⌘⇧N");
  assert.equal(formatShortcutKeys(["Mod", "Shift", "N"], false), "Ctrl+Shift+N");
});

test("detects common Apple platform user agent substrings", () => {
  assert.equal(detectIsMacPlatform("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7)"), true);
  assert.equal(detectIsMacPlatform("Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X)"), true);
  assert.equal(detectIsMacPlatform("Mozilla/5.0 (Windows NT 10.0; Win64; x64)"), false);
  assert.equal(detectIsMacPlatform("Mozilla/5.0 (X11; Linux x86_64)"), false);
});
