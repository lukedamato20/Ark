import assert from "node:assert/strict";
import test from "node:test";
import { formatGenerationSettingSource, parseGenerationProvenance } from "./generationProvenance.ts";

test("parses recognized generation provenance fields", () => {
  assert.deepEqual(
    parseGenerationProvenance(
      JSON.stringify({
        projectId: "project-1",
        personaId: "persona-1",
        personaVersion: 3,
        temperature: 0.2,
        temperatureSource: "project",
        maxTokens: 4096,
        maxTokensSource: "provider_default",
        systemPromptSource: "application",
        responseStyle: "technical",
        responseStyleSource: "persona",
        tone: "direct",
        toneSource: "conversation",
      }),
    ),
    {
      projectId: "project-1",
      personaId: "persona-1",
      personaVersion: 3,
      temperature: 0.2,
      temperatureSource: "project",
      maxTokens: 4096,
      maxTokensSource: "provider_default",
      systemPromptSource: "application",
      responseStyle: "technical",
      responseStyleSource: "persona",
      tone: "direct",
      toneSource: "conversation",
    },
  );
});

test("fails closed for malformed JSON and ignores invalid scalar claims", () => {
  assert.equal(parseGenerationProvenance("{"), null);
  assert.equal(parseGenerationProvenance(JSON.stringify({ temperature: "hot", temperatureSource: "attacker" })), null);
});

test("formats the provider-default source for people", () => {
  assert.equal(formatGenerationSettingSource("provider_default"), "provider default");
  assert.equal(formatGenerationSettingSource("persona"), "persona");
  assert.equal(formatGenerationSettingSource(null), null);
});
