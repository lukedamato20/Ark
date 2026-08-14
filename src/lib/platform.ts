/**
 * UX-007: pure OS-key-label formatting for the shortcuts reference, kept separate from the
 * `navigator` read in `usePlatform()` (`ui/shortcutsDialog.tsx`) so the formatting itself is
 * unit-testable without mocking `navigator`.
 */
const MAC_SYMBOLS: Record<string, string> = { Mod: "⌘", Shift: "⇧", Alt: "⌥" };
const OTHER_LABELS: Record<string, string> = { Mod: "Ctrl", Shift: "Shift", Alt: "Alt" };

export function formatShortcutKeys(keys: string[], isMac: boolean): string {
  const labelled = keys.map((key) => (isMac ? (MAC_SYMBOLS[key] ?? key) : (OTHER_LABELS[key] ?? key)));
  return labelled.join(isMac ? "" : "+");
}

export function detectIsMacPlatform(userAgent: string): boolean {
  return /Mac|iPhone|iPad|iPod/.test(userAgent);
}
