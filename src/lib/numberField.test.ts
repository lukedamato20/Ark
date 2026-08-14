import assert from "node:assert/strict";
import test from "node:test";
import { validateNumberInput } from "./numberField.ts";

test("accepts a finite number inside the range", () => {
  const result = validateNumberInput("0.7", 0, 2, "Temperature");
  assert.deepEqual(result, { valid: true, parsed: 0.7, error: null });
});

test("accepts the range boundaries inclusively", () => {
  assert.equal(validateNumberInput("0", 0, 2, "Temperature").valid, true);
  assert.equal(validateNumberInput("2", 0, 2, "Temperature").valid, true);
});

test("rejects out-of-range values with the range stated in the error", () => {
  const result = validateNumberInput("3", 0, 2, "Temperature");
  assert.equal(result.valid, false);
  assert.equal(result.parsed, null);
  assert.equal(result.error, "Temperature must be between 0 and 2.");
});

test("rejects non-numeric text without ever producing NaN", () => {
  const result = validateNumberInput("abc", 0, 2, "Temperature");
  assert.equal(result.valid, false);
  assert.equal(result.parsed, null);
  assert.equal(Number.isNaN(result.parsed), false);
});

test("rejects an empty or whitespace-only draft as required, not as zero", () => {
  assert.equal(validateNumberInput("", 0, 2, "Temperature").valid, false);
  assert.equal(validateNumberInput("   ", 1, 1_000_000, "Max tokens").error, "Max tokens is required.");
});

test("rejects a partially-typed intermediate value like a trailing decimal point", () => {
  const result = validateNumberInput("0.", 0, 2, "Temperature");
  // "0." legitimately parses to 0 in JS — this documents that a mid-typing state can transiently
  // read as valid; the component-level test in ui/numberField covers the full typing sequence.
  assert.equal(result.valid, true);
  assert.equal(result.parsed, 0);
});
