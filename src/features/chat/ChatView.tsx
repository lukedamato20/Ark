import {
  Check,
  ChevronDown,
  Cloud,
  Download,
  FileJson,
  FileText,
  Loader2,
  Monitor,
  MoreVertical,
  Network,
  Send,
  Square,
  Trash2,
} from "lucide-react";
import * as React from "react";
import { cn } from "../../lib/cn";
import { downloadText, safeFilename } from "../../lib/download";
import { getErrorMessage } from "../../lib/arkErrors";
import { useArkClient } from "../../lib/useArkClient";
import type {
  Conversation,
  DestinationClass,
  Message,
  ModelInfo,
  ProviderConfig,
  ProviderHealth,
  SendChatResult,
} from "../../types/ark";
import { Button } from "../../ui/button";
import { Textarea } from "../../ui/textarea";
import { SetupBanner } from "../onboarding/SetupBanner";
import { ChatMessageList } from "./ChatMessageList";
import { MessageScrollContainer } from "./MessageScrollContainer";

/** COR-009: mirrors the authoritative limit enforced in `export::validate_conversation_export`. */
const MAX_IMPORT_FILE_BYTES = 50 * 1024 * 1024;

interface ActiveImport {
  id: string;
  fileName: string;
  completedMessages: number;
  totalMessages: number;
  cancelling: boolean;
}

interface ChatViewProps {
  conversation?: Conversation;
  messages: Message[];
  providers: ProviderConfig[];
  models: ModelInfo[];
  providerHealth: Record<string, ProviderHealth>;
  isLoading: boolean;
  onMessagesChange: (messages: Message[]) => void;
  onConversationDeleted: () => void;
  onConversationImported: (conversation: Conversation) => void;
  onConversationRenamed: (conversation: Conversation) => void;
  onModelsRefresh: (result: { health: ProviderHealth; models: ModelInfo[]; provider: ProviderConfig }) => void;
  onError: (message: string) => void;
  onInfo: (message: string) => void;
}

export function ChatView({
  conversation,
  messages,
  providers,
  models,
  providerHealth,
  isLoading,
  onMessagesChange,
  onConversationDeleted,
  onConversationImported,
  onConversationRenamed,
  onModelsRefresh,
  onError,
  onInfo,
}: ChatViewProps) {
  const client = useArkClient();
  const [draft, setDraft] = React.useState("");
  const [isSending, setIsSending] = React.useState(false);
  const [isRenaming, setIsRenaming] = React.useState(false);
  const [titleDraft, setTitleDraft] = React.useState("");
  const [providerId, setProviderId] = React.useState(providers[0]?.id ?? "");
  const [model, setModel] = React.useState("");
  const [editingMessageId, setEditingMessageId] = React.useState<string | null>(null);
  const [activeImport, setActiveImport] = React.useState<ActiveImport | null>(null);
  const fileInputRef = React.useRef<HTMLInputElement | null>(null);
  const previousConversationIdRef = React.useRef<string | undefined>(undefined);
  const autoRefreshedProviderIdsRef = React.useRef<Set<string>>(new Set());

  React.useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void client
      .onImportProgress((progress) => {
        setActiveImport((current) =>
          current?.id === progress.importId
            ? {
                ...current,
                completedMessages: progress.completedMessages,
                totalMessages: progress.totalMessages,
              }
            : current,
        );
      })
      .then((cleanup) => {
        if (disposed) cleanup();
        else unlisten = cleanup;
      })
      .catch((error) => onError(getErrorMessage(error)));
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [client, onError]);

  const provider = providers.find((item) => item.id === providerId) ?? providers[0];
  const health = providerHealth[provider?.id ?? ""] ?? null;
  const providerModels = models.filter((item) => item.providerId === (provider?.id ?? ""));
  const selectedModelAvailable = providerModels.some((item) => item.name === model && item.isAvailable);
  const activeAssistant = React.useMemo(
    () => [...messages].reverse().find((m) => m.role === "assistant" && m.status === "streaming"),
    [messages],
  );
  // Callbacks below intentionally depend on this boolean, not `activeAssistant` itself — they
  // only ever check its truthiness, so depending on the derived boolean instead of the object
  // avoids recreating the callback on every streaming-content update.
  const hasActiveAssistant = Boolean(activeAssistant);
  const canSend =
    Boolean(conversation && provider && model && selectedModelAvailable && draft.trim()) && !activeAssistant;

  React.useEffect(() => {
    const conversationChanged = previousConversationIdRef.current !== conversation?.id;
    previousConversationIdRef.current = conversation?.id;

    setProviderId((currentProviderId) => {
      const currentProviderStillExists = providers.some((item) => item.id === currentProviderId);
      if (conversationChanged) {
        return conversation?.providerId ?? providers[0]?.id ?? "";
      }
      if (!currentProviderStillExists) {
        return conversation?.providerId ?? providers[0]?.id ?? "";
      }
      return currentProviderId;
    });
  }, [conversation?.id, conversation?.providerId, providers]);

  React.useEffect(() => {
    if (!provider || providerModels.length > 0 || health || autoRefreshedProviderIdsRef.current.has(provider.id)) {
      return;
    }

    autoRefreshedProviderIdsRef.current.add(provider.id);
    void handleRefreshModels();
    // Deliberately narrow deps: must run only when provider/model-list/health identity
    // actually changes, not on every recreation of handleRefreshModels or health object.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [provider?.id, providerModels.length, health?.status]);

  React.useEffect(() => {
    setTitleDraft(conversation?.title ?? "");
    setIsRenaming(false);
    setEditingMessageId(null);
  }, [conversation?.id, conversation?.title]);

  React.useEffect(() => {
    const providerDefault = provider?.defaultModelId;
    const conversationModel = conversation?.modelId;
    const firstAvailable = providerModels.find((item) => item.isAvailable)?.name;

    const candidates = [conversationModel, providerDefault, firstAvailable].filter(Boolean) as string[];
    const availableCandidate = candidates.find((candidate) =>
      providerModels.some((item) => item.name === candidate && item.isAvailable),
    );
    setModel(availableCandidate ?? "");
  }, [conversation?.modelId, provider?.id, provider?.defaultModelId, providerModels]);

  async function handleRefreshModels() {
    try {
      const result = await client.refreshModels(provider?.id ?? "");
      onModelsRefresh(result);
      if (!model && result.provider.defaultModelId) {
        setModel(result.provider.defaultModelId);
      }
    } catch (error) {
      onError(getErrorMessage(error));
    }
  }

  const reconcileGenerationFailure = React.useCallback(
    async (error: unknown) => {
      const originalMessage = getErrorMessage(error);
      if (!conversation) {
        onError(originalMessage);
        return;
      }
      try {
        onMessagesChange(await client.getConversationMessages(conversation.id));
        onError(originalMessage);
      } catch (reconciliationError) {
        onError(`${originalMessage} Refresh also failed: ${getErrorMessage(reconciliationError)}`);
      }
    },
    [client, conversation, onError, onMessagesChange],
  );

  async function handleSend() {
    if (!conversation || !canSend) {
      return;
    }

    const content = draft.trim();
    setDraft("");
    setIsSending(true);

    try {
      const result: SendChatResult = await client.sendChatMessage({
        conversationId: conversation.id,
        content,
        providerId: provider!.id,
        model,
        temperature: provider?.defaultTemperature,
        maxTokens: provider?.defaultMaxTokens,
      });

      const now = new Date().toISOString();
      onMessagesChange([
        ...messages,
        {
          id: result.userMessageId,
          conversationId: conversation.id,
          parentMessageId: conversation.currentMessageId ?? null,
          revisionOfMessageId: null,
          pathIndex: messages.length + 1,
          role: "user",
          content,
          status: "complete",
          createdAt: now,
          updatedAt: now,
          providerId: provider!.id,
          modelId: model,
        },
        {
          id: result.assistantMessageId,
          conversationId: conversation.id,
          parentMessageId: result.userMessageId,
          revisionOfMessageId: null,
          pathIndex: messages.length + 2,
          role: "assistant",
          content: "",
          status: "streaming",
          createdAt: now,
          updatedAt: now,
          providerId: provider!.id,
          modelId: model,
        },
      ]);
      await client.startPendingStream(result.assistantMessageId);
    } catch (error) {
      setDraft(content);
      await reconcileGenerationFailure(error);
    } finally {
      setIsSending(false);
    }
  }

  async function handleRename() {
    if (!conversation) {
      return;
    }

    const nextTitle = titleDraft.trim();
    if (!nextTitle || nextTitle === conversation.title) {
      setIsRenaming(false);
      setTitleDraft(conversation.title);
      return;
    }

    try {
      const renamed = await client.renameConversation(conversation.id, nextTitle);
      onConversationRenamed(renamed);
      setIsRenaming(false);
    } catch (error) {
      onError(getErrorMessage(error));
    }
  }

  const handleStartEdit = React.useCallback(
    (message: Message) => {
      if (!conversation || !provider || !selectedModelAvailable || activeAssistant) {
        return;
      }
      setEditingMessageId(message.id);
    },
    // Intentionally depends on IDs, not the full conversation/provider objects, to avoid
    // recreating this callback on every unrelated field update (e.g. a title rename).
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [conversation?.id, provider?.id, selectedModelAvailable, hasActiveAssistant],
  );

  const handleSaveEdit = React.useCallback(
    async (message: Message, nextContent: string) => {
      if (!conversation || !provider || !selectedModelAvailable || activeAssistant) {
        return;
      }
      if (!nextContent.trim() || nextContent.trim() === message.content.trim()) {
        setEditingMessageId(null);
        return;
      }

      setEditingMessageId(null);
      try {
        const result = await client.editUserMessage({
          conversationId: conversation.id,
          messageId: message.id,
          content: nextContent.trim(),
          providerId: provider.id,
          model,
          temperature: provider.defaultTemperature,
          maxTokens: provider.defaultMaxTokens,
        });
        const now = new Date().toISOString();
        const index = messages.findIndex((item) => item.id === message.id);
        const prefix = index >= 0 ? messages.slice(0, index) : messages;
        onMessagesChange([
          ...prefix,
          {
            ...message,
            id: result.userMessageId,
            content: nextContent.trim(),
            revisionOfMessageId: message.id,
            status: "complete",
            createdAt: now,
            updatedAt: now,
            providerId: provider.id,
            modelId: model,
          },
          {
            id: result.assistantMessageId,
            conversationId: conversation.id,
            parentMessageId: result.userMessageId,
            revisionOfMessageId: null,
            pathIndex: message.pathIndex + 1,
            role: "assistant",
            content: "",
            status: "streaming",
            createdAt: now,
            updatedAt: now,
            providerId: provider.id,
            modelId: model,
          },
        ]);
        await client.startPendingStream(result.assistantMessageId);
      } catch (error) {
        await reconcileGenerationFailure(error);
      }
    },
    // Depends on conversation.id, not the whole object — see handleStartEdit above.
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [
      conversation?.id,
      provider,
      model,
      messages,
      selectedModelAvailable,
      hasActiveAssistant,
      onMessagesChange,
      onError,
      client,
      reconcileGenerationFailure,
    ],
  );

  const handleCancelEdit = React.useCallback(() => setEditingMessageId(null), []);

  const handleRegenerateMessage = React.useCallback(
    async (message: Message) => {
      if (!conversation || !provider || !selectedModelAvailable || activeAssistant) {
        return;
      }

      try {
        const result = await client.regenerateAssistantMessage({
          conversationId: conversation.id,
          messageId: message.id,
          providerId: provider.id,
          model,
          temperature: provider.defaultTemperature,
          maxTokens: provider.defaultMaxTokens,
        });
        const now = new Date().toISOString();
        const index = messages.findIndex((item) => item.id === message.id);
        const prefix = index >= 0 ? messages.slice(0, index) : messages;
        onMessagesChange([
          ...prefix,
          {
            ...message,
            id: result.assistantMessageId,
            content: "",
            revisionOfMessageId: message.id,
            status: "streaming",
            createdAt: now,
            updatedAt: now,
            providerId: provider.id,
            modelId: model,
          },
        ]);
        await client.startPendingStream(result.assistantMessageId);
      } catch (error) {
        await reconcileGenerationFailure(error);
      }
    },
    // eslint-disable-next-line react-hooks/exhaustive-deps -- see handleStartEdit above.
    [
      conversation?.id,
      provider,
      model,
      messages,
      selectedModelAvailable,
      hasActiveAssistant,
      onMessagesChange,
      onError,
      client,
      reconcileGenerationFailure,
    ],
  );

  const handleLoadAlternatives = React.useCallback(
    async (message: Message) => {
      if (!conversation || message.role !== "assistant") {
        return [];
      }
      return client.getAssistantAlternatives(conversation.id, message.id);
    },
    // Depends on conversation.id, not the whole object — see handleStartEdit above.
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [conversation?.id, client],
  );

  const handleSwitchBranch = React.useCallback(
    async (messageId: string) => {
      if (!conversation || activeAssistant) {
        return;
      }
      const nextMessages = await client.switchActiveBranch(conversation.id, messageId);
      onMessagesChange(nextMessages);
    },
    // eslint-disable-next-line react-hooks/exhaustive-deps -- see handleStartEdit above.
    [conversation?.id, hasActiveAssistant, onMessagesChange, client],
  );

  const handleKeepPartial = React.useCallback(
    async (message: Message) => {
      if (!conversation) {
        return;
      }
      try {
        const updated = await client.keepPartialMessage(message.id);
        onMessagesChange(messages.map((item) => (item.id === updated.id ? updated : item)));
      } catch (error) {
        onError(getErrorMessage(error));
      }
    },
    // eslint-disable-next-line react-hooks/exhaustive-deps -- see handleStartEdit above.
    [conversation?.id, messages, onMessagesChange, onError, client],
  );

  const handleDiscardInterrupted = React.useCallback(
    async (message: Message) => {
      if (!conversation) {
        return;
      }
      try {
        const nextMessages = await client.discardInterruptedMessage(conversation.id, message.id);
        onMessagesChange(nextMessages);
      } catch (error) {
        onError(getErrorMessage(error));
      }
    },
    // eslint-disable-next-line react-hooks/exhaustive-deps -- see handleStartEdit above.
    [conversation?.id, onMessagesChange, onError, client],
  );

  async function handleCancel() {
    if (!activeAssistant) {
      return;
    }
    try {
      await client.cancelStream(activeAssistant.id);
    } catch (error) {
      onError(getErrorMessage(error));
    }
  }

  async function handleDelete() {
    if (!conversation) {
      return;
    }

    const confirmed = window.confirm(`Delete "${conversation.title}"? This removes the local conversation history.`);
    if (!confirmed) {
      return;
    }

    try {
      await client.deleteConversation(conversation.id);
      onConversationDeleted();
    } catch (error) {
      onError(getErrorMessage(error));
    }
  }

  async function handleExport(format: "markdown" | "json") {
    if (!conversation) {
      return;
    }

    try {
      if (format === "markdown") {
        const markdown = await client.exportConversationMarkdown(conversation.id);
        downloadText(`${safeFilename(conversation.title)}.md`, markdown, "text/markdown;charset=utf-8");
      } else {
        const json = await client.exportConversationJson(conversation.id);
        downloadText(`${safeFilename(conversation.title)}.json`, json, "application/json;charset=utf-8");
      }
    } catch (error) {
      onError(getErrorMessage(error));
    }
  }

  async function handleImport(event: React.ChangeEvent<HTMLInputElement>) {
    const file = event.target.files?.[0];
    event.target.value = "";
    if (!file) {
      return;
    }
    if (activeImport) {
      onError("Wait for the current import to finish or cancel it before starting another.");
      return;
    }

    // COR-009: reject an oversized file before loading it into memory as a JS string.
    // This is a fast-fail UX nicety only — the Rust side enforces the authoritative limit
    // (see export::validate_conversation_export) regardless of what the frontend checks.
    if (file.size > MAX_IMPORT_FILE_BYTES) {
      onError(
        `"${file.name}" is ${(file.size / (1024 * 1024)).toFixed(1)} MB, which exceeds the ${MAX_IMPORT_FILE_BYTES / (1024 * 1024)} MB import limit.`,
      );
      return;
    }

    try {
      const json = await file.text();
      const preview = await client.previewConversationImport(json);
      const mappings = preview.providerMappings
        .map((mapping) => `${mapping.sourceProviderId ?? "unspecified"} → ${mapping.targetProviderId}`)
        .join(", ");
      const confirmed = window.confirm(
        `Import ${preview.conversationCount} conversation with ${preview.messageCount} messages?\n\n` +
          `Maximum branch depth: ${preview.maximumBranchDepth}\n` +
          `Transient states normalized: ${preview.normalizedMessageCount}\n` +
          `Conflicts: ${preview.conflicts.length}\n` +
          `Provider mapping: ${mappings}\n` +
          `Estimated storage: ${(preview.estimatedStorageBytes / 1024).toFixed(1)} KiB`,
      );
      if (!confirmed) return;

      const importId = crypto.randomUUID();
      setActiveImport({
        id: importId,
        fileName: file.name,
        completedMessages: 0,
        totalMessages: preview.messageCount,
        cancelling: false,
      });
      const result = await client.importConversationJson(importId, json);
      onConversationImported(result.conversation);
      if (result.normalizedMessageCount > 0) {
        const plural = result.normalizedMessageCount === 1 ? "message was" : "messages were";
        onInfo(
          `Import complete. ${result.normalizedMessageCount} ${plural} still mid-generation when exported and ` +
            `${result.normalizedMessageCount === 1 ? "has" : "have"} been marked interrupted — use Retry, Keep partial, or Discard on ${result.normalizedMessageCount === 1 ? "it" : "them"}.`,
        );
      }
    } catch (error) {
      if ((error as { code?: string })?.code === "import_cancelled") {
        onInfo("Import cancelled. No conversation data was written.");
      } else {
        onError(getErrorMessage(error));
      }
    } finally {
      setActiveImport(null);
    }
  }

  async function handleCancelImport() {
    if (!activeImport || activeImport.cancelling) return;
    setActiveImport({ ...activeImport, cancelling: true });
    try {
      await client.cancelImport(activeImport.id);
    } catch (error) {
      setActiveImport((current) => (current ? { ...current, cancelling: false } : current));
      onError(getErrorMessage(error));
    }
  }

  return (
    <section className="flex min-w-0 flex-1 flex-col">
      <header className="flex min-h-14 items-center justify-between gap-3 border-b border-border px-4">
        <div className="min-w-0">
          {isRenaming ? (
            <input
              value={titleDraft}
              onChange={(event) => setTitleDraft(event.target.value)}
              onBlur={handleRename}
              onKeyDown={(event) => {
                if (event.key === "Enter") {
                  event.preventDefault();
                  void handleRename();
                }
                if (event.key === "Escape") {
                  setTitleDraft(conversation?.title ?? "");
                  setIsRenaming(false);
                }
              }}
              autoFocus
              className="h-7 max-w-96 rounded-md border border-input bg-background px-2 text-sm font-semibold outline-none focus-visible:ring-2 focus-visible:ring-ring"
            />
          ) : (
            <button
              className="truncate rounded-sm text-left text-sm font-semibold outline-none focus-visible:ring-2 focus-visible:ring-ring"
              onClick={() => conversation && setIsRenaming(true)}
              disabled={!conversation}
            >
              {conversation?.title ?? "No conversation selected"}
            </button>
          )}
          <div className="mt-0.5 flex flex-wrap items-center gap-2">
            <ProviderStatusIcon provider={provider} />
            <span className="text-xs text-muted-foreground">{health?.message ?? "Runtime status unknown"}</span>
          </div>
        </div>

        <div className="flex items-center gap-2">
          <ProviderModelDropdown
            providers={providers}
            models={models}
            providerHealth={providerHealth}
            providerId={providerId}
            model={model}
            onSelect={(nextProviderId, nextModel) => {
              setProviderId(nextProviderId);
              setModel(nextModel);
            }}
          />
          <HeaderOverflowMenu
            conversationSelected={Boolean(conversation)}
            onExportMarkdown={() => void handleExport("markdown")}
            onExportJson={() => void handleExport("json")}
            onImportJson={() => fileInputRef.current?.click()}
            onDelete={() => void handleDelete()}
          />
          <input
            ref={fileInputRef}
            type="file"
            accept="application/json,.json"
            className="hidden"
            onChange={handleImport}
          />
        </div>
      </header>

      <div className="space-y-3 border-b border-border p-3">
        <SetupBanner
          health={health}
          provider={provider}
          models={providerModels}
          selectedModel={model}
          onRefresh={handleRefreshModels}
        />
        {activeImport && (
          <div
            className="flex items-center justify-between gap-3 rounded-md border border-border bg-card px-3 py-2"
            role="status"
            aria-live="polite"
          >
            <div className="min-w-0 text-sm">
              <div className="truncate font-medium">Importing {activeImport.fileName}</div>
              <div className="text-xs text-muted-foreground">
                {activeImport.completedMessages} of {activeImport.totalMessages} messages
              </div>
            </div>
            <Button variant="secondary" onClick={() => void handleCancelImport()} disabled={activeImport.cancelling}>
              {activeImport.cancelling ? <Loader2 className="h-4 w-4 animate-spin" /> : <Square className="h-4 w-4" />}
              {activeImport.cancelling ? "Cancelling" : "Cancel import"}
            </Button>
          </div>
        )}
      </div>

      {isLoading ? (
        <div className="flex min-h-0 flex-1 items-center justify-center text-sm text-muted-foreground">
          <Loader2 className="mr-2 h-4 w-4 animate-spin" />
          Loading conversation
        </div>
      ) : !conversation ? (
        <div className="flex min-h-0 flex-1 items-center justify-center">
          <EmptyChat />
        </div>
      ) : messages.length === 0 ? (
        <div className="flex min-h-0 flex-1 items-center justify-center">
          <EmptyChat />
        </div>
      ) : (
        <MessageScrollContainer resetKey={conversation.id}>
          <ChatMessageList
            messages={messages}
            canBranch={selectedModelAvailable && !activeAssistant}
            canSwitchBranch={!activeAssistant}
            editingMessageId={editingMessageId}
            onStartEdit={handleStartEdit}
            onSaveEdit={handleSaveEdit}
            onCancelEdit={handleCancelEdit}
            onRegenerate={handleRegenerateMessage}
            onLoadAlternatives={handleLoadAlternatives}
            onSwitchBranch={handleSwitchBranch}
            onKeepPartial={handleKeepPartial}
            onDiscardInterrupted={handleDiscardInterrupted}
            onError={onError}
          />
        </MessageScrollContainer>
      )}

      <footer className="border-t border-border p-4">
        <div className="mx-auto max-w-3xl">
          <div className="rounded-lg border border-border bg-card p-2">
            <Textarea
              value={draft}
              onChange={(event) => setDraft(event.target.value)}
              onKeyDown={(event) => {
                if (event.key === "Enter" && (event.metaKey || event.ctrlKey)) {
                  event.preventDefault();
                  void handleSend();
                }
              }}
              placeholder={
                selectedModelAvailable
                  ? "Ask Ark..."
                  : "Refresh models and select an installed local model before chatting."
              }
              disabled={!conversation || !selectedModelAvailable || Boolean(activeAssistant)}
              className="max-h-44 min-h-20 border-0 bg-transparent focus-visible:ring-0"
            />
            <div className="flex items-center justify-between px-1 pt-2">
              <div className="text-xs text-muted-foreground">Ctrl/Cmd + Enter to send</div>
              {activeAssistant ? (
                <Button variant="secondary" onClick={handleCancel}>
                  <Square className="h-4 w-4" />
                  Stop
                </Button>
              ) : (
                <Button variant="primary" onClick={handleSend} disabled={!canSend || isSending}>
                  {isSending ? <Loader2 className="h-4 w-4 animate-spin" /> : <Send className="h-4 w-4" />}
                  Send
                </Button>
              )}
            </div>
          </div>
        </div>
      </footer>
    </section>
  );
}

// SEC-001: classification comes from the backend (ProviderConfig.destinationClass, computed
// in Rust by security::classify_destination) rather than being re-derived here — the frontend
// must never be the source of truth for a privacy-relevant trust boundary.
const CONNECTION_METADATA: Record<
  DestinationClass,
  { icon: React.ComponentType<React.SVGProps<SVGSVGElement>>; label: string; tone: string; description: string }
> = {
  loopback: {
    icon: Monitor,
    label: "local",
    tone: "text-emerald-600 dark:text-emerald-300",
    description:
      "Running locally on this device. User prompts, conversation history, and the configured system prompt do not leave this computer.",
  },
  private_lan: {
    icon: Network,
    label: "network",
    tone: "text-sky-600 dark:text-sky-300",
    description:
      "Connecting to a server on your local network. User prompts, conversation history, and the configured system prompt leave this device but stay within your network.",
  },
  public: {
    icon: Cloud,
    label: "cloud",
    tone: "text-amber-600 dark:text-amber-300",
    description:
      "Connecting to a remote server outside your network. User prompts, conversation history, and the configured system prompt are sent to this destination.",
  },
};

function ProviderStatusIcon({ provider }: { provider?: ProviderConfig }) {
  const destinationClass = provider?.destinationClass ?? "loopback";
  const { icon: Icon, label, tone, description } = CONNECTION_METADATA[destinationClass];
  const tooltipId = `provider-status-${provider?.id ?? "none"}`;

  return (
    <span className="group relative inline-flex items-center gap-1">
      <span className={cn("inline-flex items-center gap-1 text-xs font-medium", tone)} aria-describedby={tooltipId}>
        <Icon className="h-3 w-3" aria-hidden="true" />
        {label}
      </span>
      <span
        id={tooltipId}
        role="tooltip"
        className={cn(
          "pointer-events-none absolute bottom-full left-0 z-50 mb-1.5 w-56 rounded-md border border-border",
          "bg-popover px-2.5 py-1.5 text-xs text-popover-foreground shadow-md",
          "opacity-0 transition-opacity duration-150 group-hover:opacity-100",
        )}
      >
        {description}
      </span>
    </span>
  );
}

/**
 * UX-002: a single interactive control for switching provider/model in place, without
 * navigating to Settings. Each entry's icon/tooltip is derived from the backend-computed
 * destinationClass (see ProviderStatusIcon) — never re-derived on the frontend.
 */
function ProviderModelDropdown({
  providers,
  models,
  providerHealth,
  providerId,
  model,
  onSelect,
}: {
  providers: ProviderConfig[];
  models: ModelInfo[];
  providerHealth: Record<string, ProviderHealth>;
  providerId: string;
  model: string;
  onSelect: (providerId: string, model: string) => void;
}) {
  const [open, setOpen] = React.useState(false);
  const containerRef = React.useRef<HTMLDivElement | null>(null);
  const provider = providers.find((item) => item.id === providerId);
  const destinationClass = provider?.destinationClass ?? "loopback";
  const { icon: Icon, label: classLabel, tone } = CONNECTION_METADATA[destinationClass];

  React.useEffect(() => {
    if (!open) {
      return;
    }

    function handlePointerDown(event: MouseEvent) {
      if (containerRef.current && !containerRef.current.contains(event.target as Node)) {
        setOpen(false);
      }
    }
    function handleKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape") {
        setOpen(false);
      }
    }

    document.addEventListener("mousedown", handlePointerDown);
    document.addEventListener("keydown", handleKeyDown);
    return () => {
      document.removeEventListener("mousedown", handlePointerDown);
      document.removeEventListener("keydown", handleKeyDown);
    };
  }, [open]);

  const triggerLabel = provider
    ? `Provider and model: ${provider.name}, ${model || "no model selected"}, ${classLabel} connection`
    : "Select a provider and model";

  return (
    <div className="relative" ref={containerRef}>
      <button
        type="button"
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-label={triggerLabel}
        onClick={() => setOpen((value) => !value)}
        className="flex h-9 items-center gap-1.5 rounded-md border border-input bg-background px-2.5 text-xs font-medium outline-none hover:bg-accent focus-visible:ring-2 focus-visible:ring-ring"
      >
        <Icon className={cn("h-3.5 w-3.5 shrink-0", tone)} aria-hidden="true" />
        <span className="max-w-[140px] truncate">{provider?.name ?? "No provider"}</span>
        <span className="max-w-[120px] truncate text-muted-foreground">{model || "No model"}</span>
        <ChevronDown className="h-3.5 w-3.5 shrink-0 text-muted-foreground" aria-hidden="true" />
      </button>

      {open && (
        <div
          role="listbox"
          aria-label="Providers and models"
          className="absolute left-0 top-full z-50 mt-1.5 max-h-96 w-72 overflow-y-auto rounded-lg border border-border bg-popover p-1.5 shadow-lg"
        >
          {providers.map((item) => {
            const itemHealth = providerHealth[item.id];
            const itemModels = models.filter((entry) => entry.providerId === item.id);
            const itemMeta = CONNECTION_METADATA[item.destinationClass];
            const ItemIcon = itemMeta.icon;
            const tooltipId = `provider-option-${item.id}`;

            return (
              <div key={item.id} className="mb-1 last:mb-0">
                <div className="group relative flex items-center gap-1.5 px-2 py-1">
                  <ItemIcon className={cn("h-3 w-3 shrink-0", itemMeta.tone)} aria-hidden="true" />
                  <span className="text-xs font-semibold text-foreground">{item.name}</span>
                  <span className={cn("ml-auto text-[10px] font-medium", itemMeta.tone)} aria-describedby={tooltipId}>
                    {itemMeta.label}
                  </span>
                  <span
                    id={tooltipId}
                    role="tooltip"
                    className={cn(
                      "pointer-events-none absolute right-0 top-full z-50 mt-1 w-56 rounded-md border border-border",
                      "bg-popover px-2.5 py-1.5 text-xs text-popover-foreground shadow-md",
                      "opacity-0 transition-opacity duration-150 group-hover:opacity-100",
                    )}
                  >
                    {itemMeta.description}
                  </span>
                </div>
                {itemModels.length === 0 ? (
                  <div className="px-2 py-1 text-xs text-muted-foreground">
                    {itemHealth && !itemHealth.isReachable ? "Unavailable — check Settings" : "No models found"}
                  </div>
                ) : (
                  itemModels.map((modelItem) => {
                    const selected = item.id === providerId && modelItem.name === model;
                    return (
                      <button
                        key={modelItem.id}
                        type="button"
                        role="option"
                        aria-selected={selected}
                        disabled={!modelItem.isAvailable}
                        onClick={() => {
                          onSelect(item.id, modelItem.name);
                          setOpen(false);
                        }}
                        className={cn(
                          "flex w-full items-center justify-between gap-2 rounded-md px-2 py-1.5 text-left text-xs outline-none transition-colors duration-150",
                          "hover:bg-accent hover:text-accent-foreground focus-visible:ring-2 focus-visible:ring-ring",
                          "disabled:cursor-not-allowed disabled:opacity-40",
                          selected && "bg-primary/10 font-medium text-foreground",
                        )}
                      >
                        <span className="truncate">{modelItem.displayName ?? modelItem.name}</span>
                        {selected && <Check className="h-3.5 w-3.5 shrink-0" aria-hidden="true" />}
                      </button>
                    );
                  })
                )}
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}

/**
 * UX-002: moves export/import/delete out of the always-visible header (freeing up compact-
 * width space) into an accessible overflow menu, with the destructive action (delete)
 * visually separated by a divider and styled distinctly from the safe actions above it.
 */
function HeaderOverflowMenu({
  conversationSelected,
  onExportMarkdown,
  onExportJson,
  onImportJson,
  onDelete,
}: {
  conversationSelected: boolean;
  onExportMarkdown: () => void;
  onExportJson: () => void;
  onImportJson: () => void;
  onDelete: () => void;
}) {
  const [open, setOpen] = React.useState(false);
  const containerRef = React.useRef<HTMLDivElement | null>(null);
  const triggerRef = React.useRef<HTMLButtonElement | null>(null);

  React.useEffect(() => {
    if (!open) {
      return;
    }

    function handlePointerDown(event: MouseEvent) {
      if (containerRef.current && !containerRef.current.contains(event.target as Node)) {
        setOpen(false);
      }
    }
    function handleKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape") {
        setOpen(false);
        // Return focus to the trigger, matching standard menu/dialog keyboard patterns —
        // closing a menu must not strand focus on a now-hidden element.
        triggerRef.current?.focus();
      }
    }

    document.addEventListener("mousedown", handlePointerDown);
    document.addEventListener("keydown", handleKeyDown);
    return () => {
      document.removeEventListener("mousedown", handlePointerDown);
      document.removeEventListener("keydown", handleKeyDown);
    };
  }, [open]);

  function runAndClose(action: () => void) {
    setOpen(false);
    action();
  }

  return (
    <div className="relative" ref={containerRef}>
      <button
        ref={triggerRef}
        type="button"
        aria-haspopup="menu"
        aria-expanded={open}
        aria-label="More conversation actions"
        onClick={() => setOpen((value) => !value)}
        className="flex h-9 w-9 items-center justify-center rounded-md border border-input bg-background text-muted-foreground outline-none hover:bg-accent hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring"
      >
        <MoreVertical className="h-4 w-4" />
      </button>

      {open && (
        <div
          role="menu"
          aria-label="Conversation actions"
          className="absolute right-0 top-full z-50 mt-1.5 w-52 rounded-lg border border-border bg-popover p-1.5 shadow-lg"
        >
          <button
            type="button"
            role="menuitem"
            disabled={!conversationSelected}
            onClick={() => runAndClose(onExportMarkdown)}
            className="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm outline-none transition-colors duration-150 hover:bg-accent hover:text-accent-foreground focus-visible:ring-2 focus-visible:ring-ring disabled:cursor-not-allowed disabled:opacity-40"
          >
            <FileText className="h-4 w-4" aria-hidden="true" />
            Export as Markdown
          </button>
          <button
            type="button"
            role="menuitem"
            disabled={!conversationSelected}
            onClick={() => runAndClose(onExportJson)}
            className="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm outline-none transition-colors duration-150 hover:bg-accent hover:text-accent-foreground focus-visible:ring-2 focus-visible:ring-ring disabled:cursor-not-allowed disabled:opacity-40"
          >
            <FileJson className="h-4 w-4" aria-hidden="true" />
            Export as JSON
          </button>
          <button
            type="button"
            role="menuitem"
            onClick={() => runAndClose(onImportJson)}
            className="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm outline-none transition-colors duration-150 hover:bg-accent hover:text-accent-foreground focus-visible:ring-2 focus-visible:ring-ring"
          >
            <Download className="h-4 w-4" aria-hidden="true" />
            Import JSON
          </button>
          <div role="separator" className="my-1.5 border-t border-border" />
          <button
            type="button"
            role="menuitem"
            disabled={!conversationSelected}
            onClick={() => runAndClose(onDelete)}
            className="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm text-destructive outline-none transition-colors duration-150 hover:bg-destructive/10 focus-visible:ring-2 focus-visible:ring-ring disabled:cursor-not-allowed disabled:opacity-40"
          >
            <Trash2 className="h-4 w-4" aria-hidden="true" />
            Delete conversation
          </button>
        </div>
      )}
    </div>
  );
}

function EmptyChat() {
  return (
    <div className="flex h-full items-center justify-center">
      <div className="max-w-md text-center">
        <div className="text-lg font-semibold">Your local AI workspace is ready.</div>
        <p className="mt-2 text-sm text-muted-foreground">
          Start a conversation once your local runtime is reachable and a model is selected.
        </p>
      </div>
    </div>
  );
}
