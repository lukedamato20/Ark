import matrix from "../../config/release-capabilities.json";

export type ProviderReleaseMode = "ollama" | "local_inference_host" | "built_in" | "openai";

/**
 * FND-001: the typed frontend view of the versioned artifact claim. The JSON file is also
 * validated against Tauri configuration, documentation, and CI by `pnpm support:check`.
 */
export const releaseCapabilities = matrix;

export function providerIsVisible(providerType: string): boolean {
  if (!(providerType in releaseCapabilities.providers)) return false;
  const claim = releaseCapabilities.providers[providerType as ProviderReleaseMode];
  if (claim.visible) return true;
  return (
    providerType === "built_in" && import.meta.env.DEV && "developmentVisible" in claim && claim.developmentVisible
  );
}
