export const GENERATION_SETTING_SOURCES = [
  "request",
  "conversation",
  "persona",
  "project",
  "application",
  "provider_default",
] as const;

export type GenerationSettingSource = (typeof GENERATION_SETTING_SOURCES)[number];

export interface GenerationProvenanceView {
  projectId?: string | null;
  personaId?: string | null;
  personaVersion?: number | null;
  temperature?: number | null;
  temperatureSource?: GenerationSettingSource | null;
  maxTokens?: number | null;
  maxTokensSource?: GenerationSettingSource | null;
  systemPromptSource?: GenerationSettingSource | null;
  responseStyle?: string | null;
  responseStyleSource?: GenerationSettingSource | null;
  tone?: string | null;
  toneSource?: GenerationSettingSource | null;
}

const SETTING_SOURCE_SET = new Set<string>(GENERATION_SETTING_SOURCES);

function nullableString(value: unknown): string | null | undefined {
  return value === null ? null : typeof value === "string" ? value : undefined;
}

function nullableFiniteNumber(value: unknown): number | null | undefined {
  return value === null ? null : typeof value === "number" && Number.isFinite(value) ? value : undefined;
}

function nullableInteger(value: unknown): number | null | undefined {
  const parsed = nullableFiniteNumber(value);
  return parsed === null || (parsed !== undefined && Number.isInteger(parsed)) ? parsed : undefined;
}

function nullableSource(value: unknown): GenerationSettingSource | null | undefined {
  if (value === null) return null;
  return typeof value === "string" && SETTING_SOURCE_SET.has(value) ? (value as GenerationSettingSource) : undefined;
}

/**
 * FTR-005: imported message metadata is untrusted JSON. Parse only the bounded, known scalar
 * generation-provenance fields the comparison UI displays; unknown/malformed fields are ignored
 * and never reach rendering as structured claims.
 */
export function parseGenerationProvenance(metadataJson?: string | null): GenerationProvenanceView | null {
  if (!metadataJson) return null;
  try {
    const raw: unknown = JSON.parse(metadataJson);
    if (!raw || typeof raw !== "object" || Array.isArray(raw)) return null;
    const record = raw as Record<string, unknown>;
    const parsed: GenerationProvenanceView = {
      projectId: nullableString(record.projectId),
      personaId: nullableString(record.personaId),
      personaVersion: nullableInteger(record.personaVersion),
      temperature: nullableFiniteNumber(record.temperature),
      temperatureSource: nullableSource(record.temperatureSource),
      maxTokens: nullableInteger(record.maxTokens),
      maxTokensSource: nullableSource(record.maxTokensSource),
      systemPromptSource: nullableSource(record.systemPromptSource),
      responseStyle: nullableString(record.responseStyle),
      responseStyleSource: nullableSource(record.responseStyleSource),
      tone: nullableString(record.tone),
      toneSource: nullableSource(record.toneSource),
    };
    return Object.values(parsed).some((value) => value !== undefined) ? parsed : null;
  } catch {
    return null;
  }
}

export function formatGenerationSettingSource(source?: GenerationSettingSource | null): string | null {
  if (!source) return null;
  return source === "provider_default" ? "provider default" : source;
}
