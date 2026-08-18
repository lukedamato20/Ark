import assert from "node:assert/strict";
import test from "node:test";
import { SETTINGS_SECTIONS } from "./settingsSections.ts";

test("Settings exposes one first-class Tools section without placeholders", () => {
  const ids = SETTINGS_SECTIONS.map((section) => section.id);
  assert.equal(ids.filter((id) => id === "tools").length, 1);
  assert.ok(SETTINGS_SECTIONS.find((section) => section.id === "tools")?.description.includes("Capability grants"));
});
