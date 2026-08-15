import * as React from "react";
import { getErrorMessage } from "./arkErrors";
import type { SideEffectPreview } from "../types/ark";

/** CMP-004: extracted from `ConversationNotesButton`'s private `attemptWrite` helper (CMP-003) so
 * the composer's web-search flow can reuse the same "attempt, catch `approval_required`, preview,
 * let the user approve, retry" shape instead of duplicating it. Unlike the original, `attempt`
 * resolves with the eventual result (or `undefined` on cancel/failure) rather than returning
 * `void` — a caller like the composer needs the actual search results back before it can proceed
 * with sending, not just a side effect. */
export interface PendingToolApproval {
  preview: SideEffectPreview;
}

export interface ToolApproval {
  pendingApproval: PendingToolApproval | null;
  busy: boolean;
  /** Runs `run(false)` first; if the backend reports `approval_required`, fetches `preview()` and
   * waits for the user to call `approve()`/`cancel()` before resolving. Any other failure is
   * reported via `onError` and resolves `undefined`. */
  attempt<T>(
    run: (approve: boolean) => Promise<T>,
    preview: () => Promise<SideEffectPreview>,
    onError: (message: string) => void,
  ): Promise<T | undefined>;
  approve(): void;
  cancel(): void;
}

export function useToolApproval(): ToolApproval {
  const [pendingApproval, setPendingApproval] = React.useState<PendingToolApproval | null>(null);
  const [busy, setBusy] = React.useState(false);
  const pendingRef = React.useRef<{
    run: () => Promise<unknown>;
    resolve: (value: unknown) => void;
    onError: (message: string) => void;
  } | null>(null);

  const attempt = React.useCallback(
    async <T>(
      run: (approve: boolean) => Promise<T>,
      preview: () => Promise<SideEffectPreview>,
      onError: (message: string) => void,
    ): Promise<T | undefined> => {
      setBusy(true);
      try {
        const result = await run(false);
        return result;
      } catch (error) {
        const code =
          error && typeof error === "object" && "code" in error ? (error as { code?: string }).code : undefined;
        if (code !== "approval_required") {
          onError(getErrorMessage(error));
          return undefined;
        }
      } finally {
        setBusy(false);
      }

      try {
        const shown = await preview();
        return new Promise<T | undefined>((resolve) => {
          pendingRef.current = {
            run: () => run(true),
            resolve: resolve as (value: unknown) => void,
            onError,
          };
          setPendingApproval({ preview: shown });
        });
      } catch (previewError) {
        onError(getErrorMessage(previewError));
        return undefined;
      }
    },
    [],
  );

  const approve = React.useCallback(() => {
    const current = pendingRef.current;
    if (!current) return;
    pendingRef.current = null;
    setPendingApproval(null);
    setBusy(true);
    void (async () => {
      try {
        const result = await current.run();
        current.resolve(result);
      } catch (error) {
        current.onError(getErrorMessage(error));
        current.resolve(undefined);
      } finally {
        setBusy(false);
      }
    })();
  }, []);

  const cancel = React.useCallback(() => {
    const current = pendingRef.current;
    pendingRef.current = null;
    setPendingApproval(null);
    current?.resolve(undefined);
  }, []);

  return { pendingApproval, busy, attempt, approve, cancel };
}
