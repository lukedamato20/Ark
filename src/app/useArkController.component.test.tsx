import { act, renderHook, waitFor } from "@testing-library/react";
import type { PropsWithChildren } from "react";
import { expect, it, vi } from "vitest";
import { createFakeArkClient, type ArkClient } from "../lib/ArkClient";
import { ArkClientProvider } from "../lib/ArkClientContext";
import { createConversationOrganizationFixtureClient } from "../lib/developmentArkClient";
import { createArkStores, entityList } from "../state/arkStores";
import { ArkStateContext } from "../state/arkStateContext";
import type { Conversation } from "../types/ark";
import { useArkController } from "./useArkController";

function renderController(createConversation: ArkClient["createConversation"], overrides: Partial<ArkClient> = {}) {
  const stores = createArkStores();
  const client = createFakeArkClient({
    ...createConversationOrganizationFixtureClient(),
    createConversation,
    ...overrides,
  });
  const wrapper = ({ children }: PropsWithChildren) => (
    <ArkClientProvider client={client}>
      <ArkStateContext.Provider value={stores}>{children}</ArkStateContext.Provider>
    </ArkClientProvider>
  );
  const hook = renderHook(() => useArkController(), { wrapper });
  return { ...hook, stores };
}

function nextConversation(source: Conversation, id: string): Conversation {
  return { ...source, id, title: "New conversation", pinnedAt: null, projectId: null };
}

it("New Chat protects drafts, preserves active generation, and selects the returned durable identity", async () => {
  const createConversation = vi.fn<ArkClient["createConversation"]>();
  const harness = renderController(createConversation);
  await waitFor(() => expect(harness.stores.shell.getSnapshot().booting).toBe(false));
  const original = entityList(harness.stores.catalog.getSnapshot().conversations)[0];
  const created = nextConversation(original, "confirmed-new-chat");
  createConversation.mockResolvedValue(created);
  harness.stores.shell.set((state) => ({ ...state, chatComposerDraft: "private unsent draft" }));
  const generationBefore = {
    byMessageId: {
      "active-message": {
        conversationId: original.id,
        content: "partial",
        status: "streaming",
        revision: 2,
      },
    },
    activeMessageIdByConversation: { [original.id]: "active-message" },
  };
  harness.stores.generation.set(generationBefore);

  await act(() => harness.result.current.createConversation());
  expect(createConversation).not.toHaveBeenCalled();
  expect(harness.stores.shell.getSnapshot().newChatConfirmationRequested).toBe(true);
  expect(harness.stores.shell.getSnapshot().chatComposerDraft).toBe("private unsent draft");

  await act(() => harness.result.current.createConversation(true));
  expect(createConversation).toHaveBeenCalledTimes(1);
  expect(harness.stores.catalog.getSnapshot().activeId).toBe(created.id);
  expect(harness.stores.transcript.getSnapshot()).toMatchObject({ conversationId: created.id, messages: [] });
  expect(harness.stores.shell.getSnapshot()).toMatchObject({
    chatComposerDraft: "",
    newChatConfirmationRequested: false,
    view: "chat",
  });
  expect(harness.stores.generation.getSnapshot()).toEqual(generationBefore);
});

it("New Chat failure and overlapping dispatch leave all authoritative state intact", async () => {
  let rejectCreate: ((reason: Error) => void) | undefined;
  const pending = new Promise<Conversation>((_resolve, reject) => {
    rejectCreate = reject;
  });
  const createConversation = vi.fn<ArkClient["createConversation"]>().mockReturnValue(pending);
  const harness = renderController(createConversation);
  await waitFor(() => expect(harness.stores.shell.getSnapshot().booting).toBe(false));
  const catalogBefore = harness.stores.catalog.getSnapshot();
  const transcriptBefore = harness.stores.transcript.getSnapshot();
  harness.stores.shell.set((state) => ({ ...state, chatComposerDraft: "keep me" }));

  let first: Promise<void>;
  act(() => {
    first = harness.result.current.createConversation(true);
    void harness.result.current.createConversation(true);
  });
  expect(createConversation).toHaveBeenCalledTimes(1);
  rejectCreate?.(new Error("database unavailable"));
  await act(() => first!);

  expect(harness.stores.catalog.getSnapshot()).toEqual(catalogBefore);
  expect(harness.stores.transcript.getSnapshot()).toEqual(transcriptBefore);
  expect(harness.stores.shell.getSnapshot().chatComposerDraft).toBe("keep me");
  expect(harness.stores.shell.getSnapshot().error).toBe("database unavailable");
});

it("accent changes apply immediately, persist as a typed device setting, and roll back on failure", async () => {
  const createConversation = vi.fn<ArkClient["createConversation"]>();
  const updateDeviceSettings = vi
    .fn<ArkClient["updateDeviceSettings"]>()
    .mockImplementationOnce(async (settings) => settings)
    .mockRejectedValueOnce(new Error("settings write failed"));
  const harness = renderController(createConversation, { updateDeviceSettings });
  await waitFor(() => expect(harness.stores.shell.getSnapshot().booting).toBe(false));

  await act(() => harness.result.current.changeAccentPalette("teal"));
  expect(harness.stores.settings.getSnapshot().accentPalette).toBe("teal");
  expect(document.documentElement.dataset.accent).toBe("teal");
  expect(updateDeviceSettings).toHaveBeenLastCalledWith(expect.objectContaining({ accentPalette: "teal" }));

  await act(() => harness.result.current.changeAccentPalette("amber"));
  expect(harness.stores.settings.getSnapshot().accentPalette).toBe("teal");
  expect(document.documentElement.dataset.accent).toBe("teal");
  expect(harness.stores.shell.getSnapshot().error).toBe("settings write failed");
});
