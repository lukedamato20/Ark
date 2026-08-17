import assert from "node:assert/strict";
import test from "node:test";
import type { ProviderConfig } from "../types/ark";
import { buildRemoteRequestDisclosure } from "./providerDisclosure.ts";

function provider(destinationClass: ProviderConfig["destinationClass"]): ProviderConfig {
  return {
    id: "provider",
    name: "Provider",
    providerType: "openai",
    baseUrl: "https://api.openai.com",
    apiKeyRef: null,
    defaultModelId: null,
    defaultTemperature: 0.7,
    defaultMaxTokens: 2048,
    isLocal: destinationClass === "loopback",
    allowInsecureRemote: false,
    destinationClass,
    capabilities: {
      streaming: true,
      modelListing: true,
      modelPull: false,
      modelDelete: false,
      modelUnload: false,
      requiresAuth: true,
      reportsContextWindow: false,
      vision: false,
      embeddings: false,
      tools: false,
    },
    isUserManaged: true,
    isEnabled: true,
    createdAt: "2026-08-17T00:00:00Z",
    updatedAt: "2026-08-17T00:00:00Z",
  };
}

test("local providers do not show an outbound disclosure", () => {
  assert.equal(buildRemoteRequestDisclosure(provider("loopback"), "model", 1, true), null);
});

test("remote disclosure names endpoint route model and all outbound context categories", () => {
  const disclosure = buildRemoteRequestDisclosure(provider("public"), "gpt-test", 2, true);
  assert.deepEqual(disclosure, {
    endpoint: "https://api.openai.com",
    route: "POST /v1/chat/completions",
    model: "gpt-test",
    contextItems: [
      "current message",
      "active conversation history",
      "configured app/project/persona/conversation instructions",
      "2 staged attachment(s)",
      "approved web-search query/results",
    ],
  });
});
