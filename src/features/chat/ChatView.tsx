import {
  Check,
  ChevronDown,
  Download,
  FileJson,
  FileText,
  Loader2,
  MoreVertical,
  Paperclip,
  Pencil,
  Send,
  SlidersHorizontal,
  Square,
  StickyNote,
  Trash2,
  X,
} from "lucide-react";
import * as React from "react";
import { cn } from "../../lib/cn";
import { CONNECTION_METADATA } from "../../lib/destinationClass";
import { downloadText, safeFilename } from "../../lib/download";
import { getErrorMessage } from "../../lib/arkErrors";
import { formatRelativeTime, isProviderHealthStale } from "../../lib/relativeTime";
import { useArkClient } from "../../lib/useArkClient";
import type {
  Attachment,
  Conversation,
  ConversationNote,
  Message,
  ModelInfo,
  Persona,
  Project,
  ProviderConfig,
  ProviderHealth,
  SendChatResult,
  SideEffectPreview,
} from "../../types/ark";
import { Button } from "../../ui/button";
import { Input } from "../../ui/input";
import { Textarea } from "../../ui/textarea";
import { SetupBanner } from "../onboarding/SetupBanner";
import { ChatMessageList } from "./ChatMessageList";
import { MessageScrollContainer } from "./MessageScrollContainer";

/** COR-009: mirrors the authoritative limit enforced in `export::validate_conversation_export`. */
const MAX_IMPORT_FILE_BYTES = 50 * 1024 * 1024;

/** CMP-001: mirrors `validation::MAX_ATTACHMENT_BYTES` — a fast client-side rejection so a huge
 * file doesn't get read into memory and uploaded only to bounce off the server-side limit. */
const MAX_ATTACHMENT_BYTES = 2 * 1024 * 1024;
/** UX only — the server-side content sniff (NUL-byte rejection) is the real, authoritative
 * boundary; a generous plain-text-ish extension list here just steers the file picker. */
const ATTACHMENT_ACCEPT =
  ".txt,.md,.markdown,.csv,.tsv,.json,.log,.yaml,.yml,.toml,.ini,.xml,.html,.css,.ts,.tsx,.js,.jsx,.py,.rs,.go,.java,.c,.cpp,.h,.sh";

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
  /** FTR-003: for the project picker in the conversation settings panel — expected to stay
   * small, so passed as a plain list rather than paginated like conversations. */
  projects: Project[];
  /** FTR-003: for the persona picker in the conversation settings panel — mirrors `projects`. */
  personas: Persona[];
  isLoading: boolean;
  /** UX-007: bumped by an explicit "New Chat"/conversation-select action (see `ShellState`'s own
   * doc comment) — focuses the composer, never on a passive background update. */
  focusComposerSignal: number;
  onMessagesChange: (messages: Message[]) => void;
  onConversationDeleted: () => void;
  onConversationImported: (conversation: Conversation) => void;
  onConversationRenamed: (conversation: Conversation) => void;
  onConversationProjectChange: (id: string, projectId: string | null) => Promise<void>;
  onConversationPersonaChange: (id: string, personaId: string | null) => Promise<void>;
  /** FTR-009: centralized in the controller (sequenced/deduplicated per provider) rather than
   * this component fetching and applying a result itself — see `useArkController.ts`'s
   * `refreshProviderModels` doc comment. */
  onRefreshProviderModels: (providerId: string) => Promise<void>;
  onError: (message: string) => void;
  onInfo: (message: string) => void;
}

export function ChatView({
  conversation,
  messages,
  providers,
  models,
  providerHealth,
  projects,
  personas,
  isLoading,
  focusComposerSignal,
  onMessagesChange,
  onConversationDeleted,
  onConversationImported,
  onConversationRenamed,
  onConversationProjectChange,
  onConversationPersonaChange,
  onRefreshProviderModels,
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
  const [stagedAttachments, setStagedAttachments] = React.useState<Attachment[]>([]);
  const [sentAttachmentsByMessageId, setSentAttachmentsByMessageId] = React.useState<Record<string, Attachment[]>>({});
  const [attaching, setAttaching] = React.useState(false);
  const fileInputRef = React.useRef<HTMLInputElement | null>(null);
  const attachmentInputRef = React.useRef<HTMLInputElement | null>(null);
  const composerRef = React.useRef<HTMLTextAreaElement | null>(null);
  const previousConversationIdRef = React.useRef<string | undefined>(undefined);
  const autoRefreshedProviderIdsRef = React.useRef<Set<string>>(new Set());

  // UX-007: focuses the composer only on an explicit "New Chat"/select action (see
  // `focusComposerSignal`'s doc comment on `ShellState`) — a plain `useEffect` on `conversation`
  // would also fire on the initial bootstrap load and on background reconciliation, stealing
  // focus from deliberate reading/search. `signal === 0` is the initial/no-action value, so the
  // very first render never triggers a focus steal either.
  React.useEffect(() => {
    if (focusComposerSignal > 0) composerRef.current?.focus();
  }, [focusComposerSignal]);

  // CMP-001: loads every attachment for the conversation on switch — staged ones (`messageId`
  // still `null`) restore the compose-in-progress state (see `handleAttachFiles`'s doc comment
  // for why that matters), while already-sent ones are indexed by message id so
  // `ChatMessageList` can show which messages carried a file.
  React.useEffect(() => {
    if (!conversation) {
      setStagedAttachments([]);
      setSentAttachmentsByMessageId({});
      return;
    }
    let cancelled = false;
    void client
      .listConversationAttachments(conversation.id)
      .then((attachments) => {
        if (cancelled) return;
        setStagedAttachments(attachments.filter((a) => !a.messageId));
        const byMessageId: Record<string, Attachment[]> = {};
        for (const attachment of attachments) {
          if (!attachment.messageId) continue;
          (byMessageId[attachment.messageId] ??= []).push(attachment);
        }
        setSentAttachmentsByMessageId(byMessageId);
      })
      .catch((error) => onError(getErrorMessage(error)));
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [conversation?.id]);

  async function handleAttachFiles(files: FileList | File[]) {
    if (!conversation) return;
    setAttaching(true);
    try {
      for (const file of Array.from(files)) {
        if (file.size > MAX_ATTACHMENT_BYTES) {
          onError(
            `"${file.name}" is ${(file.size / (1024 * 1024)).toFixed(1)} MB, which exceeds the ${MAX_ATTACHMENT_BYTES / (1024 * 1024)} MB attachment limit.`,
          );
          continue;
        }
        const content = await file.text();
        try {
          const attachment = await client.attachTextFile(conversation.id, file.name, content);
          setStagedAttachments((current) => [...current, attachment]);
        } catch (error) {
          onError(getErrorMessage(error));
        }
      }
    } finally {
      setAttaching(false);
    }
  }

  async function handleRemoveStagedAttachment(id: string) {
    try {
      await client.deleteAttachment(id);
      setStagedAttachments((current) => current.filter((a) => a.id !== id));
    } catch (error) {
      onError(getErrorMessage(error));
    }
  }

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
    const conversationModel = conversation?.modelId;
    const conversationModelKnown = Boolean(
      conversationModel && providerModels.some((item) => item.name === conversationModel),
    );
    if (conversationModel && conversationModelKnown) {
      // FTR-009 AC3: keep the conversation's own model selected even if it's no longer
      // available, rather than silently substituting something else — `selectedModelAvailable`
      // already gates sending, and the notice rendered below explains why and offers
      // alternatives. Only fall through to picking a substitute when Ark genuinely has no
      // record of this model at all (e.g. the provider hasn't been refreshed yet).
      setModel(conversationModel);
      return;
    }

    const providerDefault = provider?.defaultModelId;
    const firstAvailable = providerModels.find((item) => item.isAvailable)?.name;
    const candidates = [providerDefault, firstAvailable].filter(Boolean) as string[];
    const availableCandidate = candidates.find((candidate) =>
      providerModels.some((item) => item.name === candidate && item.isAvailable),
    );
    setModel(availableCandidate ?? "");
  }, [conversation?.modelId, provider?.id, provider?.defaultModelId, providerModels]);

  // FTR-009 AC3: distinct from "no model selected yet" — this is specifically "the model this
  // conversation was using is known to Ark but no longer available," which needs its own
  // explanation and alternatives rather than the generic setup banner's message.
  const removedSelectedModel =
    conversation?.modelId && model === conversation.modelId
      ? providerModels.find((item) => item.name === conversation.modelId && !item.isAvailable)
      : undefined;

  async function handleRefreshModels() {
    // FTR-009: refreshProviderModels owns error reporting (via the global setError) for every
    // caller, so no local try/catch here — the model-selection effect above already reacts to
    // the store update this produces (its deps include provider?.defaultModelId/providerModels),
    // so no manual setModel follow-up is needed either.
    await onRefreshProviderModels(provider?.id ?? "");
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
    const attachmentIds = stagedAttachments.map((a) => a.id);
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
        attachmentIds,
      });
      if (stagedAttachments.length > 0) {
        setSentAttachmentsByMessageId((current) => ({
          ...current,
          [result.userMessageId]: stagedAttachments.map((a) => ({ ...a, messageId: result.userMessageId })),
        }));
      }
      setStagedAttachments([]);

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

  const handleLoadMessage = React.useCallback((messageId: string) => client.getMessage(messageId), [client]);

  const handleRenameBranch = React.useCallback(
    async (messageId: string, name: string | null) => {
      await client.setBranchName(messageId, name);
    },
    [client],
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
      // UX-004: every completed import gets a terminal summary, not only ones with something to
      // flag — previously the toast only fired when normalizedMessageCount > 0, so a routine
      // successful import ended in silence with just the progress indicator disappearing.
      const messagePlural = preview.messageCount === 1 ? "message" : "messages";
      const summary = `Import complete. "${result.conversation.title}" — ${preview.messageCount} ${messagePlural} imported.`;
      if (result.normalizedMessageCount > 0) {
        const plural = result.normalizedMessageCount === 1 ? "message was" : "messages were";
        onInfo(
          `${summary} ${result.normalizedMessageCount} ${plural} still mid-generation when exported and ` +
            `${result.normalizedMessageCount === 1 ? "has" : "have"} been marked interrupted — use Retry, Keep partial, or Discard on ${result.normalizedMessageCount === 1 ? "it" : "them"}.`,
        );
      } else {
        onInfo(summary);
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
    <main aria-label="Chat" className="flex min-w-0 flex-1 flex-col">
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
            {health?.checkedAt && (
              <span
                className={cn(
                  "text-xs",
                  isProviderHealthStale(health.checkedAt)
                    ? "text-amber-600 dark:text-amber-400"
                    : "text-muted-foreground",
                )}
                title={new Date(health.checkedAt).toLocaleString()}
              >
                · checked {formatRelativeTime(health.checkedAt)}
                {isProviderHealthStale(health.checkedAt) ? " (stale)" : ""}
              </span>
            )}
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
          <ConversationSettingsButton
            conversation={conversation}
            provider={provider}
            projects={projects}
            onProjectChange={onConversationProjectChange}
            personas={personas}
            onPersonaChange={onConversationPersonaChange}
            onSettingsSaved={onConversationRenamed}
            onError={onError}
          />
          <ConversationNotesButton conversation={conversation} onError={onError} />
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
        {removedSelectedModel ? (
          <UnavailableModelNotice
            modelName={removedSelectedModel.name}
            alternatives={providerModels.filter((item) => item.isAvailable)}
            onSelectAlternative={setModel}
            onRefresh={handleRefreshModels}
          />
        ) : (
          <SetupBanner
            health={health}
            provider={provider}
            models={providerModels}
            selectedModel={model}
            onRefresh={handleRefreshModels}
          />
        )}
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
            providers={providers}
            attachmentsByMessageId={sentAttachmentsByMessageId}
            canBranch={selectedModelAvailable && !activeAssistant}
            canSwitchBranch={!activeAssistant}
            editingMessageId={editingMessageId}
            onStartEdit={handleStartEdit}
            onSaveEdit={handleSaveEdit}
            onCancelEdit={handleCancelEdit}
            onRegenerate={handleRegenerateMessage}
            onLoadAlternatives={handleLoadAlternatives}
            onSwitchBranch={handleSwitchBranch}
            onLoadMessage={handleLoadMessage}
            onRenameBranch={handleRenameBranch}
            onKeepPartial={handleKeepPartial}
            onDiscardInterrupted={handleDiscardInterrupted}
            onError={onError}
          />
        </MessageScrollContainer>
      )}

      <footer className="border-t border-border p-4">
        <div className="mx-auto max-w-3xl">
          <div
            className="rounded-lg border border-border bg-card p-2"
            onDragOver={(event) => {
              if (event.dataTransfer.types.includes("Files")) event.preventDefault();
            }}
            onDrop={(event) => {
              if (event.dataTransfer.files.length === 0) return;
              event.preventDefault();
              void handleAttachFiles(event.dataTransfer.files);
            }}
          >
            {stagedAttachments.length > 0 && (
              <div className="mb-2 flex flex-wrap gap-1.5 px-1">
                {stagedAttachments.map((attachment) => (
                  <span
                    key={attachment.id}
                    className="inline-flex items-center gap-1.5 rounded-md border border-border bg-muted/50 py-1 pl-2 pr-1 text-xs"
                  >
                    <FileText className="h-3.5 w-3.5 text-muted-foreground" aria-hidden="true" />
                    <span className="max-w-40 truncate" title={attachment.fileName}>
                      {attachment.fileName}
                    </span>
                    <span className="text-muted-foreground">{(attachment.byteSize / 1024).toFixed(1)} KB</span>
                    <button
                      type="button"
                      aria-label={`Remove attachment ${attachment.fileName}`}
                      onClick={() => void handleRemoveStagedAttachment(attachment.id)}
                      className="rounded-sm p-0.5 outline-none hover:bg-muted focus-visible:ring-2 focus-visible:ring-ring"
                    >
                      <X className="h-3 w-3" />
                    </button>
                  </span>
                ))}
              </div>
            )}
            <Textarea
              ref={composerRef}
              value={draft}
              onChange={(event) => setDraft(event.target.value)}
              onKeyDown={(event) => {
                if (event.key === "Enter" && (event.metaKey || event.ctrlKey)) {
                  event.preventDefault();
                  void handleSend();
                }
              }}
              onPaste={(event) => {
                if (event.clipboardData.files.length === 0) return;
                event.preventDefault();
                void handleAttachFiles(event.clipboardData.files);
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
              <div className="flex items-center gap-2">
                <Button
                  variant="ghost"
                  size="sm"
                  onClick={() => attachmentInputRef.current?.click()}
                  disabled={!conversation || attaching}
                  aria-label="Attach a text file"
                >
                  {attaching ? <Loader2 className="h-4 w-4 animate-spin" /> : <Paperclip className="h-4 w-4" />}
                </Button>
                <input
                  ref={attachmentInputRef}
                  type="file"
                  multiple
                  accept={ATTACHMENT_ACCEPT}
                  className="hidden"
                  onChange={(event) => {
                    if (event.target.files) void handleAttachFiles(event.target.files);
                    event.target.value = "";
                  }}
                />
                <div className="text-xs text-muted-foreground">Ctrl/Cmd + Enter to send</div>
              </div>
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
    </main>
  );
}

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
          "opacity-0 transition-opacity duration-fast group-hover:opacity-100",
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
                      "opacity-0 transition-opacity duration-fast group-hover:opacity-100",
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
                          "flex w-full items-center justify-between gap-2 rounded-md px-2 py-1.5 text-left text-xs outline-none transition-colors duration-fast",
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

const MIN_TEMPERATURE = 0;
const MAX_TEMPERATURE = 2;
const MIN_MAX_TOKENS = 1;
const MAX_MAX_TOKENS = 1_000_000;

/**
 * FTR-004: the per-conversation system-prompt/temperature/max-tokens override panel — the
 * "effective settings and their source are visible before send" acceptance criterion. Every
 * field is independently clearable (an empty draft means "no override, inherit the provider's
 * current default"), unlike Settings' provider-level `NumberField`, which treats empty as
 * invalid — a conversation override genuinely has no required value, so this uses its own
 * light validation rather than forcing that component's different semantics.
 */
function ConversationSettingsButton({
  conversation,
  provider,
  projects,
  onProjectChange,
  personas,
  onPersonaChange,
  onSettingsSaved,
  onError,
}: {
  conversation?: Conversation;
  provider?: ProviderConfig;
  projects: Project[];
  onProjectChange: (id: string, projectId: string | null) => Promise<void>;
  personas: Persona[];
  onPersonaChange: (id: string, personaId: string | null) => Promise<void>;
  onSettingsSaved: (conversation: Conversation) => void;
  onError: (message: string) => void;
}) {
  const client = useArkClient();
  const [open, setOpen] = React.useState(false);
  const [systemPromptDraft, setSystemPromptDraft] = React.useState("");
  const [temperatureDraft, setTemperatureDraft] = React.useState("");
  const [maxTokensDraft, setMaxTokensDraft] = React.useState("");
  const [saving, setSaving] = React.useState(false);
  const [projectChanging, setProjectChanging] = React.useState(false);
  const [personaChanging, setPersonaChanging] = React.useState(false);
  const containerRef = React.useRef<HTMLDivElement | null>(null);

  React.useEffect(() => {
    if (!open) return;
    setSystemPromptDraft(conversation?.systemPrompt ?? "");
    setTemperatureDraft(conversation?.temperature != null ? String(conversation.temperature) : "");
    setMaxTokensDraft(conversation?.maxTokens != null ? String(conversation.maxTokens) : "");
  }, [open, conversation]);

  React.useEffect(() => {
    if (!open) return;

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

  const temperatureTrimmed = temperatureDraft.trim();
  const maxTokensTrimmed = maxTokensDraft.trim();
  const temperatureNumber = temperatureTrimmed === "" ? null : Number(temperatureTrimmed);
  const maxTokensNumber = maxTokensTrimmed === "" ? null : Number(maxTokensTrimmed);
  const temperatureValid =
    temperatureTrimmed === "" ||
    (temperatureNumber !== null &&
      Number.isFinite(temperatureNumber) &&
      temperatureNumber >= MIN_TEMPERATURE &&
      temperatureNumber <= MAX_TEMPERATURE);
  const maxTokensValid =
    maxTokensTrimmed === "" ||
    (maxTokensNumber !== null &&
      Number.isInteger(maxTokensNumber) &&
      maxTokensNumber >= MIN_MAX_TOKENS &&
      maxTokensNumber <= MAX_MAX_TOKENS);

  async function save() {
    if (!conversation || !temperatureValid || !maxTokensValid) return;
    setSaving(true);
    try {
      const updated = await client.updateConversationSettings({
        id: conversation.id,
        systemPrompt: systemPromptDraft.trim() || null,
        temperature: temperatureNumber,
        maxTokens: maxTokensNumber,
      });
      onSettingsSaved(updated);
      setOpen(false);
    } catch (error) {
      onError(getErrorMessage(error));
    } finally {
      setSaving(false);
    }
  }

  const hasOverride = Boolean(
    conversation?.systemPrompt || conversation?.temperature != null || conversation?.maxTokens != null,
  );

  return (
    <div className="relative" ref={containerRef}>
      <button
        type="button"
        aria-haspopup="dialog"
        aria-expanded={open}
        aria-label={hasOverride ? "Conversation settings (custom overrides active)" : "Conversation settings"}
        onClick={() => setOpen((value) => !value)}
        disabled={!conversation}
        className="relative flex h-9 w-9 items-center justify-center rounded-md border border-input bg-background outline-none hover:bg-accent focus-visible:ring-2 focus-visible:ring-ring disabled:cursor-not-allowed disabled:opacity-40"
      >
        <SlidersHorizontal className="h-4 w-4" aria-hidden="true" />
        {hasOverride && (
          <span aria-hidden="true" className="absolute right-1 top-1 h-1.5 w-1.5 rounded-full bg-primary" />
        )}
      </button>

      {open && conversation && (
        <div
          role="dialog"
          aria-label="Conversation settings"
          className="absolute right-0 top-full z-50 mt-1.5 w-80 rounded-lg border border-border bg-popover p-3 shadow-lg"
        >
          <div className="grid gap-3">
            <label className="grid gap-1.5 text-xs font-medium">
              Project
              <select
                value={conversation.projectId ?? ""}
                disabled={projectChanging}
                onChange={async (event) => {
                  setProjectChanging(true);
                  try {
                    await onProjectChange(conversation.id, event.target.value || null);
                  } finally {
                    setProjectChanging(false);
                  }
                }}
                className="h-8 rounded-md border border-input bg-background px-2 text-xs outline-none focus-visible:ring-2 focus-visible:ring-ring"
              >
                <option value="">No project</option>
                {projects
                  .filter((project) => !project.archivedAt || project.id === conversation.projectId)
                  .map((project) => (
                    <option key={project.id} value={project.id}>
                      {project.name}
                    </option>
                  ))}
              </select>
            </label>
            <label className="grid gap-1.5 text-xs font-medium">
              Persona
              <select
                value={conversation.personaId ?? ""}
                disabled={personaChanging}
                onChange={async (event) => {
                  setPersonaChanging(true);
                  try {
                    await onPersonaChange(conversation.id, event.target.value || null);
                  } finally {
                    setPersonaChanging(false);
                  }
                }}
                className="h-8 rounded-md border border-input bg-background px-2 text-xs outline-none focus-visible:ring-2 focus-visible:ring-ring"
              >
                <option value="">No persona</option>
                {personas
                  .filter((persona) => !persona.archivedAt || persona.id === conversation.personaId)
                  .map((persona) => (
                    <option key={persona.id} value={persona.id}>
                      {persona.name}
                    </option>
                  ))}
              </select>
            </label>
            <label className="grid gap-1.5 text-xs font-medium">
              System prompt
              <Textarea
                value={systemPromptDraft}
                onChange={(event) => setSystemPromptDraft(event.target.value)}
                rows={3}
                placeholder="No override — sent without a system prompt"
                className="text-xs"
              />
            </label>
            <label className="grid gap-1.5 text-xs font-medium">
              Temperature
              <Input
                value={temperatureDraft}
                onChange={(event) => setTemperatureDraft(event.target.value)}
                inputMode="decimal"
                placeholder={
                  provider?.defaultTemperature != null
                    ? `Provider default (${provider.defaultTemperature})`
                    : "Provider default (none)"
                }
                aria-invalid={!temperatureValid}
                className={cn(!temperatureValid && "border-destructive focus-visible:ring-destructive")}
              />
              {!temperatureValid && (
                <span role="alert" className="font-normal text-destructive">
                  Must be between {MIN_TEMPERATURE} and {MAX_TEMPERATURE}, or empty to use the provider default.
                </span>
              )}
            </label>
            <label className="grid gap-1.5 text-xs font-medium">
              Max tokens
              <Input
                value={maxTokensDraft}
                onChange={(event) => setMaxTokensDraft(event.target.value)}
                inputMode="numeric"
                placeholder={
                  provider?.defaultMaxTokens != null
                    ? `Provider default (${provider.defaultMaxTokens})`
                    : "Provider default (none)"
                }
                aria-invalid={!maxTokensValid}
                className={cn(!maxTokensValid && "border-destructive focus-visible:ring-destructive")}
              />
              {!maxTokensValid && (
                <span role="alert" className="font-normal text-destructive">
                  Must be a whole number between {MIN_MAX_TOKENS} and {MAX_MAX_TOKENS}, or empty to use the provider
                  default.
                </span>
              )}
            </label>
            <div className="flex justify-end gap-2">
              <Button type="button" variant="secondary" onClick={() => setOpen(false)}>
                Cancel
              </Button>
              <Button
                type="button"
                onClick={() => void save()}
                disabled={saving || !temperatureValid || !maxTokensValid}
              >
                {saving ? <Loader2 className="h-4 w-4 animate-spin" /> : null}
                Save
              </Button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

/**
 * CMP-003: the built-in "notes" tool's UI — a per-conversation scratch note list. Every write
 * (create/update/delete) goes through the same preview-then-approve flow the backend enforces: an
 * attempt without a currently valid grant comes back as a typed `approval_required` error, at
 * which point this component fetches the human-readable preview and shows an inline Approve
 * step; approving resubmits the same write with `approve: true`, which both performs it and
 * creates a short-lived grant so the next write in this session doesn't need to ask again.
 */
function ConversationNotesButton({
  conversation,
  onError,
}: {
  conversation?: Conversation;
  onError: (message: string) => void;
}) {
  const client = useArkClient();
  const [open, setOpen] = React.useState(false);
  const [notes, setNotes] = React.useState<ConversationNote[]>([]);
  const [loading, setLoading] = React.useState(false);
  const [draft, setDraft] = React.useState("");
  const [editingId, setEditingId] = React.useState<string | null>(null);
  const [editDraft, setEditDraft] = React.useState("");
  const [pendingApproval, setPendingApproval] = React.useState<{
    preview: SideEffectPreview;
    run: () => Promise<void>;
  } | null>(null);
  const [busy, setBusy] = React.useState(false);
  const containerRef = React.useRef<HTMLDivElement | null>(null);

  const refresh = React.useCallback(
    async (conversationId: string) => {
      setLoading(true);
      try {
        setNotes(await client.listConversationNotes(conversationId));
      } catch (error) {
        onError(getErrorMessage(error));
      } finally {
        setLoading(false);
      }
    },
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [],
  );

  React.useEffect(() => {
    if (open && conversation) void refresh(conversation.id);
  }, [open, conversation, refresh]);

  React.useEffect(() => {
    if (!open) return;
    function handlePointerDown(event: MouseEvent) {
      if (containerRef.current && !containerRef.current.contains(event.target as Node)) {
        setOpen(false);
      }
    }
    function handleKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape") setOpen(false);
    }
    document.addEventListener("mousedown", handlePointerDown);
    document.addEventListener("keydown", handleKeyDown);
    return () => {
      document.removeEventListener("mousedown", handlePointerDown);
      document.removeEventListener("keydown", handleKeyDown);
    };
  }, [open]);

  /** Runs `attempt` with `approve: false` first; if the backend reports `approval_required`,
   * fetches the preview and stashes a retry (`approve: true`) for the user to confirm instead of
   * failing outright. */
  async function attemptWrite(attempt: (approve: boolean) => Promise<void>, preview: () => Promise<SideEffectPreview>) {
    setBusy(true);
    try {
      await attempt(false);
      setPendingApproval(null);
    } catch (error) {
      const code =
        error && typeof error === "object" && "code" in error ? (error as { code?: string }).code : undefined;
      if (code === "approval_required") {
        try {
          const shown = await preview();
          setPendingApproval({ preview: shown, run: () => attempt(true) });
        } catch (previewError) {
          onError(getErrorMessage(previewError));
        }
      } else {
        onError(getErrorMessage(error));
      }
    } finally {
      setBusy(false);
    }
  }

  async function addNote() {
    const content = draft.trim();
    if (!content || !conversation) return;
    const conversationId = conversation.id;
    await attemptWrite(
      async (approve) => {
        await client.createNote(conversationId, content, approve);
        setDraft("");
        await refresh(conversationId);
      },
      () => client.previewNoteWrite("create", content),
    );
  }

  async function saveEdit(id: string) {
    const content = editDraft.trim();
    if (!content || !conversation) return;
    const conversationId = conversation.id;
    await attemptWrite(
      async (approve) => {
        await client.updateNote(id, content, approve);
        setEditingId(null);
        await refresh(conversationId);
      },
      () => client.previewNoteWrite("update", content),
    );
  }

  async function removeNote(id: string) {
    if (!conversation) return;
    const conversationId = conversation.id;
    await attemptWrite(
      async (approve) => {
        await client.deleteNote(id, approve);
        await refresh(conversationId);
      },
      () => client.previewNoteWrite("delete"),
    );
  }

  return (
    <div className="relative" ref={containerRef}>
      <button
        type="button"
        aria-haspopup="dialog"
        aria-expanded={open}
        aria-label={notes.length > 0 ? `Notes (${notes.length})` : "Notes"}
        onClick={() => setOpen((value) => !value)}
        disabled={!conversation}
        className="relative flex h-9 w-9 items-center justify-center rounded-md border border-input bg-background outline-none hover:bg-accent focus-visible:ring-2 focus-visible:ring-ring disabled:cursor-not-allowed disabled:opacity-40"
      >
        <StickyNote className="h-4 w-4" aria-hidden="true" />
      </button>

      {open && (
        <div
          role="dialog"
          aria-label="Conversation notes"
          className="absolute right-0 top-11 z-50 grid w-80 gap-3 rounded-md border border-border bg-popover p-3 text-popover-foreground shadow-md"
        >
          <div className="text-sm font-semibold">Notes</div>

          {pendingApproval && (
            <div className="grid gap-2 rounded-md border border-warning/50 bg-warning/10 p-2 text-xs">
              <div>{pendingApproval.preview.summary}</div>
              <div className="flex justify-end gap-2">
                <Button variant="ghost" onClick={() => setPendingApproval(null)}>
                  Cancel
                </Button>
                <Button
                  variant="secondary"
                  disabled={busy}
                  onClick={() => {
                    const run = pendingApproval.run;
                    setPendingApproval(null);
                    void (async () => {
                      setBusy(true);
                      try {
                        await run();
                      } catch (error) {
                        onError(getErrorMessage(error));
                      } finally {
                        setBusy(false);
                      }
                    })();
                  }}
                >
                  Approve
                </Button>
              </div>
            </div>
          )}

          {loading ? (
            <div className="flex items-center gap-2 text-xs text-muted-foreground">
              <Loader2 className="h-3.5 w-3.5 animate-spin" /> Loading notes…
            </div>
          ) : (
            <ul className="grid gap-2">
              {notes.length === 0 && <li className="text-xs text-muted-foreground">No notes yet.</li>}
              {notes.map((note) => (
                <li key={note.id} className="rounded-md border border-border p-2 text-xs">
                  {editingId === note.id ? (
                    <div className="grid gap-2">
                      <Textarea
                        value={editDraft}
                        onChange={(event) => setEditDraft(event.target.value)}
                        rows={2}
                        className="text-xs"
                      />
                      <div className="flex justify-end gap-2">
                        <Button variant="ghost" onClick={() => setEditingId(null)}>
                          Cancel
                        </Button>
                        <Button variant="secondary" disabled={busy} onClick={() => void saveEdit(note.id)}>
                          Save
                        </Button>
                      </div>
                    </div>
                  ) : (
                    <div className="flex items-start gap-2">
                      <span className="whitespace-pre-wrap break-words">{note.content}</span>
                      <div className="ml-auto flex shrink-0 gap-1">
                        <button
                          type="button"
                          aria-label="Edit note"
                          className="rounded p-1 hover:bg-accent"
                          onClick={() => {
                            setEditingId(note.id);
                            setEditDraft(note.content);
                          }}
                        >
                          <Pencil className="h-3.5 w-3.5" />
                        </button>
                        <button
                          type="button"
                          aria-label="Delete note"
                          className="rounded p-1 hover:bg-accent"
                          onClick={() => void removeNote(note.id)}
                        >
                          <Trash2 className="h-3.5 w-3.5" />
                        </button>
                      </div>
                    </div>
                  )}
                </li>
              ))}
            </ul>
          )}

          <div className="flex gap-2 border-t border-border pt-2">
            <Textarea
              value={draft}
              onChange={(event) => setDraft(event.target.value)}
              placeholder="Add a note…"
              rows={2}
              className="text-xs"
            />
            <Button variant="secondary" disabled={busy || !draft.trim()} onClick={() => void addNote()}>
              Add
            </Button>
          </div>
        </div>
      )}
    </div>
  );
}

/**
 * FTR-009 AC3: shown instead of the generic `SetupBanner` "install or select a model" message
 * when Ark specifically knows this conversation's own model was removed (still present in the
 * provider's model list, just marked unavailable) — names it explicitly and offers other
 * available models from the same provider as one-click alternatives, rather than leaving the
 * user to guess why sending is disabled or to reopen the provider dropdown themselves.
 */
function UnavailableModelNotice({
  modelName,
  alternatives,
  onSelectAlternative,
  onRefresh,
}: {
  modelName: string;
  alternatives: ModelInfo[];
  onSelectAlternative: (name: string) => void;
  onRefresh: () => void;
}) {
  return (
    <div className="flex flex-wrap items-start justify-between gap-3 rounded-md border border-amber-500/25 bg-amber-500/10 px-3 py-2 text-sm">
      <div className="flex min-w-0 items-start gap-2">
        <FileText className="mt-0.5 h-4 w-4 shrink-0 text-amber-500" aria-hidden="true" />
        <div className="min-w-0">
          <div className="font-medium">"{modelName}" is no longer available</div>
          <div className="text-muted-foreground">
            This conversation was using a model that Ark can't find anymore — it may have been deleted or renamed.
            {alternatives.length > 0 ? " Choose a compatible alternative:" : " Refresh models or install it again."}
          </div>
          {alternatives.length > 0 && (
            <div className="mt-2 flex flex-wrap gap-1.5">
              {alternatives.map((alternative) => (
                <button
                  key={alternative.id}
                  type="button"
                  onClick={() => onSelectAlternative(alternative.name)}
                  className="rounded-md border border-input bg-background px-2 py-1 text-xs font-medium outline-none hover:bg-accent focus-visible:ring-2 focus-visible:ring-ring"
                >
                  {alternative.displayName ?? alternative.name}
                </button>
              ))}
            </div>
          )}
        </div>
      </div>
      <Button size="sm" onClick={onRefresh} className="shrink-0">
        Refresh
      </Button>
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
            className="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm outline-none transition-colors duration-fast hover:bg-accent hover:text-accent-foreground focus-visible:ring-2 focus-visible:ring-ring disabled:cursor-not-allowed disabled:opacity-40"
          >
            <FileText className="h-4 w-4" aria-hidden="true" />
            Export as Markdown
          </button>
          <button
            type="button"
            role="menuitem"
            disabled={!conversationSelected}
            onClick={() => runAndClose(onExportJson)}
            className="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm outline-none transition-colors duration-fast hover:bg-accent hover:text-accent-foreground focus-visible:ring-2 focus-visible:ring-ring disabled:cursor-not-allowed disabled:opacity-40"
          >
            <FileJson className="h-4 w-4" aria-hidden="true" />
            Export as JSON
          </button>
          <button
            type="button"
            role="menuitem"
            onClick={() => runAndClose(onImportJson)}
            className="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm outline-none transition-colors duration-fast hover:bg-accent hover:text-accent-foreground focus-visible:ring-2 focus-visible:ring-ring"
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
            className="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm text-destructive outline-none transition-colors duration-fast hover:bg-destructive/10 focus-visible:ring-2 focus-visible:ring-ring disabled:cursor-not-allowed disabled:opacity-40"
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
