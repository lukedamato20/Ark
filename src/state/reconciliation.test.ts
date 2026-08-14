import assert from "node:assert/strict";
import test from "node:test";
import type { Conversation, Message, StreamEvent } from "../types/ark.ts";
import {
  createArkStores,
  entityCollection,
  entityList,
  messageWithGenerationOverlay,
  streamOverlayFromEvent,
  upsertEntity,
} from "./arkStores.ts";
import {
  classifyStreamEvent,
  isLatestRequest,
  mergeConversationPage,
  preserveSelectedConversation,
} from "./reconciliation.ts";

function conversation(id: string, title = id): Conversation {
  return {
    id,
    title,
    createdAt: "2026-08-14T00:00:00Z",
    updatedAt: "2026-08-14T00:00:00Z",
    archived: false,
  };
}

function message(id = "message-1"): Message {
  return {
    id,
    conversationId: "conversation-1",
    pathIndex: 0,
    role: "assistant",
    content: "base",
    status: "streaming",
    createdAt: "2026-08-14T00:00:00Z",
    updatedAt: "2026-08-14T00:00:00Z",
  };
}

function event(overrides: Partial<StreamEvent> = {}): StreamEvent {
  return {
    conversationId: "conversation-1",
    messageId: "message-1",
    delta: " delta",
    content: null,
    status: "streaming",
    error: null,
    revision: 1,
    schemaVersion: 1,
    ...overrides,
  };
}

test("stream revisions apply once and gaps request authoritative state", () => {
  assert.equal(classifyStreamEvent(undefined, event()), "apply-delta");
  assert.equal(classifyStreamEvent(1, event({ revision: 1 })), "ignore-duplicate");
  assert.equal(classifyStreamEvent(2, event({ revision: 1 })), "ignore-duplicate");
  assert.equal(classifyStreamEvent(1, event({ revision: 3 })), "refetch");
  assert.equal(classifyStreamEvent(undefined, event({ revision: null })), "refetch");
  assert.equal(classifyStreamEvent(8, event({ delta: null, revision: null, status: "complete" })), "apply-terminal");
});

test("request sequencing rejects stale history and transcript responses", () => {
  assert.equal(isLatestRequest(4, 4), true);
  assert.equal(isLatestRequest(3, 4), false);
  assert.equal(isLatestRequest(5, 4), false);
});

test("page merging deduplicates and refresh preserves a selected conversation absent from results", () => {
  const selected = conversation("selected", "Original");
  const refreshedSelected = conversation("selected", "Renamed");
  assert.deepEqual(
    mergeConversationPage([selected, conversation("a")], [conversation("a"), conversation("b")]).map((item) => item.id),
    ["selected", "a", "b"],
  );
  assert.equal(preserveSelectedConversation(selected, [refreshedSelected])?.title, "Renamed");
  assert.strictEqual(preserveSelectedConversation(selected, [conversation("other")]), selected);
});

test("normalized entity collections keep stable identities and deterministic ordering", () => {
  const originalA = conversation("a");
  const collection = entityCollection([originalA, conversation("b")]);
  const updated = upsertEntity(collection, conversation("b", "Renamed"));
  const appended = upsertEntity(updated, conversation("c"));

  assert.deepEqual(appended.ids, ["a", "b", "c"]);
  assert.deepEqual(
    entityList(appended).map((item) => item.title),
    ["a", "Renamed", "c"],
  );
  assert.strictEqual(appended.byId.a, originalA);
});

test("generation updates notify only the generation store and preserve unrelated entity references", () => {
  const stores = createArkStores();
  let catalogNotifications = 0;
  let settingsNotifications = 0;
  let generationNotifications = 0;
  stores.catalog.subscribe(() => catalogNotifications++);
  stores.settings.subscribe(() => settingsNotifications++);
  stores.generation.subscribe(() => generationNotifications++);

  const firstOverlay = {
    conversationId: "conversation-1",
    content: "first",
    status: "streaming",
    revision: 1,
  };
  stores.generation.set({
    byMessageId: { first: firstOverlay },
    activeMessageIdByConversation: { "conversation-1": "first" },
  });
  const selectedFirst = stores.generation.getSnapshot().byMessageId.first;
  stores.generation.set((current) => ({
    ...current,
    byMessageId: {
      ...current.byMessageId,
      second: {
        conversationId: "conversation-2",
        content: "second",
        status: "streaming",
        revision: 1,
      },
    },
  }));

  assert.equal(catalogNotifications, 0);
  assert.equal(settingsNotifications, 0);
  assert.equal(generationNotifications, 2);
  assert.strictEqual(stores.generation.getSnapshot().byMessageId.first, selectedFirst);
});

test("delta overlays accumulate and terminal events without content preserve the accumulated text", () => {
  const base = message();
  const first = streamOverlayFromEvent(undefined, base, event({ delta: " one", revision: 1 }));
  const second = streamOverlayFromEvent(first, base, event({ delta: " two", revision: 2 }));
  const terminal = streamOverlayFromEvent(
    second,
    base,
    event({ delta: null, content: null, revision: null, status: "complete" }),
  );
  assert.equal(second.content, "base one two");
  assert.equal(terminal.content, "base one two");
  assert.deepEqual(messageWithGenerationOverlay(base, terminal), {
    ...base,
    content: "base one two",
    status: "complete",
  });
});

test("one thousand immediate completions preserve the placeholder and converge idempotently", () => {
  for (let run = 0; run < 1_000; run += 1) {
    const base = message(`message-${run}`);
    const deltaEvent = event({ messageId: base.id, delta: "ok", revision: 1 });
    assert.equal(classifyStreamEvent(undefined, deltaEvent), "apply-delta");
    const streaming = streamOverlayFromEvent(undefined, base, deltaEvent);
    assert.equal(classifyStreamEvent(streaming.revision, deltaEvent), "ignore-duplicate");

    const terminalEvent = event({
      messageId: base.id,
      delta: null,
      content: "baseok",
      status: "complete",
      revision: null,
    });
    const complete = messageWithGenerationOverlay(base, streamOverlayFromEvent(streaming, base, terminalEvent));
    const duplicate = messageWithGenerationOverlay(base, streamOverlayFromEvent(undefined, complete, terminalEvent));
    assert.equal(complete.status, "complete", `run ${run}`);
    assert.equal(complete.content, "baseok", `run ${run}`);
    assert.deepEqual(duplicate, complete, `duplicate terminal run ${run}`);
  }
});
