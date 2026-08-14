/**
 * Stable frontend error envelope. Tauri serializes `AppError` objects, but transport/runtime
 * failures may still reject with a string or an arbitrary value.
 */
export interface ArkError {
  code: string;
  message: string;
}

export function normalizeError(error: unknown): ArkError {
  if (error && typeof error === "object" && "message" in error) {
    const shaped = error as { code?: unknown; message?: unknown };
    return {
      code: typeof shaped.code === "string" ? shaped.code : "unknown_error",
      message: typeof shaped.message === "string" ? shaped.message : "Unexpected Ark error.",
    };
  }
  if (typeof error === "string") {
    return { code: "unknown_error", message: error };
  }
  return { code: "unknown_error", message: "Unexpected Ark error." };
}

export function getErrorMessage(error: unknown): string {
  return normalizeError(error).message;
}
