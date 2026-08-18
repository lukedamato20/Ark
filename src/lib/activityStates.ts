export type ActivityState =
  "preparing" | "provider" | "generating" | "tool" | "approval" | "clarification" | "cancelling";

const LABELS: Record<ActivityState, string> = {
  preparing: "Preparing",
  provider: "Waiting for provider",
  generating: "Generating",
  tool: "Using tool",
  approval: "Waiting for approval",
  clarification: "Waiting for clarification",
  cancelling: "Cancelling",
};

export function activityLabel(state: ActivityState, trustedToolName?: string) {
  const safeToolName =
    trustedToolName && /^[\p{L}\p{N}][\p{L}\p{N} ._/-]{0,63}$/u.test(trustedToolName) ? trustedToolName : null;
  return state === "tool" && safeToolName ? `Using ${safeToolName}` : LABELS[state];
}
