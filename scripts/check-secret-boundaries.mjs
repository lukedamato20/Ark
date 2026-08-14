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
const productionSources = [commands, arkClient, settings, diagnostics, exportService, sidecar].join("\n");

assert(secretStore.includes("pub struct SecretValue(String);"), "raw secrets must have a dedicated non-serializable type");
assert(!/derive\([^)]*(?:Debug|Serialize)[^)]*\)\s*pub struct SecretValue/.test(secretStore), "SecretValue must not implement Debug or Serialize");
assert(!commands.includes("read_provider_secret"), "IPC must not expose raw-secret reads");
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
assert(
  sidecar.includes("log_redaction_covers_known_paths_secrets_and_common_auth_shapes"),
  "runtime logging must retain its sensitive-value redaction regression test",
);

// Ark has no crash-report transport yet. This fail-closed guard forces OPS-001 to replace the
// absence assertion with payload-level redaction tests before adding one.
assert(
  !/(?:sentry|crash[_A-Z]?report|reportCrash)/i.test(productionSources),
  "a crash-report surface was added without replacing the SEC-005 absence guard with redaction tests",
);

console.log("Secret boundary check passed: IPC, UI state, clipboard, exports, diagnostics, logs, and crash-report absence are guarded.");
