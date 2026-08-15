import assert from "node:assert/strict";
import test from "node:test";
import { SUGGESTED_OLLAMA_MODELS } from "./ollamaSuggestedModels.ts";

test("suggested Ollama models have unique tag names", () => {
  const names = SUGGESTED_OLLAMA_MODELS.map((model) => model.name);
  assert.equal(new Set(names).size, names.length);
});

test("suggested Ollama models have non-empty labels and descriptions", () => {
  for (const model of SUGGESTED_OLLAMA_MODELS) {
    assert.ok(model.label.trim().length > 0, `${model.name} must have a label`);
    assert.ok(model.description.trim().length > 0, `${model.name} must have a description`);
  }
});

test("suggested Ollama models have plausible positive sizes", () => {
  for (const model of SUGGESTED_OLLAMA_MODELS) {
    assert.ok(
      model.approxSizeGb > 0 && model.approxSizeGb < 100,
      `${model.name} has an implausible approxSizeGb: ${model.approxSizeGb}`,
    );
  }
});

test("suggested Ollama model tag names are non-empty and contain no whitespace", () => {
  for (const model of SUGGESTED_OLLAMA_MODELS) {
    assert.ok(model.name.trim().length > 0);
    assert.ok(!/\s/.test(model.name), `${model.name} must not contain whitespace`);
  }
});
