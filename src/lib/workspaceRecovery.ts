import type { AppErrorShape, WorkspaceInfo } from "../types/ark";

export type WorkspaceRecoveryAction = "retry" | "choose-workspace" | "copy-diagnostics";

const STORAGE_RECOVERY_CODES = new Set([
  "database_corrupt",
  "database_schema_too_new",
  "database_migration_failed",
  "database_migration_gap",
  "database_migration_checksum_mismatch",
  "database_locked",
  "workspace_missing",
  "workspace_read_only",
  "disk_full",
  "workspace_change_interrupted",
]);

/** Every startup storage failure receives conservative, non-destructive actions. */
export function getWorkspaceRecoveryActions(_code: string): readonly WorkspaceRecoveryAction[] {
  return ["retry", "choose-workspace", "copy-diagnostics"];
}

export function isKnownStorageRecoveryCode(code: string): boolean {
  return STORAGE_RECOVERY_CODES.has(code);
}

/** Builds a whitelist-only payload, so runtime extra properties can never leak chat content. */
export function buildWorkspaceDiagnostics(
  error: AppErrorShape,
  workspace: WorkspaceInfo | null,
  capturedAt: string,
): string {
  return JSON.stringify(
    {
      recoveryCode: error.code ?? "database_error",
      message: error.message ?? "Ark could not open the workspace database.",
      workspaceRoot: workspace?.rootPath ?? null,
      databasePath: workspace?.databasePath ?? null,
      configPath: workspace?.configPath ?? null,
      capturedAt,
    },
    null,
    2,
  );
}

/**
 * UX-004: the total-bootstrap-failure counterpart to `buildWorkspaceDiagnostics` above — no
 * `WorkspaceInfo` to include, since a failure this early means nothing (workspace included) has
 * loaded yet. Same whitelist-only shape and reasoning: never include chat content.
 */
export function buildBootstrapDiagnostics(error: AppErrorShape, capturedAt: string): string {
  return JSON.stringify(
    {
      recoveryCode: error.code ?? "bootstrap_error",
      message: error.message ?? "Ark could not start up.",
      capturedAt,
    },
    null,
    2,
  );
}
