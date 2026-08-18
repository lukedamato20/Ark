import assert from "node:assert/strict";
import test from "node:test";
import { activityLabel } from "./activityStates.ts";

test("activity states use concise public labels", () => {
  assert.equal(activityLabel("preparing"), "Preparing");
  assert.equal(activityLabel("provider"), "Waiting for provider");
  assert.equal(activityLabel("generating"), "Generating");
  assert.equal(activityLabel("approval"), "Waiting for approval");
});

test("only a trusted tool definition name is interpolated", () => {
  assert.equal(activityLabel("tool"), "Using tool");
  assert.equal(activityLabel("tool", "Repository search"), "Using Repository search");
  assert.equal(activityLabel("tool", '<img src=x onerror="alert(1)">'), "Using tool");
  assert.equal(activityLabel("tool", "x".repeat(65)), "Using tool");
});
