import { Download, Edit3, FileJson, FileText, GitBranch, Loader2, RefreshCw, Send, Square, Trash2 } from "lucide-react";
import * as React from "react";
import { cn } from "../../lib/cn";
import { downloadText, safeFilename } from "../../lib/download";
import {
  cancelStream,
  deleteConversation,
  editUserMessage,
  exportConversationJson,
  exportConversationMarkdown,
  getAssistantAlternatives,
  getErrorMessage,
  importConversationJson,
  regenerateAssistantMessage,
  refreshModels,
  renameConversation,
  sendChatMessage,
  switchActiveBranch,
} from "../../lib/api";
import type {
  BranchAlternative,
  Conversation,
  Message,
  ModelInfo,
  ProviderConfig,
  ProviderHealth,
  SendChatResult,
} from "../../types/ark";
import { Badge } from "../../ui/badge";
import { Button } from "../../ui/button";
import { Select } from "../../ui/select";
import { Textarea } from "../../ui/textarea";
import { SetupBanner } from "../onboarding/SetupBanner";
import { MarkdownMessage } from "./MarkdownMessage";

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
}: ChatViewProps) {
  const [draft, setDraft] = React.useState("");
  const [isSending, setIsSending] = React.useState(false);
  const [isRenaming, setIsRenaming] = React.useState(false);
  const [titleDraft, setTitleDraft] = React.useState("");
  const [providerId, setProviderId] = React.useState(providers[0]?.id ?? "");
  const [model, setModel] = React.useState("");
  const [editingMessageId, setEditingMessageId] = React.useState<string | null>(null);
  const fileInputRef = React.useRef<HTMLInputElement | null>(null);
  const previousConversationIdRef = React.useRef<string | undefined>(undefined);
  const autoRefreshedProviderIdsRef = React.useRef<Set<string>>(new Set());

  const provider = providers.find((item) => item.id === providerId) ?? providers[0];
  const health = providerHealth[provider?.id ?? ""] ?? null;
  const providerModels = models.filter((item) => item.providerId === (provider?.id ?? ""));
  const selectedModelAvailable = providerModels.some((item) => item.name === model && item.isAvailable);
  const activeAssistant = React.useMemo(
    () => [...messages].reverse().find((m) => m.role === "assistant" && m.status === "streaming"),
    [messages],
  );
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
      const result = await refreshModels(provider?.id ?? "");
      onModelsRefresh(result);
      if (!model && result.provider.defaultModelId) {
        setModel(result.provider.defaultModelId);
      }
    } catch (error) {
      onError(getErrorMessage(error));
    }
  }

  async function handleSend() {
    if (!conversation || !canSend) {
      return;
    }

    const content = draft.trim();
    setDraft("");
    setIsSending(true);

    try {
      const result: SendChatResult = await sendChatMessage({
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
    } catch (error) {
      setDraft(content);
      onError(getErrorMessage(error));
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
      const renamed = await renameConversation(conversation.id, nextTitle);
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
    [conversation?.id, provider?.id, selectedModelAvailable, !!activeAssistant],
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
        const result = await editUserMessage({
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
      } catch (error) {
        onError(getErrorMessage(error));
      }
    },
    [conversation?.id, provider, model, messages, selectedModelAvailable, !!activeAssistant, onMessagesChange, onError],
  );

  const handleCancelEdit = React.useCallback(() => setEditingMessageId(null), []);

  const handleRegenerateMessage = React.useCallback(
    async (message: Message) => {
      if (!conversation || !provider || !selectedModelAvailable || activeAssistant) {
        return;
      }

      try {
        const result = await regenerateAssistantMessage({
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
      } catch (error) {
        onError(getErrorMessage(error));
      }
    },
    [conversation?.id, provider, model, messages, selectedModelAvailable, !!activeAssistant, onMessagesChange, onError],
  );

  const handleLoadAlternatives = React.useCallback(
    async (message: Message) => {
      if (!conversation || message.role !== "assistant") {
        return [];
      }
      return getAssistantAlternatives(conversation.id, message.id);
    },
    [conversation?.id],
  );

  const handleSwitchBranch = React.useCallback(
    async (messageId: string) => {
      if (!conversation || activeAssistant) {
        return;
      }
      const nextMessages = await switchActiveBranch(conversation.id, messageId);
      onMessagesChange(nextMessages);
    },
    [conversation?.id, !!activeAssistant, onMessagesChange],
  );

  async function handleCancel() {
    if (!activeAssistant) {
      return;
    }
    try {
      await cancelStream(activeAssistant.id);
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
      await deleteConversation(conversation.id);
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
        const markdown = await exportConversationMarkdown(conversation.id);
        downloadText(`${safeFilename(conversation.title)}.md`, markdown, "text/markdown;charset=utf-8");
      } else {
        const json = await exportConversationJson(conversation.id);
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

    try {
      const json = await file.text();
      const imported = await importConversationJson(json);
      onConversationImported(imported);
    } catch (error) {
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
            <Badge tone={provider?.isLocal ? "success" : "warning"}>{provider?.isLocal ? "local" : "cloud"}</Badge>
            <span className="text-xs text-muted-foreground">{health?.message ?? "Runtime status unknown"}</span>
          </div>
        </div>

        <div className="flex items-center gap-2">
          <Select
            value={providerId}
            onChange={(event) => {
              setProviderId(event.target.value);
              setModel("");
            }}
            aria-label="Provider"
          >
            {providers.map((item) => (
              <option key={item.id} value={item.id}>
                {item.name}
              </option>
            ))}
          </Select>
          <Select
            value={model}
            onChange={(event) => setModel(event.target.value)}
            aria-label="Model"
            disabled={!providerModels.length}
            className="max-w-56"
          >
            {!providerModels.length && <option value="">No models</option>}
            {providerModels.map((item) => (
              <option key={item.id} value={item.name}>
                {item.displayName ?? item.name}
              </option>
            ))}
          </Select>
          <Button size="sm" variant="ghost" onClick={() => handleExport("markdown")} disabled={!conversation}>
            <FileText className="h-4 w-4" />
            Export
          </Button>
          <Button
            size="icon"
            variant="ghost"
            onClick={() => handleExport("json")}
            disabled={!conversation}
            aria-label="Export JSON"
          >
            <FileJson className="h-4 w-4" />
          </Button>
          <Button
            size="icon"
            variant="ghost"
            onClick={() => fileInputRef.current?.click()}
            aria-label="Import JSON"
          >
            <Download className="h-4 w-4" />
          </Button>
          <Button
            size="icon"
            variant="ghost"
            onClick={handleDelete}
            disabled={!conversation}
            aria-label="Delete conversation"
          >
            <Trash2 className="h-4 w-4" />
          </Button>
          <input ref={fileInputRef} type="file" accept="application/json,.json" className="hidden" onChange={handleImport} />
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
      </div>

      <div className="min-h-0 flex-1 overflow-y-auto px-4 py-5">
        {isLoading ? (
          <div className="flex h-full items-center justify-center text-sm text-muted-foreground">
            <Loader2 className="mr-2 h-4 w-4 animate-spin" />
            Loading conversation
          </div>
        ) : !conversation ? (
          <EmptyChat />
        ) : messages.length === 0 ? (
          <EmptyChat />
        ) : (
          <div className="mx-auto flex w-full max-w-3xl flex-col gap-5">
            {messages.map((message) => (
              <MessageBubble
                key={message.id}
                message={message}
                canBranch={selectedModelAvailable && !activeAssistant}
                canSwitchBranch={!activeAssistant}
                isEditing={editingMessageId === message.id}
                onStartEdit={handleStartEdit}
                onSaveEdit={handleSaveEdit}
                onCancelEdit={handleCancelEdit}
                onRegenerate={handleRegenerateMessage}
                onLoadAlternatives={handleLoadAlternatives}
                onSwitchBranch={handleSwitchBranch}
                onError={onError}
              />
            ))}
          </div>
        )}
      </div>

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

const MessageBubble = React.memo(function MessageBubble({
  message,
  canBranch,
  canSwitchBranch,
  isEditing,
  onStartEdit,
  onSaveEdit,
  onCancelEdit,
  onRegenerate,
  onLoadAlternatives,
  onSwitchBranch,
  onError,
}: {
  message: Message;
  canBranch: boolean;
  canSwitchBranch: boolean;
  isEditing: boolean;
  onStartEdit: (message: Message) => void;
  onSaveEdit: (message: Message, content: string) => void;
  onCancelEdit: () => void;
  onRegenerate: (message: Message) => void;
  onLoadAlternatives: (message: Message) => Promise<BranchAlternative[]>;
  onSwitchBranch: (messageId: string) => Promise<void>;
  onError: (message: string) => void;
}) {
  const isUser = message.role === "user";
  const displayContent = message.content;
  const [alternatives, setAlternatives] = React.useState<BranchAlternative[] | null>(null);
  const [isLoadingAlternatives, setIsLoadingAlternatives] = React.useState(false);
  const [switchingBranchId, setSwitchingBranchId] = React.useState<string | null>(null);
  const activeAlternativeIndex = alternatives?.findIndex((alternative) => alternative.isActive) ?? -1;
  const branchLabel =
    alternatives && alternatives.length > 1 && activeAlternativeIndex >= 0
      ? `${activeAlternativeIndex + 1}/${alternatives.length}`
      : "Branches";

  async function handleToggleAlternatives() {
    if (isUser) {
      return;
    }

    if (alternatives) {
      setAlternatives(null);
      return;
    }

    setIsLoadingAlternatives(true);
    try {
      setAlternatives(await onLoadAlternatives(message));
    } catch (error) {
      onError(getErrorMessage(error));
    } finally {
      setIsLoadingAlternatives(false);
    }
  }

  async function handleSelectAlternative(messageId: string) {
    if (messageId === message.id || switchingBranchId) {
      return;
    }

    setSwitchingBranchId(messageId);
    try {
      await onSwitchBranch(messageId);
      setAlternatives(null);
    } catch (error) {
      onError(getErrorMessage(error));
    } finally {
      setSwitchingBranchId(null);
    }
  }

  return (
    <article className={cn("flex", isUser ? "justify-end" : "justify-start")}>
      <div
        className={cn(
          "max-w-[88%] rounded-lg border px-4 py-3",
          isUser ? "border-primary/25 bg-primary text-primary-foreground" : "border-border bg-card",
          isEditing && "w-full max-w-full",
        )}
      >
        <div className="mb-2 flex items-center justify-between gap-3">
          <span className="text-xs font-medium uppercase tracking-wide opacity-70">{isUser ? "You" : "Ark"}</span>
          <div className="flex items-center gap-2">
            {message.status !== "complete" && message.status !== "streaming" && (
              <Badge tone={message.status === "failed" ? "danger" : "warning"}>{message.status}</Badge>
            )}
            {isUser ? (
              !isEditing && (
                <Button
                  size="sm"
                  variant="ghost"
                  onClick={() => onStartEdit(message)}
                  disabled={!canBranch}
                  aria-label="Edit message"
                  className="h-6 px-1.5 text-xs opacity-70"
                >
                  <Edit3 className="h-3.5 w-3.5" />
                  Edit
                </Button>
              )
            ) : (
              <>
                <Button
                  size="sm"
                  variant="ghost"
                  onClick={() => void handleToggleAlternatives()}
                  disabled={!canSwitchBranch || message.status === "streaming" || isLoadingAlternatives}
                  aria-label="Show response branches"
                  className="h-6 px-1.5 text-xs opacity-70"
                >
                  {isLoadingAlternatives ? (
                    <Loader2 className="h-3.5 w-3.5 animate-spin" />
                  ) : (
                    <GitBranch className="h-3.5 w-3.5" />
                  )}
                  {branchLabel}
                </Button>
                <Button
                  size="sm"
                  variant="ghost"
                  onClick={() => onRegenerate(message)}
                  disabled={!canBranch || message.status === "streaming"}
                  aria-label="Regenerate response"
                  className="h-6 px-1.5 text-xs opacity-70"
                >
                  <RefreshCw className="h-3.5 w-3.5" />
                  Retry
                </Button>
              </>
            )}
          </div>
        </div>
        {alternatives && !isUser && (
          <div className="mb-3 border-t border-border/70 pt-2">
            {alternatives.length <= 1 ? (
              <div className="text-xs text-muted-foreground">No alternate responses saved yet.</div>
            ) : (
              <div className="space-y-1">
                {alternatives.map((alternative, index) => (
                  <button
                    key={alternative.messageId}
                    type="button"
                    disabled={alternative.isActive || !canSwitchBranch || Boolean(switchingBranchId)}
                    onClick={() => void handleSelectAlternative(alternative.messageId)}
                    className={cn(
                      "flex w-full items-start justify-between gap-3 rounded-md px-2 py-1.5 text-left text-xs outline-none transition-colors duration-150",
                      "focus-visible:ring-2 focus-visible:ring-ring disabled:cursor-default",
                      alternative.isActive
                        ? "bg-primary/10 text-foreground"
                        : "hover:bg-accent hover:text-accent-foreground",
                    )}
                  >
                    <span className="min-w-0">
                      <span className="block font-medium">Response {index + 1}</span>
                      <span className="mt-0.5 block truncate text-muted-foreground">{alternative.contentPreview}</span>
                    </span>
                    <span className="flex shrink-0 items-center gap-1.5">
                      {switchingBranchId === alternative.messageId && <Loader2 className="h-3 w-3 animate-spin" />}
                      {alternative.hasDescendants && !alternative.isActive && (
                        <span className="text-[10px] text-muted-foreground">+ history</span>
                      )}
                      <Badge
                        tone={
                          alternative.isActive
                            ? "success"
                            : alternative.status === "failed"
                              ? "danger"
                              : alternative.status === "streaming"
                                ? "warning"
                                : "muted"
                        }
                      >
                        {alternative.isActive ? "active" : alternative.status}
                      </Badge>
                    </span>
                  </button>
                ))}
              </div>
            )}
          </div>
        )}
        {isUser && isEditing ? (
          <InlineEditor
            initialContent={message.content}
            onSave={(content) => onSaveEdit(message, content)}
            onCancel={onCancelEdit}
          />
        ) : displayContent ? (
          isUser ? (
            <div className="whitespace-pre-wrap text-sm leading-6">{displayContent}</div>
          ) : (
            <MarkdownMessage content={displayContent} />
          )
        ) : (
          <div className="flex items-center gap-2 text-sm text-muted-foreground">
            <Loader2 className="h-4 w-4 animate-spin" />
            Thinking
          </div>
        )}
        {message.errorMessage && <div className="mt-2 text-xs text-destructive">{message.errorMessage}</div>}
      </div>
    </article>
  );
});

function InlineEditor({
  initialContent,
  onSave,
  onCancel,
}: {
  initialContent: string;
  onSave: (content: string) => void;
  onCancel: () => void;
}) {
  const [content, setContent] = React.useState(initialContent);
  const unchanged = content.trim() === initialContent.trim();

  return (
    <div className="space-y-2">
      <textarea
        value={content}
        onChange={(event) => setContent(event.target.value)}
        autoFocus
        rows={Math.max(3, content.split("\n").length)}
        onKeyDown={(event) => {
          if (event.key === "Escape") {
            event.preventDefault();
            onCancel();
          }
          if (event.key === "Enter" && (event.metaKey || event.ctrlKey)) {
            event.preventDefault();
            if (!unchanged && content.trim()) {
              onSave(content.trim());
            }
          }
        }}
        className="w-full resize-none rounded-md border border-primary/40 bg-primary/5 px-3 py-2 text-sm leading-6 text-primary-foreground placeholder:text-primary-foreground/50 outline-none focus-visible:ring-2 focus-visible:ring-ring"
      />
      <div className="flex items-center gap-2">
        <Button
          size="sm"
          variant="primary"
          onClick={() => onSave(content.trim())}
          disabled={unchanged || !content.trim()}
        >
          Save &amp; Regenerate
        </Button>
        <Button size="sm" variant="ghost" onClick={onCancel} className="opacity-70">
          Cancel
        </Button>
        <span className="ml-auto text-xs opacity-50">Ctrl/Cmd+Enter to save · Esc to cancel</span>
      </div>
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
