import { readFileSync } from "node:fs";
import { join } from "node:path";

const root = join(import.meta.dirname, "..");
const read = (path) => readFileSync(join(root, path), "utf8");

function assert(condition, message) {
  if (!condition) throw new Error(`Secret-boundary check failed: ${message}`);
}

const secretStore = read("src-tauri/src/secret_store.rs");
const commands = read("src-tauri/src/commands/mod.rs");
const arkClient = read("src/lib/ArkClient.ts");
const settings = read("src/features/settings/SettingsView.tsx");
const diagnostics = read("src-tauri/src/diagnostics.rs");
const diagnosticsProduction = diagnostics.split("#[cfg(test)]", 1)[0];
const exportService = read("src-tauri/src/import_export.rs");
const sidecar = read("src-tauri/src/sidecar.rs");
const redaction = read("src-tauri/src/redaction.rs");
const observability = read("src-tauri/src/observability.rs");
const diagnosticsBundle = read("src-tauri/src/diagnostics_bundle.rs");
const productionSources = [
  commands,
  arkClient,
  settings,
  diagnostics,
  exportService,
  sidecar,
  redaction,
  observability,
  diagnosticsBundle,
].join("\n");

assert(secretStore.includes("pub struct SecretValue(String);"), "raw secrets must have a dedicated non-serializable type");
assert(!/derive\([^)]*(?:Debug|Serialize)[^)]*\)\s*pub struct SecretValue/.test(secretStore), "SecretValue must not implement Debug or Serialize");
assert(!commands.includes("read_provider_secret"), "IPC must not expose raw-secret reads");
assert(!commands.includes("read_companion_api_token"), "IPC must not expose raw companion API token reads");
assert(!commands.includes("read_tool_secret"), "IPC must not expose raw tool-secret reads");
assert(!arkClient.includes("getProviderSecret("), "ArkClient must not expose raw-secret reads");
assert(arkClient.includes("getProviderSecretMetadata("), "ArkClient must expose metadata-only reads");

const submitStart = settings.indexOf("async function saveSecret()");
const submitEnd = settings.indexOf("async function deleteSecret()", submitStart);
assert(submitStart >= 0 && submitEnd > submitStart, "credential submission handler must remain identifiable");
const submit = settings.slice(submitStart, submitEnd);
assert(
  submit.indexOf('setSecretDraft("")') >= 0 &&
    submit.indexOf('setSecretDraft("")') < submit.indexOf("await client.upsertProviderSecret"),
  "the UI must clear its credential field before awaiting persistence",
);
assert(!submit.includes("console."), "credential submission must not log values or errors directly");
assert(!settings.includes("navigator.clipboard"), "credential UI must not write credentials to the clipboard");
assert(!settings.includes("localStorage") || !submit.includes("localStorage"), "credential submission must not persist in browser storage");

for (const forbidden of ["SecretValue", "read_provider_secret", "api_key_ref"]) {
  assert(!diagnosticsProduction.includes(forbidden), `diagnostics must not access ${forbidden}`);
}
assert(exportService.includes("provider.api_key_ref = None;"), "portable JSON export must clear opaque credential references");
assert(
  exportService.includes("conversation_export_excludes_provider_secret_references_and_values"),
  "export exclusion must have a focused regression test",
);
// OPS-001: runtime-log redaction moved from sidecar.rs into the shared redaction.rs module
// (reused by observability.rs's structured log/crash capture) — the regression test moved with
// it, under a new name reflecting that it's no longer sidecar-specific.
assert(
  redaction.includes("redacts_known_paths_secrets_and_common_auth_shapes"),
  "runtime logging must retain its sensitive-value redaction regression test",
);
assert(sidecar.includes("use crate::redaction::redact"), "sidecar log redaction must go through the shared redaction module");

// OPS-001: Ark now has local-only, opt-in crash capture (never transmitted anywhere
// automatically — see observability.rs's module doc). The previous version of this guard only
// asserted crash reporting did not exist yet; now that it does, the guard must instead prove the
// payload is actually redacted before it is ever written, and that no network/remote transport
// exists for it.
assert(
  observability.includes("record_crash_directly_to_file") && observability.includes("redact(message, &[])"),
  "crash capture must redact its message before writing to the local log",
);
assert(
  observability.includes("crash_records_written_directly_to_file_are_redacted_and_readable_back"),
  "crash capture must retain its payload-redaction regression test",
);
assert(
  !/(?:sentry|reqwest::.*sentry|reportCrash)/i.test(productionSources),
  "crash capture must remain purely local — no third-party crash-report transport",
);
assert(
  diagnosticsBundle.includes("save_diagnostics_bundle") && !diagnosticsBundle.match(/reqwest|http_client|ureq/i),
  "diagnostics bundle export must remain a local file save, never an automatic network upload",
);

console.log("Secret boundary check passed: IPC, UI state, clipboard, exports, diagnostics, logs, and crash-report absence are guarded.");
