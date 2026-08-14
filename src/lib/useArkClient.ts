import * as React from "react";
import type { ArkClient } from "./ArkClient";

/**
 * ARC-002: the injection point that makes `useArkClient()` swappable. Production code never
 * needs to touch this directly — `main.tsx` mounts `<ArkClientProvider>` (see
 * `ArkClientContext.tsx`) once at the app root with the real Tauri-backed client. A component
 * test wraps the component under test in its own `<ArkClientProvider client={createFakeArkClient({...})}>`
 * instead, which is what "UI tests can substitute a fake ArkClient without global Tauri mocks"
 * means concretely: no module mocking, no `vi.mock("@tauri-apps/api/core")` — just a different
 * value passed to this context.
 *
 * Split into its own plain `.ts` module (no JSX) so that `useArkClient` — a hook, not a
 * component — doesn't share a file with the `ArkClientProvider` component; keeping component
 * and non-component exports in separate files is what Fast Refresh needs to hot-reload
 * `ArkClientProvider` without a full page reload.
 */
export const ArkClientReactContext = React.createContext<ArkClient | null>(null);

export function useArkClient(): ArkClient {
  const client = React.useContext(ArkClientReactContext);
  if (!client) {
    throw new Error("useArkClient() was called outside an <ArkClientProvider>.");
  }
  return client;
}
