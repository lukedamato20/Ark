import assert from "node:assert/strict";
import test from "node:test";
import type { ModelInfo, ProviderConfig } from "../types/ark.ts";
import { assessHardwareFit, presentModel } from "./modelPresentation.ts";

test("hardware fit is categorical and evidence based", () => {
  assert.equal(assessHardwareFit(4, 12, false).category, "excellent");
  assert.equal(assessHardwareFit(4, 8, false).category, "good");
  assert.equal(assessHardwareFit(4, 5, false).category, "constrained");
  assert.equal(assessHardwareFit(4, 3, false).category, "not_recommended");
});

test("remote or incomplete evidence is unknown", () => {
  assert.equal(assessHardwareFit(4, 12, true).category, "unknown");
  assert.equal(assessHardwareFit(null, 12, false).category, "unknown");
});

test("fit boundaries are stable and remote execution always fails toward unknown", () => {
  assert.equal(assessHardwareFit(100, 300, false).category, "excellent");
  assert.equal(assessHardwareFit(100, 299.99, false).category, "good");
  assert.equal(assessHardwareFit(100, 200, false).category, "good");
  assert.equal(assessHardwareFit(100, 199.99, false).category, "constrained");
  assert.equal(assessHardwareFit(100, 125, false).category, "constrained");
  assert.equal(assessHardwareFit(100, 124.99, false).category, "not_recommended");
  for (const memory of [0, 100, 1_000, Number.MAX_SAFE_INTEGER]) {
    const fit = assessHardwareFit(100, memory, true);
    assert.equal(fit.category, "unknown");
    assert.equal(fit.confidence, "insufficient");
  }
});

test("model presentation bounds and normalizes untrusted provider metadata", () => {
  const model = {
    providerId: "ollama-local",
    name: "example:latest",
    isAvailable: true,
    supportsStreaming: true,
    supportsTools: true,
    supportsVision: false,
    supportsEmbeddings: false,
    metadataJson: JSON.stringify({
      size: 4_000_000_000,
      details: { family: "llama", parameter_size: "8B", quantization_level: "Q4_K_M" },
      arkShow: { contextWindow: 32_768, licenseSummary: "Apache-2.0" },
    }),
  } as ModelInfo;
  const provider = {
    name: "Local Ollama",
    providerType: "ollama",
    destinationClass: "loopback",
    capabilities: { modelDelete: true, modelUnload: false },
  } as ProviderConfig;

  assert.deepEqual(presentModel(model, provider, 12_000_000_000), {
    schemaVersion: 1,
    displayName: "example:latest",
    providerId: "ollama-local",
    providerName: "Local Ollama",
    runtime: "ollama",
    available: true,
    capabilities: { streaming: true, tools: true, vision: false, embeddings: false },
    supportedActions: ["delete"],
    sizeBytes: 4_000_000_000,
    family: "llama",
    parameterSize: "8B",
    quantization: "Q4_K_M",
    contextWindow: 32_768,
    licenseSummary: "Apache-2.0",
    source: "provider",
    sourceLabel: "Provided by Local Ollama",
    metadataConfidence: "reported",
    fieldSources: {
      sizeBytes: "provider",
      family: "provider",
      parameterSize: "provider",
      quantization: "provider",
      contextWindow: "provider",
      licenseSummary: "provider",
    },
    sourceUrl: null,
    reviewedAt: null,
    fit: "excellent",
    fitReason: "Available memory is at least three times the model download size.",
    fitConfidence: "low",
    fitMethodVersion: "ark-fit-v1",
  });

  model.metadataJson = JSON.stringify({ details: { family: "x".repeat(257) }, arkShow: { licenseSummary: 7 } });
  const malformed = presentModel(model, provider, null);
  assert.equal(malformed.family, null);
  assert.equal(malformed.licenseSummary, null);
  assert.equal(malformed.fit, "unknown");
});

test("oversized metadata is rejected as a whole and missing fields retain per-field provenance", () => {
  const model = {
    providerId: "remote",
    name: "partial",
    isAvailable: true,
    supportsStreaming: true,
    supportsTools: false,
    supportsVision: false,
    supportsEmbeddings: false,
    contextWindow: 8_192,
    metadataJson: `{"padding":"${"x".repeat(128 * 1024)}"}`,
  } as ModelInfo;
  const provider = {
    name: "Remote provider",
    providerType: "openai_compatible",
    destinationClass: "public",
    capabilities: { modelDelete: false, modelUnload: false },
  } as ProviderConfig;

  const presentation = presentModel(model, provider, 64_000_000_000);
  assert.equal(presentation.source, "unavailable");
  assert.equal(presentation.metadataConfidence, "unavailable");
  assert.equal(presentation.fieldSources.family, "unavailable");
  assert.equal(presentation.fieldSources.contextWindow, "provider");
  assert.equal(presentation.fit, "unknown");
  assert.equal(presentation.fitConfidence, "insufficient");
});
