import { Cloud, Monitor, Network } from "lucide-react";
import type { ComponentType, SVGProps } from "react";
import type { DestinationClass } from "../types/ark";

/**
 * SEC-001: classification comes from the backend (`ProviderConfig.destinationClass`, computed in
 * Rust by `security::classify_destination`) rather than being re-derived here — the frontend must
 * never be the source of truth for a privacy-relevant trust boundary. Extracted from
 * `ChatView.tsx` (UX-011) so `ChatMessageList.tsx`'s per-message metadata disclosure uses the
 * exact same icon/label/tone/description as the header's provider indicator, rather than a second
 * copy that could drift out of sync with it.
 */
export const CONNECTION_METADATA: Record<
  DestinationClass,
  { icon: ComponentType<SVGProps<SVGSVGElement>>; label: string; tone: string; description: string }
> = {
  loopback: {
    icon: Monitor,
    label: "local",
    tone: "text-emerald-600 dark:text-emerald-300",
    description:
      "Running locally on this device. User prompts, conversation history, and the configured system prompt do not leave this computer.",
  },
  private_lan: {
    icon: Network,
    label: "network",
    tone: "text-sky-600 dark:text-sky-300",
    description:
      "Connecting to a server on your local network. User prompts, conversation history, and the configured system prompt leave this device but stay within your network.",
  },
  public: {
    icon: Cloud,
    label: "cloud",
    tone: "text-amber-600 dark:text-amber-300",
    description:
      "Connecting to a remote server outside your network. User prompts, conversation history, and the configured system prompt are sent to this destination.",
  },
};
