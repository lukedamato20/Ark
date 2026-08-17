import type { ProviderConfig } from "../types/ark";

export interface RemoteRequestDisclosure {
  endpoint: string;
  route: "POST /v1/chat/completions";
  model: string;
  contextItems: string[];
}

/**
 * FTR-007: the deterministic, pre-send disclosure for every non-loopback compatible provider.
 * Trust classification remains backend-authoritative; this helper only turns it into copy.
 */
export function buildRemoteRequestDisclosure(
  provider: ProviderConfig | undefined,
  model: string,
  attachmentCount: number,
  webSearchEnabled: boolean,
): RemoteRequestDisclosure | null {
  if (!provider || provider.destinationClass === "loopback") return null;

  const contextItems = [
    "current message",
    "active conversation history",
    "configured app/project/persona/conversation instructions",
  ];
  if (attachmentCount > 0) contextItems.push(`${attachmentCount} staged attachment(s)`);
  if (webSearchEnabled) contextItems.push("approved web-search query/results");

  return {
    endpoint: provider.baseUrl ?? "not configured",
    route: "POST /v1/chat/completions",
    model: model || "not selected",
    contextItems,
  };
}
