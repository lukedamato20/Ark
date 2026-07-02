import * as React from "react";
import { cn } from "../lib/cn";

export function Badge({
  className,
  tone = "muted",
  ...props
}: React.HTMLAttributes<HTMLSpanElement> & { tone?: "muted" | "success" | "warning" | "danger" }) {
  const tones = {
    muted: "border-border bg-muted text-muted-foreground",
    success: "border-emerald-500/25 bg-emerald-500/10 text-emerald-600 dark:text-emerald-300",
    warning: "border-amber-500/25 bg-amber-500/10 text-amber-600 dark:text-amber-300",
    danger: "border-destructive/30 bg-destructive/10 text-destructive",
  };

  return (
    <span
      className={cn(
        "inline-flex items-center rounded-md border px-2 py-0.5 text-xs font-medium",
        tones[tone],
        className,
      )}
      {...props}
    />
  );
}
