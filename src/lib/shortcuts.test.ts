import assert from "node:assert/strict";
import test from "node:test";
import { findShortcut, matchesShortcut, SHORTCUTS, type ShortcutKeyboardEvent } from "./shortcuts.ts";

function keyEvent(overrides: Partial<ShortcutKeyboardEvent>): ShortcutKeyboardEvent {
  return { key: "", metaKey: false, ctrlKey: false, shiftKey: false, altKey: false, ...overrides };
}

test("every shortcut has a unique id and a non-empty description", () => {
  const ids = SHORTCUTS.map((shortcut) => shortcut.id);
  assert.equal(new Set(ids).size, ids.length);
  for (const shortcut of SHORTCUTS) {
    assert.ok(shortcut.description.trim().length > 0, `${shortcut.id} must have a description`);
    assert.ok(shortcut.keys.length > 0, `${shortcut.id} must declare at least one key`);
  }
});

test("no two shortcuts declare the exact same key combination", () => {
  const signatures = SHORTCUTS.map((shortcut) => [...shortcut.keys].sort().join("+"));
  assert.equal(new Set(signatures).size, signatures.length);
});

test("findShortcut returns the matching entry and throws for an unknown id", () => {
  assert.equal(findShortcut("newChat").description, "New chat");
  // @ts-expect-error deliberately invalid id to exercise the not-found path
  assert.throws(() => findShortcut("doesNotExist"));
});

test("matchesShortcut requires Mod plus the exact letter, case-insensitively", () => {
  const keys = findShortcut("newChat").keys; // ["Mod", "N"]
  assert.equal(matchesShortcut(keyEvent({ key: "n", ctrlKey: true }), keys), true);
  assert.equal(matchesShortcut(keyEvent({ key: "N", metaKey: true }), keys), true);
  assert.equal(matchesShortcut(keyEvent({ key: "n" }), keys), false, "no modifier must not match");
  assert.equal(
    matchesShortcut(keyEvent({ key: "n", ctrlKey: true, shiftKey: true }), keys),
    false,
    "extra Shift must not match",
  );
  assert.equal(matchesShortcut(keyEvent({ key: "m", ctrlKey: true }), keys), false, "wrong letter must not match");
});

test("matchesShortcut handles a named key with no modifier", () => {
  const keys = findShortcut("closeMenu").keys; // ["Escape"]
  assert.equal(matchesShortcut(keyEvent({ key: "Escape" }), keys), true);
  assert.equal(
    matchesShortcut(keyEvent({ key: "escape" }), keys),
    false,
    "named keys compare exactly, not case-insensitively",
  );
});

test("matchesShortcut requires Shift for the show-shortcuts combo and rejects a plain '?' without it", () => {
  const keys = findShortcut("showShortcuts").keys; // ["Shift", "?"]
  assert.equal(matchesShortcut(keyEvent({ key: "?", shiftKey: true }), keys), true);
  assert.equal(matchesShortcut(keyEvent({ key: "?", shiftKey: false }), keys), false);
});
