import * as React from "react";
import { validateNumberInput } from "../lib/numberField";
import { cn } from "../lib/cn";
import { Input } from "./input";

interface NumberFieldProps {
  id: string;
  label: string;
  value: string;
  onChange: (value: string) => void;
  min: number;
  max: number;
  /** Shown under the field as constraint guidance — e.g. "Controls response randomness." The
   * numeric range itself is appended automatically, so callers describe *what* the field does,
   * not its bounds. */
  help?: string;
  disabled?: boolean;
}

/**
 * UX-005: a numeric settings field that never lets an invalid intermediate edit collapse to
 * `NaN` — the input stays a plain text field (not `type="number"`, whose own blur-to-empty/NaN
 * behavior is exactly what this task's acceptance criteria calls out) bound to the raw string
 * draft the caller owns, with validation timing on every keystroke rather than only on blur or
 * submit: since this always edits an existing, already-valid server value, showing the error the
 * moment it stops being true (e.g. the field is cleared) is more useful here than deferring it.
 */
export function NumberField({ id, label, value, onChange, min, max, help, disabled }: NumberFieldProps) {
  const validation = validateNumberInput(value, min, max, label);
  const helpId = `${id}-help`;
  const errorId = `${id}-error`;

  return (
    <div className="grid gap-1.5 text-sm">
      <label htmlFor={id}>{label}</label>
      <Input
        id={id}
        value={value}
        onChange={(event) => onChange(event.target.value)}
        inputMode="decimal"
        disabled={disabled}
        aria-invalid={!validation.valid}
        aria-describedby={cn(helpId, !validation.valid && errorId)}
        className={cn(!validation.valid && "border-destructive focus-visible:ring-destructive")}
      />
      <span id={helpId} className="text-xs text-muted-foreground">
        {help ? `${help} ` : ""}Range: {min}–{max}.
      </span>
      {!validation.valid && validation.error && (
        <span id={errorId} role="alert" className="text-xs text-destructive">
          {validation.error}
        </span>
      )}
    </div>
  );
}
