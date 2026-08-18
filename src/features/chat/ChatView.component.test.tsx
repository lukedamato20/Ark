import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, it, vi } from "vitest";
import { createFakeArkClient, type ArkClient } from "../../lib/ArkClient";
import { ArkClientProvider } from "../../lib/ArkClientContext";
import type { Conversation, ModelInfo, ProviderConfig } from "../../types/ark";
import { ChatView } from "./ChatView";

const timestamp = "2026-08-17T12:00:00Z";

function provider(id: string, name: string, destinationClass: ProviderConfig["destinationClass"]): ProviderConfig {
  return {
    id,
    name,
    providerType: id === "local" ? "ollama" : "openai_compatible",
    baseUrl: destinationClass === "loopback" ? "http://127.0.0.1:11434" : "https://provider.example/v1",
    defaultModelId: `${id}-model`,
    defaultTemperature: 0.7,
    defaultMaxTokens: 2_048,
    isLocal: destinationClass === "loopback",
    allowInsecureRemote: false,
    destinationClass,
    capabilities: {
      streaming: true,
      modelListing: true,
      modelPull: false,
      modelDelete: false,
      modelUnload: false,
      requiresAuth: destinationClass !== "loopback",
      reportsContextWindow: true,
      vision: false,
      embeddings: false,
      tools: true,
    },
    isUserManaged: destinationClass !== "loopback",
    isEnabled: true,
    createdAt: timestamp,
    updatedAt: timestamp,
  };
}

function model(providerId: string): ModelInfo {
  return {
    id: `${providerId}-model-id`,
    providerId,
    name: `${providerId}-model`,
    displayName: `${providerId === "local" ? "Local" : "Cloud"} model`,
    contextWindow: 8_192,
    supportsStreaming: true,
    supportsTools: true,
    toolCallingMode: "native",
    supportsVision: false,
    supportsEmbeddings: false,
    isAvailable: true,
    lastSeenAt: timestamp,
    createdAt: timestamp,
    updatedAt: timestamp,
  };
}

it("uses one composer picker and sends the exact selected provider/model while retaining header route status", async () => {
  const user = userEvent.setup();
  const local = provider("local", "Local Ollama", "loopback");
  const cloud = provider("cloud", "Cloud provider", "public");
  const conversation = {
    id: "conversation",
    title: "Routing test",
    providerId: local.id,
    modelId: "local-model",
    archived: false,
    createdAt: timestamp,
    updatedAt: timestamp,
  } as Conversation;
  const sendChatMessage = vi.fn<ArkClient["sendChatMessage"]>().mockResolvedValue({
    conversationId: conversation.id,
    userMessageId: "user-message",
    assistantMessageId: "assistant-message",
  });
  const onMessagesChange = vi.fn();
  const client = createFakeArkClient({ sendChatMessage });

  render(
    <ArkClientProvider client={client}>
      <ChatView
        conversation={conversation}
        messages={[]}
        providers={[local, cloud]}
        models={[model(local.id), model(cloud.id)]}
        providerHealth={{
          local: { providerId: "local", isReachable: true, status: "ok", message: "Local ready", checkedAt: timestamp },
          cloud: { providerId: "cloud", isReachable: true, status: "ok", message: "Cloud ready", checkedAt: timestamp },
        }}
        projects={[]}
        personas={[]}
        isLoading={false}
        hasMoreOlderMessages={false}
        isLoadingOlderMessages={false}
        onLoadOlderMessages={async () => undefined}
        focusComposerSignal={0}
        composerDraft=""
        onMessagesChange={onMessagesChange}
        onConversationDeleted={vi.fn()}
        onConversationImported={vi.fn()}
        onConversationRenamed={vi.fn()}
        onConversationProjectChange={async () => undefined}
        onConversationPersonaChange={async () => undefined}
        onRefreshProviderModels={async () => undefined}
        onError={vi.fn()}
        onInfo={vi.fn()}
        onDraftChange={vi.fn()}
      />
    </ArkClientProvider>,
  );

  await waitFor(() => expect(screen.getByPlaceholderText("Ask Ark...")).toBeEnabled());
  const pickers = screen.getAllByRole("button", { name: /^Provider and model:/ });
  expect(pickers).toHaveLength(1);
  expect(screen.getByText("local")).toBeVisible();
  await user.click(pickers[0]);
  await user.click(screen.getByRole("option", { name: "Cloud model" }));
  expect(screen.getByText("cloud")).toBeVisible();

  await user.type(screen.getByPlaceholderText("Ask Ark..."), "route this request");
  await user.click(screen.getByRole("button", { name: "Send" }));

  await waitFor(() => expect(sendChatMessage).toHaveBeenCalledTimes(1));
  expect(sendChatMessage).toHaveBeenCalledWith(
    expect.objectContaining({
      conversationId: conversation.id,
      content: "route this request",
      providerId: cloud.id,
      model: "cloud-model",
    }),
  );
  const optimisticMessages = onMessagesChange.mock.calls.at(-1)?.[0];
  expect(optimisticMessages).toEqual([
    expect.objectContaining({ role: "user", providerId: cloud.id, modelId: "cloud-model" }),
    expect.objectContaining({ role: "assistant", providerId: cloud.id, modelId: "cloud-model" }),
  ]);
});
