import { cn } from "../lib/cn";
import { activityLabel, type ActivityState } from "../lib/activityStates";

export function ActivityIndicator({
  state,
  toolName,
  className,
  announce = true,
}: {
  state: ActivityState;
  toolName?: string;
  className?: string;
  /** Set false when an enclosing status region owns the announcement. */
  announce?: boolean;
}) {
  const label = activityLabel(state, toolName);
  const waiting = state === "approval" || state === "clarification";
  return (
    <div
      className={cn("inline-flex min-h-6 items-center gap-2 text-xs text-muted-foreground", className)}
      role={announce ? "status" : undefined}
      aria-live={announce ? "polite" : undefined}
    >
      <span className={cn("flex gap-1", waiting && "opacity-60")} aria-hidden="true">
        {[0, 1, 2].map((index) => (
          <span
            key={index}
            className="h-1.5 w-1.5 animate-pulse rounded-full bg-current motion-reduce:animate-none"
            style={{ animationDelay: `${index * 160}ms` }}
          />
        ))}
      </span>
      <span>{label}</span>
    </div>
  );
}
