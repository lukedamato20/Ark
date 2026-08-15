import type { ResponseStyle, Tone } from "../types/ark";

/**
 * UX: the single source of truth for the response-style/tone dropdown options — imported by
 * every consumer (`ConversationSettingsButton`, `PersonaEditor`, `ProjectEditor`) so the label
 * text and the exact allowed values can never drift between them. Mirrors the Rust-side allow-list
 * (`validation::RESPONSE_STYLE_VALUES`/`TONE_VALUES`) exactly — kept in sync manually since the
 * two languages have no shared enum, the same tradeoff the existing contract-fixture system
 * already accepts for closed string-enum fields (see `docs/protocol-versioning.md`'s "known
 * gaps").
 */
export interface GenerationPresetOption<T extends string> {
  value: T;
  label: string;
}

export const RESPONSE_STYLE_OPTIONS: GenerationPresetOption<ResponseStyle>[] = [
  { value: "balanced", label: "Balanced" },
  { value: "concise", label: "Concise" },
  { value: "detailed", label: "Detailed" },
  { value: "explanatory", label: "Explanatory" },
  { value: "technical", label: "Technical" },
  { value: "creative", label: "Creative" },
];

export const TONE_OPTIONS: GenerationPresetOption<Tone>[] = [
  { value: "neutral", label: "Neutral" },
  { value: "professional", label: "Professional" },
  { value: "friendly", label: "Friendly" },
  { value: "direct", label: "Direct" },
  { value: "casual", label: "Casual" },
];
