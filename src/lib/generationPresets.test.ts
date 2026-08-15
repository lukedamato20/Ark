import assert from "node:assert/strict";
import test from "node:test";
import { RESPONSE_STYLE_OPTIONS, TONE_OPTIONS } from "./generationPresets.ts";

test("response style options have unique values and non-empty labels", () => {
  const values = RESPONSE_STYLE_OPTIONS.map((option) => option.value);
  assert.equal(new Set(values).size, values.length);
  for (const option of RESPONSE_STYLE_OPTIONS) {
    assert.ok(option.label.trim().length > 0, `${option.value} must have a label`);
  }
});

test("tone options have unique values and non-empty labels", () => {
  const values = TONE_OPTIONS.map((option) => option.value);
  assert.equal(new Set(values).size, values.length);
  for (const option of TONE_OPTIONS) {
    assert.ok(option.label.trim().length > 0, `${option.value} must have a label`);
  }
});

test("response style options match the Rust allow-list exactly", () => {
  const values = RESPONSE_STYLE_OPTIONS.map((option) => option.value).sort();
  assert.deepEqual(values, ["balanced", "concise", "creative", "detailed", "explanatory", "technical"]);
});

test("tone options match the Rust allow-list exactly", () => {
  const values = TONE_OPTIONS.map((option) => option.value).sort();
  assert.deepEqual(values, ["casual", "direct", "friendly", "neutral", "professional"]);
});
