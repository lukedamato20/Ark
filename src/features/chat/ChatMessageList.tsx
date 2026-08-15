import { AlertTriangle, Edit3, FileText, GitBranch, Info, Loader2, RefreshCw } from "lucide-react";
import * as React from "react";
import { getErrorMessage } from "../../lib/arkErrors";
import { cn } from "../../lib/cn";
import { CONNECTION_METADATA } from "../../lib/destinationClass";
import { computeAnnouncementDelta } from "../../lib/streamAnnouncement";
import { messageWithGenerationOverlay } from "../../state/arkStores";
import { useStoreSelector } from "../../state/externalStore";
import { useArkStores } from "../../state/useArkStores";
import type { Attachment, BranchAlternative, Message, ProviderConfig } from "../../types/ark";
import { Badge } from "../../ui/badge";
import { Button } from "../../ui/button";
import { MarkdownMessage } from "./MarkdownMessage";

interface ChatMessageListProps {
  messages: Message[];
  providers: ProviderConfig[];
  /** CMP-001: message id -> the attachments sent with it. Only user messages ever have entries;
   * looked up once per bubble rather than passed as a per-message list, since most conversations
   * have few or no attachments at all. */
  attachmentsByMessageId: Record<string, Attachment[]>;
  canBranch: boolean;
  canSwitchBranch: boolean;
  editingMessageId: string | null;
  onStartEdit: (message: Message) => void;
  onSaveEdit: (message: Message, content: string) => void;
  onCancelEdit: () => void;
  onRegenerate: (message: Message) => void;
  onLoadAlternatives: (message: Message) => Promise<BranchAlternative[]>;
  onSwitchBranch: (messageId: string) => Promise<void>;
  /** FTR-005: full content, unlike the 140-character preview `onLoadAlternatives` returns —
   * used by the branch comparison view. */
  onLoadMessage: (messageId: string) => Promise<Message>;
  /** FTR-005: `name: null` clears the label back to the default ordinal presentation. */
  onRenameBranch: (messageId: string, name: string | null) => Promise<void>;
  onKeepPartial: (message: Message) => Promise<void>;
  onDiscardInterrupted: (message: Message) => Promise<void>;
  onError: (message: string) => void;
}

/**
 * ARC-008: the transcript list owns message rendering only. The list itself receives stable
 * durable messages; each memoized bubble subscribes to its own generation overlay, so a token
 * for message A cannot rerender completed message B or the ChatView shell.
 */
export function ChatMessageList({
  messages,
  providers,
  attachmentsByMessageId,
  canBranch,
  canSwitchBranch,
  editingMessageId,
  onStartEdit,
  onSaveEdit,
  onCancelEdit,
  onRegenerate,
  onLoadAlternatives,
  onSwitchBranch,
  onLoadMessage,
  onRenameBranch,
  onKeepPartial,
  onDiscardInterrupted,
  onError,
}: ChatMessageListProps) {
  return (
    <div className="mx-auto flex w-full max-w-3xl flex-col gap-5">
      {messages.map((message) => (
        <MessageBubble
          key={message.id}
          message={message}
          provider={providers.find((item) => item.id === message.providerId)}
          providers={providers}
          attachments={attachmentsByMessageId[message.id]}
          canBranch={canBranch}
          canSwitchBranch={canSwitchBranch}
          isEditing={editingMessageId === message.id}
          onStartEdit={onStartEdit}
          onSaveEdit={onSaveEdit}
          onCancelEdit={onCancelEdit}
          onRegenerate={onRegenerate}
          onLoadAlternatives={onLoadAlternatives}
          onSwitchBranch={onSwitchBranch}
          onLoadMessage={onLoadMessage}
          onRenameBranch={onRenameBranch}
          onKeepPartial={onKeepPartial}
          onDiscardInterrupted={onDiscardInterrupted}
          onError={onError}
        />
      ))}
    </div>
  );
}

const MessageBubble = React.memo(function MessageBubble({
  message,
  provider,
  providers,
  attachments,
  canBranch,
  canSwitchBranch,
  isEditing,
  onStartEdit,
  onSaveEdit,
  onCancelEdit,
  onRegenerate,
  onLoadAlternatives,
  onSwitchBranch,
  onLoadMessage,
  onRenameBranch,
  onKeepPartial,
  onDiscardInterrupted,
  onError,
}: Omit<ChatMessageListProps, "messages" | "editingMessageId" | "attachmentsByMessageId"> & {
  message: Message;
  provider: ProviderConfig | undefined;
  attachments: Attachment[] | undefined;
  isEditing: boolean;
}) {
  const stores = useArkStores();
  const overlay = useStoreSelector(
    stores.generation,
    React.useCallback((state) => state.byMessageId[message.id], [message.id]),
  );
  const renderedMessage = messageWithGenerationOverlay(message, overlay);
  const isUser = message.role === "user";
  const displayContent = renderedMessage.content;
  const isInterrupted = !isUser && renderedMessage.status === "interrupted";
  const isStreaming = !isUser && renderedMessage.status === "streaming";

  // UX-006: a throttled, sr-only live region for the streaming response — announcing every
  // token would violate this task's own acceptance criterion, so this ticks on an interval
  // (not on every `displayContent` change) and announces only the new slice since the last
  // tick, not the whole accumulated text. `displayContentRef` decouples the interval's closure
  // from the per-render `displayContent` value, since the effect intentionally does not restart
  // on every content change (that would defeat the throttle).
  const [streamAnnouncement, setStreamAnnouncement] = React.useState("");
  const displayContentRef = React.useRef(displayContent);
  displayContentRef.current = displayContent;
  const announcedLengthRef = React.useRef(0);

  React.useEffect(() => {
    if (!isStreaming) return;
    const flush = () => {
      const { delta, nextLength } = computeAnnouncementDelta(displayContentRef.current, announcedLengthRef.current);
      if (delta) {
        announcedLengthRef.current = nextLength;
        setStreamAnnouncement(delta);
      }
    };
    const timer = window.setInterval(flush, 2000);
    return () => {
      window.clearInterval(timer);
      flush(); // announce any remaining tail once streaming stops, rather than losing it
    };
  }, [isStreaming]);
  const [alternatives, setAlternatives] = React.useState<BranchAlternative[] | null>(null);
  const [isLoadingAlternatives, setIsLoadingAlternatives] = React.useState(false);
  const [switchingBranchId, setSwitchingBranchId] = React.useState<string | null>(null);
  const [recoveryAction, setRecoveryAction] = React.useState<"retry" | "keep" | "discard" | null>(null);
  // FTR-005: branch (message revision) naming — edited inline per alternative row.
  const [renamingId, setRenamingId] = React.useState<string | null>(null);
  const [renameDraft, setRenameDraft] = React.useState("");
  const [renameSaving, setRenameSaving] = React.useState(false);
  // FTR-005: comparison view — pick exactly two alternatives to see side-by-side with full
  // content and provenance, rather than switching back and forth to re-read each one.
  const [compareMode, setCompareMode] = React.useState(false);
  const [compareSelection, setCompareSelection] = React.useState<string[]>([]);
  const [comparisonMessages, setComparisonMessages] = React.useState<Record<string, Message> | null>(null);
  const [isLoadingComparison, setIsLoadingComparison] = React.useState(false);
  // UX-011: "useful persisted metadata is hidden" — collapsed by default per this task's own
  // suggested-implementation-notes ("a compact disclosure row with an expandable detail view"),
  // since always-visible per-message metadata was flagged as a clutter risk.
  const [metadataOpen, setMetadataOpen] = React.useState(false);
  const activeAlternativeIndex = alternatives?.findIndex((alternative) => alternative.isActive) ?? -1;
  const branchLabel =
    alternatives && alternatives.length > 1 && activeAlternativeIndex >= 0
      ? `${activeAlternativeIndex + 1}/${alternatives.length}`
      : "Branches";

  async function handleRetryInterrupted() {
    setRecoveryAction("retry");
    try {
      onRegenerate(message);
    } finally {
      setRecoveryAction(null);
    }
  }

  async function handleKeepPartialClick() {
    setRecoveryAction("keep");
    try {
      await onKeepPartial(message);
    } catch (error) {
      onError(getErrorMessage(error));
    } finally {
      setRecoveryAction(null);
    }
  }

  async function handleDiscardClick() {
    setRecoveryAction("discard");
    try {
      await onDiscardInterrupted(message);
    } catch (error) {
      onError(getErrorMessage(error));
    } finally {
      setRecoveryAction(null);
    }
  }

  async function handleToggleAlternatives() {
    if (isUser) return;
    if (alternatives) {
      setAlternatives(null);
      setCompareMode(false);
      setCompareSelection([]);
      setComparisonMessages(null);
      setRenamingId(null);
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
    if (messageId === message.id || switchingBranchId) return;
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

  function handleStartRename(alternative: BranchAlternative) {
    setRenamingId(alternative.messageId);
    setRenameDraft(alternative.branchName ?? "");
  }

  async function handleSaveRename(messageId: string) {
    setRenameSaving(true);
    try {
      await onRenameBranch(messageId, renameDraft.trim() || null);
      setAlternatives(
        (current) =>
          current?.map((alternative) =>
            alternative.messageId === messageId
              ? { ...alternative, branchName: renameDraft.trim() || null }
              : alternative,
          ) ?? null,
      );
      setRenamingId(null);
    } catch (error) {
      onError(getErrorMessage(error));
    } finally {
      setRenameSaving(false);
    }
  }

  function handleToggleCompareSelection(messageId: string) {
    setCompareSelection((current) => {
      if (current.includes(messageId)) return current.filter((id) => id !== messageId);
      if (current.length >= 2) return [current[1], messageId];
      return [...current, messageId];
    });
  }

  async function handleViewComparison() {
    if (compareSelection.length !== 2) return;
    setIsLoadingComparison(true);
    try {
      const [first, second] = await Promise.all(compareSelection.map((id) => onLoadMessage(id)));
      setComparisonMessages({ [first.id]: first, [second.id]: second });
    } catch (error) {
      onError(getErrorMessage(error));
    } finally {
      setIsLoadingComparison(false);
    }
  }

  return (
    <article className={cn("flex", isUser ? "justify-end" : "justify-start")}>
      <div
        className={cn(
          "rounded-lg border px-4 py-3",
          // UX-003: assistant messages (prose, code, tables) use the full readable column set by
          // the outer `max-w-3xl` container; user messages — short prompts, not technical output
          // — stay visually constrained as a chat bubble rather than stretching edge-to-edge.
          isUser
            ? "max-w-[75%] border-primary/25 bg-primary text-primary-foreground"
            : "max-w-full border-border bg-card",
          isEditing && "w-full max-w-full",
        )}
      >
        <div className="mb-2 flex items-center justify-between gap-3">
          <div className="flex items-center gap-1.5">
            <span className="text-xs font-medium uppercase tracking-wide opacity-70">{isUser ? "You" : "Ark"}</span>
            {provider && (
              <Button
                size="sm"
                variant="ghost"
                onClick={() => setMetadataOpen((value) => !value)}
                aria-expanded={metadataOpen}
                aria-label={metadataOpen ? "Hide response details" : "Show response details"}
                className="h-5 w-5 p-0 opacity-50 hover:opacity-100"
              >
                <Info className="h-3 w-3" />
              </Button>
            )}
          </div>
          <div className="flex items-center gap-2">
            {renderedMessage.status !== "complete" && renderedMessage.status !== "streaming" && (
              <Badge tone={renderedMessage.status === "failed" ? "danger" : "warning"}>{renderedMessage.status}</Badge>
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
                  disabled={!canSwitchBranch || renderedMessage.status === "streaming" || isLoadingAlternatives}
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
                  disabled={!canBranch || renderedMessage.status === "streaming"}
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
        {metadataOpen && provider && (
          <div
            className={cn(
              "mb-3 grid gap-1 rounded-md border px-2.5 py-2 text-xs",
              isUser ? "border-primary-foreground/20 bg-primary-foreground/10" : "border-border/70 bg-muted/40",
            )}
          >
            <MetadataRow label="Provider" value={provider.name} />
            {message.modelId && <MetadataRow label="Model" value={message.modelId} />}
            <MetadataRow
              label="Route"
              value={CONNECTION_METADATA[provider.destinationClass].label}
              detail={CONNECTION_METADATA[provider.destinationClass].description}
            />
            {message.tokenCount != null && <MetadataRow label="Tokens" value={String(message.tokenCount)} />}
          </div>
        )}
        {alternatives && !isUser && (
          <div className="mb-3 border-t border-border/70 pt-2">
            {alternatives.length <= 1 ? (
              <div className="text-xs text-muted-foreground">No alternate responses saved yet.</div>
            ) : (
              <>
                <div className="mb-1.5 flex items-center justify-between">
                  <span className="text-[10px] font-medium uppercase tracking-wide text-muted-foreground">
                    {compareMode ? "Select two to compare" : "Branches"}
                  </span>
                  <Button
                    size="sm"
                    variant="ghost"
                    className="h-5 px-1.5 text-[10px]"
                    onClick={() => {
                      setCompareMode((value) => !value);
                      setCompareSelection([]);
                      setComparisonMessages(null);
                      setRenamingId(null);
                    }}
                  >
                    {compareMode ? "Cancel compare" : "Compare"}
                  </Button>
                </div>
                <div className="space-y-1">
                  {alternatives.map((alternative, index) => {
                    const isRenaming = renamingId === alternative.messageId;
                    const label = alternative.branchName || `Response ${index + 1}`;
                    return (
                      <div
                        key={alternative.messageId}
                        className={cn(
                          "flex w-full items-start justify-between gap-3 rounded-md px-2 py-1.5 text-left text-xs transition-colors duration-fast",
                          alternative.isActive && !compareMode
                            ? "bg-primary/10 text-foreground"
                            : "hover:bg-accent hover:text-accent-foreground",
                        )}
                      >
                        {compareMode && (
                          <input
                            type="checkbox"
                            className="mt-0.5 h-3.5 w-3.5 shrink-0"
                            checked={compareSelection.includes(alternative.messageId)}
                            onChange={() => handleToggleCompareSelection(alternative.messageId)}
                            aria-label={`Select ${label} for comparison`}
                          />
                        )}
                        <button
                          type="button"
                          disabled={
                            !compareMode && (alternative.isActive || !canSwitchBranch || Boolean(switchingBranchId))
                          }
                          onClick={() =>
                            compareMode
                              ? handleToggleCompareSelection(alternative.messageId)
                              : void handleSelectAlternative(alternative.messageId)
                          }
                          className="min-w-0 flex-1 text-left outline-none focus-visible:ring-2 focus-visible:ring-ring disabled:cursor-default"
                        >
                          {isRenaming ? (
                            <input
                              autoFocus
                              value={renameDraft}
                              onChange={(event) => setRenameDraft(event.target.value)}
                              onClick={(event) => event.stopPropagation()}
                              onKeyDown={(event) => {
                                if (event.key === "Enter") void handleSaveRename(alternative.messageId);
                                if (event.key === "Escape") setRenamingId(null);
                              }}
                              maxLength={80}
                              placeholder={`Response ${index + 1}`}
                              className="w-full rounded border border-input bg-background px-1.5 py-0.5 text-xs outline-none focus-visible:ring-2 focus-visible:ring-ring"
                            />
                          ) : (
                            <span className="block font-medium">{label}</span>
                          )}
                          <span className="mt-0.5 block truncate text-muted-foreground">
                            {alternative.contentPreview}
                          </span>
                        </button>
                        <span className="flex shrink-0 items-center gap-1.5">
                          {switchingBranchId === alternative.messageId && <Loader2 className="h-3 w-3 animate-spin" />}
                          {alternative.hasDescendants && !alternative.isActive && (
                            <span className="text-[10px] text-muted-foreground">+ history</span>
                          )}
                          {!compareMode &&
                            (isRenaming ? (
                              <Button
                                size="sm"
                                variant="ghost"
                                className="h-5 px-1 text-[10px]"
                                disabled={renameSaving}
                                onClick={(event) => {
                                  event.stopPropagation();
                                  void handleSaveRename(alternative.messageId);
                                }}
                              >
                                {renameSaving ? <Loader2 className="h-3 w-3 animate-spin" /> : "Save"}
                              </Button>
                            ) : (
                              <Button
                                size="sm"
                                variant="ghost"
                                className="h-5 px-1 text-[10px] opacity-60 hover:opacity-100"
                                aria-label={`Rename ${label}`}
                                onClick={(event) => {
                                  event.stopPropagation();
                                  handleStartRename(alternative);
                                }}
                              >
                                <Edit3 className="h-3 w-3" />
                              </Button>
                            ))}
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
                      </div>
                    );
                  })}
                </div>
                {compareMode && (
                  <div className="mt-2 flex justify-end">
                    <Button
                      size="sm"
                      disabled={compareSelection.length !== 2 || isLoadingComparison}
                      onClick={() => void handleViewComparison()}
                    >
                      {isLoadingComparison ? <Loader2 className="h-3.5 w-3.5 animate-spin" /> : null}
                      View comparison
                    </Button>
                  </div>
                )}
                {comparisonMessages && (
                  <BranchComparison
                    messages={compareSelection.map((id) => comparisonMessages[id]).filter(Boolean)}
                    alternatives={alternatives}
                    providers={providers}
                    onClose={() => setComparisonMessages(null)}
                  />
                )}
              </>
            )}
          </div>
        )}
        {!isUser && (
          <div className="sr-only" role="status" aria-live="polite" aria-atomic="true">
            {streamAnnouncement}
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
        {isUser && attachments && attachments.length > 0 && (
          <div className="mt-2 flex flex-wrap gap-1.5">
            {attachments.map((attachment) => (
              <span
                key={attachment.id}
                className="inline-flex items-center gap-1.5 rounded-md border border-border bg-muted/50 px-2 py-1 text-xs"
              >
                <FileText className="h-3.5 w-3.5 text-muted-foreground" aria-hidden="true" />
                <span className="max-w-40 truncate" title={attachment.fileName}>
                  {attachment.fileName}
                </span>
                <span className="text-muted-foreground">{(attachment.byteSize / 1024).toFixed(1)} KB</span>
              </span>
            ))}
          </div>
        )}
        {renderedMessage.errorMessage && (
          <div className="mt-2 text-xs text-destructive">{renderedMessage.errorMessage}</div>
        )}
        {isInterrupted && (
          <div
            role="alert"
            className="mt-3 rounded-md border border-amber-500/30 bg-amber-500/10 p-2.5 text-xs text-foreground"
          >
            <div className="mb-2 flex items-center gap-1.5 font-medium text-amber-700 dark:text-amber-300">
              <AlertTriangle className="h-3.5 w-3.5 shrink-0" aria-hidden="true" />
              This response was interrupted before it finished (app restart or crash).
            </div>
            <div className="flex flex-wrap gap-1.5">
              <Button
                size="sm"
                variant="secondary"
                onClick={() => void handleRetryInterrupted()}
                disabled={!canBranch || recoveryAction !== null}
                className="h-6 px-2 text-xs"
              >
                {recoveryAction === "retry" ? (
                  <Loader2 className="h-3 w-3 animate-spin" />
                ) : (
                  <RefreshCw className="h-3 w-3" />
                )}
                Retry
              </Button>
              <Button
                size="sm"
                variant="secondary"
                onClick={() => void handleKeepPartialClick()}
                disabled={!displayContent || recoveryAction !== null}
                className="h-6 px-2 text-xs"
                aria-label="Keep the partial response as the final answer"
              >
                {recoveryAction === "keep" && <Loader2 className="h-3 w-3 animate-spin" />}
                Keep partial
              </Button>
              <Button
                size="sm"
                variant="ghost"
                onClick={() => void handleDiscardClick()}
                disabled={recoveryAction !== null}
                className="h-6 px-2 text-xs"
                aria-label="Discard this interrupted response"
              >
                {recoveryAction === "discard" && <Loader2 className="h-3 w-3 animate-spin" />}
                Discard
              </Button>
            </div>
          </div>
        )}
      </div>
    </article>
  );
});

function MetadataRow({ label, value, detail }: { label: string; value: string; detail?: string }) {
  return (
    <div className="flex items-baseline justify-between gap-3">
      <span className="opacity-60">{label}</span>
      <span className="text-right" title={detail}>
        {value}
      </span>
    </div>
  );
}

/**
 * FTR-005: renders two selected branch alternatives side by side (stacked on narrow viewports
 * via CSS grid's auto-fit) with full content and lightweight provenance — the same
 * provider/model fields `MetadataRow` already shows for a single message, not a new parsed
 * view of `metadata_json`.
 */
function BranchComparison({
  messages,
  alternatives,
  providers,
  onClose,
}: {
  messages: Message[];
  alternatives: BranchAlternative[];
  providers: ProviderConfig[];
  onClose: () => void;
}) {
  if (messages.length !== 2) return null;
  return (
    <div className="mt-3 rounded-md border border-border/70 bg-muted/20 p-2.5">
      <div className="mb-2 flex items-center justify-between">
        <span className="text-[10px] font-medium uppercase tracking-wide text-muted-foreground">Comparison</span>
        <Button size="sm" variant="ghost" className="h-5 px-1.5 text-[10px]" onClick={onClose}>
          Close
        </Button>
      </div>
      <div className="grid gap-3 sm:grid-cols-2">
        {messages.map((compared) => {
          const alternative = alternatives.find((item) => item.messageId === compared.id);
          const comparedProvider = providers.find((item) => item.id === compared.providerId);
          return (
            <div key={compared.id} className="min-w-0 rounded-md border border-border/70 bg-background p-2.5 text-xs">
              <div className="mb-1.5 font-medium">
                {alternative?.branchName || (alternative?.isActive ? "Active response" : "Alternative response")}
              </div>
              <div className="mb-2 grid gap-0.5 text-[11px] text-muted-foreground">
                {comparedProvider && <MetadataRow label="Provider" value={comparedProvider.name} />}
                {compared.modelId && <MetadataRow label="Model" value={compared.modelId} />}
                {compared.tokenCount != null && <MetadataRow label="Tokens" value={String(compared.tokenCount)} />}
              </div>
              <div className="max-h-80 overflow-y-auto break-words">
                <MarkdownMessage content={compared.content} />
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
}

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
            if (!unchanged && content.trim()) onSave(content.trim());
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
