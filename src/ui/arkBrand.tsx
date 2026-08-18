import markUrl from "../assets/brand/ark-mark.svg";
import { cn } from "../lib/cn";

interface ArkBrandProps {
  compact?: boolean;
  className?: string;
}

export function ArkBrand({ compact = false, className }: ArkBrandProps) {
  return (
    <span
      className={cn("inline-flex min-w-0 items-center gap-2 text-foreground", className)}
      role={compact ? "img" : undefined}
      aria-label={compact ? "Ark" : undefined}
    >
      <img src={markUrl} alt="" aria-hidden="true" className="h-6 w-6 shrink-0" />
      {!compact && <span className="truncate text-sm font-semibold tracking-wide">Ark</span>}
    </span>
  );
}
