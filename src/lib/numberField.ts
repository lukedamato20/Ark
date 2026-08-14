/**
 * UX-005: pure validation for the numeric settings fields (temperature, max tokens) — kept
 * separate from the DOM/React wiring in `ui/numberField.tsx` so the range/parse logic is
 * unit-testable without a DOM. Deliberately never coerces an invalid draft to `NaN` or a
 * fallback number: the caller keeps showing exactly what the user typed, with `valid: false`
 * driving the inline error, until it parses to a finite number inside `[min, max]`.
 */
export interface NumberFieldValidation {
  valid: boolean;
  parsed: number | null;
  error: string | null;
}

export function validateNumberInput(value: string, min: number, max: number, label: string): NumberFieldValidation {
  const trimmed = value.trim();
  if (trimmed === "") {
    return { valid: false, parsed: null, error: `${label} is required.` };
  }

  const parsed = Number(trimmed);
  if (!Number.isFinite(parsed)) {
    return { valid: false, parsed: null, error: `${label} must be a number.` };
  }
  if (parsed < min || parsed > max) {
    return { valid: false, parsed: null, error: `${label} must be between ${min} and ${max}.` };
  }

  return { valid: true, parsed, error: null };
}
