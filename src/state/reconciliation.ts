import type { Conversation, StreamEvent } from "../types/ark";

export type StreamDecision = "apply-delta" | "apply-terminal" | "ignore-duplicate" | "refetch";

export function classifyStreamEvent(lastRevision: number | undefined, event: StreamEvent): StreamDecision {
  if (event.delta == null) return "apply-terminal";
  if (event.revision == null || event.revision < 1) return "refetch";
  if (event.revision === (lastRevision ?? 0) + 1) return "apply-delta";
  if (lastRevision != null && event.revision <= lastRevision) return "ignore-duplicate";
  return "refetch";
}

export function isLatestRequest(completedSequence: number, latestSequence: number): boolean {
  return completedSequence === latestSequence;
}

export function mergeConversationPage(current: Conversation[], incoming: Conversation[]): Conversation[] {
  const seen = new Set(current.map((conversation) => conversation.id));
  return [...current, ...incoming.filter((conversation) => !seen.has(conversation.id))];
}

export function preserveSelectedConversation(
  current: Conversation | undefined,
  refreshed: Conversation[],
): Conversation | undefined {
  if (!current) return undefined;
  return refreshed.find((conversation) => conversation.id === current.id) ?? current;
}
