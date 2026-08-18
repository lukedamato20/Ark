import type { CodeToolInvocation } from "../types/ark";

export type CodeTimelineItemKind =
  "clarification" | "approval" | "tool" | "model_text" | "tool_result" | "tool_error" | "system";

const APPROVAL_TOOLS = new Set(["edit_file", "git_checkpoint", "git_rollback", "run_verification_command"]);

/** Classify only from Ark's durable typed record, never from untrusted model/tool text. */
export function classifyCodeInvocation(invocation: CodeToolInvocation): CodeTimelineItemKind {
  if (invocation.toolName === "request_clarification") return "clarification";
  if (APPROVAL_TOOLS.has(invocation.toolName) && invocation.preview !== null) return "approval";
  return "tool";
}

export function codeProposalLabel(toolName: string): string {
  switch (toolName) {
    case "edit_file":
      return "Proposed file edit";
    case "git_checkpoint":
      return "Proposed Git checkpoint";
    case "git_rollback":
      return "Proposed Git rollback";
    case "run_verification_command":
      return "Proposed verification command";
    default:
      return "Proposed tool operation";
  }
}

export function codeInvocationStateLabel(invocation: CodeToolInvocation): string {
  if (classifyCodeInvocation(invocation) === "tool" && invocation.state === "applied") return "completed";
  return invocation.state.replaceAll("_", " ");
}

export function codeClarificationQuestion(json: string): string | null {
  try {
    const value = JSON.parse(json) as { question?: unknown };
    return typeof value.question === "string" ? value.question : null;
  } catch {
    return null;
  }
}

export const CODE_TIMELINE_RUN_PAGE_SIZE = 20;

export function windowCodeRuns<T>(runs: readonly T[], visibleCount: number): readonly T[] {
  return runs.slice(Math.max(0, runs.length - Math.max(1, visibleCount)));
}
