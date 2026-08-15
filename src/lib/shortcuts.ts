/**
 * UX: the single source of truth for every keyboard shortcut Ark currently supports — consumed
 * by `useArkController.ts`'s global keydown handler (via `matchesShortcut`), `ShortcutsDialog`,
 * and Settings' Keyboard Shortcuts section, so the displayed list can never drift from what
 * actually fires. Lists exactly what exists today; no customization/rebinding, no shortcuts that
 * aren't already wired somewhere in the app.
 */
export type ShortcutId = "newChat" | "search" | "settings" | "sendMessage" | "closeMenu" | "showShortcuts";

export interface ShortcutDefinition {
  id: ShortcutId;
  /** `"Mod"` means Cmd on Mac / Ctrl elsewhere (see `formatShortcutKeys`/`detectIsMacPlatform` in
   * `platform.ts`, which render this for display); every other entry is either a literal
   * `KeyboardEvent.key` value (`"N"`, `"Enter"`, `"Escape"`, `"?"`, `","`) or `"Shift"`/`"Alt"`. */
  keys: string[];
  description: string;
}

export const SHORTCUTS: ShortcutDefinition[] = [
  { id: "newChat", keys: ["Mod", "N"], description: "New chat" },
  { id: "search", keys: ["Mod", "F"], description: "Search conversations" },
  { id: "settings", keys: ["Mod", ","], description: "Open Settings" },
  { id: "sendMessage", keys: ["Mod", "Enter"], description: "Send message (while composing)" },
  { id: "closeMenu", keys: ["Escape"], description: "Close the open menu, drawer, or dialog" },
  { id: "showShortcuts", keys: ["Shift", "?"], description: "Show this shortcuts reference" },
];

export function findShortcut(id: ShortcutId): ShortcutDefinition {
  const found = SHORTCUTS.find((shortcut) => shortcut.id === id);
  if (!found) throw new Error(`Unknown shortcut id: ${id}`);
  return found;
}

const MODIFIER_TOKENS = new Set(["Mod", "Shift", "Alt"]);

/** The minimal slice of `KeyboardEvent` `matchesShortcut` needs — a narrower type than the full
 * DOM `KeyboardEvent` so this pure function is trivially unit-testable with a plain object,
 * without a `jsdom`/browser environment. */
export interface ShortcutKeyboardEvent {
  key: string;
  metaKey: boolean;
  ctrlKey: boolean;
  shiftKey: boolean;
  altKey: boolean;
}

/** Matches a live keyboard event against a registry entry's `keys`. Single-character main keys
 * (`"N"`, `","`, `"?"`) compare case-insensitively; named keys (`"Enter"`, `"Escape"`) compare
 * exactly, matching `KeyboardEvent.key`'s own casing convention. */
export function matchesShortcut(event: ShortcutKeyboardEvent, keys: string[]): boolean {
  const wantsMod = keys.includes("Mod");
  const wantsShift = keys.includes("Shift");
  const wantsAlt = keys.includes("Alt");
  const mainKey = keys.find((key) => !MODIFIER_TOKENS.has(key));
  if (!mainKey) return false;
  if (wantsMod !== (event.metaKey || event.ctrlKey)) return false;
  if (wantsShift !== event.shiftKey) return false;
  if (wantsAlt !== event.altKey) return false;
  return mainKey.length === 1 ? event.key.toLowerCase() === mainKey.toLowerCase() : event.key === mainKey;
}
