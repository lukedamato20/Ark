import * as React from "react";
import type { ArkClient } from "./ArkClient";
import { ArkClientReactContext } from "./useArkClient";

/** Mounted once at the app root (see `main.tsx`) with the real Tauri-backed client; a component
 * test mounts its own instance with `createFakeArkClient(...)` instead. See `useArkClient.ts`
 * for `useArkClient()` and the underlying React context. */
export function ArkClientProvider({ client, children }: { client: ArkClient; children: React.ReactNode }) {
  return <ArkClientReactContext.Provider value={client}>{children}</ArkClientReactContext.Provider>;
}
