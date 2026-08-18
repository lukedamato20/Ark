import { AlertTriangle, CheckCircle2, Inbox, XCircle } from "lucide-react";
import * as React from "react";
import { cn } from "../lib/cn";
import { ActivityIndicator } from "./activityIndicator";

/**
 * UX-004: one reusable state-presentation family (loading/empty/success/warning/error), used
 * wherever the app needs to explain "what's happening and what can I do" rather than each
 * feature inventing its own layout. Contextual variants come from composing `title`/
 * `description`/`actions`, not from separate components per surface.
 */
export type StatePanelTone = "loading" | "empty" | "success" | "warning" | "error";

const TONE_ICON: Record<Exclude<StatePanelTone, "loading">, React.ComponentType<React.SVGProps<SVGSVGElement>>> = {
  empty: Inbox,
  success: CheckCircle2,
  warning: AlertTriangle,
  error: XCircle,
};

const TONE_ICON_CLASS: Record<StatePanelTone, string> = {
  loading: "text-muted-foreground",
  empty: "text-muted-foreground",
  success: "text-emerald-600 dark:text-emerald-400",
  warning: "text-amber-600 dark:text-amber-400",
  error: "text-destructive",
};

interface StatePanelProps {
  tone: StatePanelTone;
  title: string;
  description?: React.ReactNode;
  /** Technical detail (error code, raw message) kept out of the primary description so the
   * plain-language explanation stays first — rendered smaller and dimmer beneath it. */
  detail?: React.ReactNode;
  actions?: React.ReactNode;
  /** `"alert"` for warning/error states an assistive-tech user must be told about immediately;
   * omit for loading/empty/success states that don't warrant an interruption. */
  role?: "alert" | "status";
  className?: string;
}

export function StatePanel({ tone, title, description, detail, actions, role, className }: StatePanelProps) {
  const Icon = tone === "loading" ? null : TONE_ICON[tone];
  return (
    <div role={role} className={cn("flex flex-col items-center gap-3 px-6 text-center", className)}>
      {Icon ? (
        <Icon className={cn("h-8 w-8", TONE_ICON_CLASS[tone])} aria-hidden="true" />
      ) : (
        <ActivityIndicator state="preparing" />
      )}
      <div className="text-base font-semibold text-foreground">{title}</div>
      {description && <div className="max-w-md text-sm text-muted-foreground">{description}</div>}
      {detail && <div className="max-w-md text-xs text-muted-foreground/70">{detail}</div>}
      {actions && <div className="mt-1 flex flex-wrap items-center justify-center gap-2">{actions}</div>}
    </div>
  );
}
