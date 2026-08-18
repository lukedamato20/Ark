import type { ModelInfo, ProviderConfig } from "../types/ark";

export type MetadataSourceKind = "provider" | "ark_reviewed" | "ark_derived" | "unavailable";
export type HardwareFitCategory = "excellent" | "good" | "constrained" | "not_recommended" | "unknown";
export type HardwareFitConfidence = "low" | "insufficient";
export type ModelAction = "delete" | "unload";

export interface ModelFactSources {
  sizeBytes: MetadataSourceKind;
  family: MetadataSourceKind;
  parameterSize: MetadataSourceKind;
  quantization: MetadataSourceKind;
  contextWindow: MetadataSourceKind;
  licenseSummary: MetadataSourceKind;
}

export interface ModelPresentation {
  schemaVersion: 1;
  displayName: string;
  providerId: string;
  providerName: string;
  runtime: string;
  available: boolean;
  capabilities: {
    streaming: boolean;
    tools: boolean;
    vision: boolean;
    embeddings: boolean;
  };
  supportedActions: ModelAction[];
  sizeBytes: number | null;
  family: string | null;
  parameterSize: string | null;
  quantization: string | null;
  contextWindow: number | null;
  licenseSummary: string | null;
  source: MetadataSourceKind;
  sourceLabel: string;
  metadataConfidence: "reported" | "unavailable";
  fieldSources: ModelFactSources;
  sourceUrl: string | null;
  reviewedAt: string | null;
  fit: HardwareFitCategory;
  fitReason: string;
  fitConfidence: HardwareFitConfidence;
  fitMethodVersion: "ark-fit-v1";
}

const MAX_METADATA_BYTES = 128 * 1024;

export function presentModel(
  model: ModelInfo,
  provider: ProviderConfig,
  availableMemoryBytes?: number | null,
): ModelPresentation {
  let raw: Record<string, unknown> | null = null;
  if (model.metadataJson && model.metadataJson.length <= MAX_METADATA_BYTES) {
    try {
      const parsed: unknown = JSON.parse(model.metadataJson);
      raw = parsed && typeof parsed === "object" && !Array.isArray(parsed) ? (parsed as Record<string, unknown>) : null;
    } catch {
      raw = null;
    }
  }
  const details = raw?.details && typeof raw.details === "object" ? (raw.details as Record<string, unknown>) : null;
  const arkShow = raw?.arkShow && typeof raw.arkShow === "object" ? (raw.arkShow as Record<string, unknown>) : null;
  const sizeBytes = finitePositive(raw?.size);
  const contextWindow = finitePositive(model.contextWindow) ?? finitePositive(arkShow?.contextWindow) ?? null;
  const remoteExecution = provider.destinationClass !== "loopback";
  const fit = assessHardwareFit(sizeBytes, availableMemoryBytes ?? null, remoteExecution);
  const providerSource = raw ? "provider" : "unavailable";

  return {
    schemaVersion: 1,
    displayName: model.displayName ?? model.name,
    providerId: model.providerId,
    providerName: provider.name,
    runtime: provider.providerType,
    available: model.isAvailable,
    capabilities: {
      streaming: model.supportsStreaming,
      tools: model.supportsTools,
      vision: model.supportsVision,
      embeddings: model.supportsEmbeddings,
    },
    supportedActions: [
      ...(provider.capabilities.modelDelete && model.isAvailable ? (["delete"] as const) : []),
      ...(provider.capabilities.modelUnload && model.isAvailable ? (["unload"] as const) : []),
    ],
    sizeBytes,
    family: boundedString(details?.family),
    parameterSize: boundedString(details?.parameter_size),
    quantization: boundedString(details?.quantization_level),
    contextWindow,
    licenseSummary: boundedString(arkShow?.licenseSummary),
    source: providerSource,
    sourceLabel: raw ? `Provided by ${provider.name}` : "Metadata unavailable",
    metadataConfidence: raw ? "reported" : "unavailable",
    fieldSources: {
      sizeBytes: sizeBytes ? providerSource : "unavailable",
      family: boundedString(details?.family) ? providerSource : "unavailable",
      parameterSize: boundedString(details?.parameter_size) ? providerSource : "unavailable",
      quantization: boundedString(details?.quantization_level) ? providerSource : "unavailable",
      contextWindow: contextWindow ? "provider" : "unavailable",
      licenseSummary: boundedString(arkShow?.licenseSummary) ? providerSource : "unavailable",
    },
    sourceUrl: null,
    reviewedAt: null,
    fit: fit.category,
    fitReason: fit.reason,
    fitConfidence: fit.confidence,
    fitMethodVersion: "ark-fit-v1",
  };
}

export function assessHardwareFit(
  modelBytes: number | null,
  availableMemoryBytes: number | null,
  remoteExecution: boolean,
) {
  if (remoteExecution)
    return {
      category: "unknown" as const,
      reason: "Execution occurs on another device; local hardware is not evidence.",
      confidence: "insufficient" as const,
    };
  if (!modelBytes || !availableMemoryBytes)
    return {
      category: "unknown" as const,
      reason: "Ark cannot yet measure enough execution memory for this model.",
      confidence: "insufficient" as const,
    };
  const ratio = availableMemoryBytes / modelBytes;
  if (ratio >= 3)
    return {
      category: "excellent" as const,
      reason: "Available memory is at least three times the model download size.",
      confidence: "low" as const,
    };
  if (ratio >= 2)
    return {
      category: "good" as const,
      reason: "Available memory is at least twice the model download size.",
      confidence: "low" as const,
    };
  if (ratio >= 1.25)
    return {
      category: "constrained" as const,
      reason: "The model may fit, but runtime and context memory leave limited headroom.",
      confidence: "low" as const,
    };
  return {
    category: "not_recommended" as const,
    reason: "Available memory is below the model download size plus runtime overhead.",
    confidence: "low" as const,
  };
}

function finitePositive(value: unknown): number | null {
  return typeof value === "number" && Number.isFinite(value) && value > 0 ? value : null;
}

function boundedString(value: unknown): string | null {
  return typeof value === "string" && value.length > 0 && value.length <= 256 ? value : null;
}
