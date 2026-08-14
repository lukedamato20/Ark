import assert from "node:assert/strict";
import test from "node:test";

import {
  buildWorkspaceDiagnostics,
  getWorkspaceRecoveryActions,
  isKnownStorageRecoveryCode,
} from "./workspaceRecovery.ts";

const recoveryCodes = [
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
];

test("every typed startup storage failure exposes safe recovery actions", () => {
  for (const code of recoveryCodes) {
    assert.equal(isKnownStorageRecoveryCode(code), true, code);
    const actions = getWorkspaceRecoveryActions(code);
    assert.equal(actions.includes("retry") || actions.includes("choose-workspace"), true, code);
    assert.equal(actions.includes("copy-diagnostics"), true, code);
  }
});

test("workspace diagnostics whitelist technical fields and exclude transcript content", () => {
  const diagnostics = buildWorkspaceDiagnostics(
    { code: "database_corrupt", message: "not a database", transcript: "secret chat" } as never,
    {
      rootPath: "C:\\Ark",
      databasePath: "C:\\Ark\\ark.sqlite3",
      defaultRootPath: "C:\\Default",
      configPath: "C:\\Config\\workspace.json",
      isPortable: true,
      requiresRestart: false,
      messages: ["secret chat"],
    } as never,
    "2026-08-14T00:00:00.000Z",
  );

  assert.equal(diagnostics.includes("secret chat"), false);
  assert.deepEqual(JSON.parse(diagnostics), {
    recoveryCode: "database_corrupt",
    message: "not a database",
    workspaceRoot: "C:\\Ark",
    databasePath: "C:\\Ark\\ark.sqlite3",
    configPath: "C:\\Config\\workspace.json",
    capturedAt: "2026-08-14T00:00:00.000Z",
  });
});
