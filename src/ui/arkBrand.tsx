import markUrl from "../assets/brand/ark-mark.svg";
import { cn } from "../lib/cn";

interface ArkBrandProps {
  compact?: boolean;
  className?: string;
}

export function ArkBrand({ compact: _compact = false, className }: ArkBrandProps) {
  return (
    <span className={cn("inline-flex min-w-0 items-center text-foreground", className)} role="img" aria-label="Ark">
      <img src={markUrl} alt="" aria-hidden="true" className="h-6 w-6 shrink-0" />
    </span>
  );
}
