import assert from "node:assert/strict";
import test from "node:test";
import { getErrorMessage, normalizeError } from "./arkErrors.ts";

test("normalizes typed, string, and unknown transport failures", () => {
  assert.deepEqual(normalizeError({ code: "database_busy", message: "Try again." }), {
    code: "database_busy",
    message: "Try again.",
  });
  assert.deepEqual(normalizeError("Tauri bridge unavailable"), {
    code: "unknown_error",
    message: "Tauri bridge unavailable",
  });
  assert.deepEqual(normalizeError({ message: 42 }), {
    code: "unknown_error",
    message: "Unexpected Ark error.",
  });
  assert.equal(getErrorMessage(null), "Unexpected Ark error.");
});
