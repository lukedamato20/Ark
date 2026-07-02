import * as React from "react";
import { cn } from "../lib/cn";

export function Panel({ className, ...props }: React.HTMLAttributes<HTMLDivElement>) {
  return <div className={cn("rounded-lg border border-border bg-card text-card-foreground", className)} {...props} />;
}
