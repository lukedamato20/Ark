import assert from "node:assert/strict";
import test from "node:test";
import { BREAKPOINT_COMPACT_MIN_PX, BREAKPOINT_DESKTOP_MIN_PX, classifyWidth } from "./useBreakpoint.ts";

test("classifies the five declared UX-001 test viewports correctly", () => {
  assert.equal(classifyWidth(390), "phone"); // 390×844
  assert.equal(classifyWidth(768), "compact"); // 768×1024
  assert.equal(classifyWidth(980), "compact"); // 980×720, the declared minimum
  assert.equal(classifyWidth(1280), "desktop"); // 1280×720
  assert.equal(classifyWidth(1920), "desktop"); // large desktop
});

test("boundaries are inclusive on the lower edge of each tier", () => {
  assert.equal(classifyWidth(BREAKPOINT_COMPACT_MIN_PX - 1), "phone");
  assert.equal(classifyWidth(BREAKPOINT_COMPACT_MIN_PX), "compact");
  assert.equal(classifyWidth(BREAKPOINT_DESKTOP_MIN_PX - 1), "compact");
  assert.equal(classifyWidth(BREAKPOINT_DESKTOP_MIN_PX), "desktop");
});

test("handles the extremes without throwing", () => {
  assert.equal(classifyWidth(0), "phone");
  assert.equal(classifyWidth(10_000), "desktop");
});
