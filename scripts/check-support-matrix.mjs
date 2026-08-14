import fs from "node:fs";
import path from "node:path";
import process from "node:process";

const root = process.cwd();
const matrix = JSON.parse(fs.readFileSync(path.join(root, "config/release-capabilities.json"), "utf8"));
const tauri = JSON.parse(fs.readFileSync(path.join(root, "src-tauri/tauri.conf.json"), "utf8"));
const workflow = fs.readFileSync(path.join(root, ".github/workflows/ci.yml"), "utf8");
const readme = fs.readFileSync(path.join(root, "README.md"), "utf8");
const supportDocument = fs.readFileSync(path.join(root, "docs/support-matrix.md"), "utf8");
const frontendGate = fs.readFileSync(path.join(root, "src/config/releaseCapabilities.ts"), "utf8");
const failures = [];

if (!Number.isInteger(matrix.schemaVersion) || matrix.schemaVersion < 1) {
  failures.push("schemaVersion must be a positive integer");
}
if (!/^[a-z0-9-]+$/.test(matrix.capabilitySet)) {
  failures.push("capabilitySet must be a stable lowercase identifier");
}
const window = tauri.app?.windows?.[0];
if (window?.minWidth !== matrix.window.minimumWidth || window?.minHeight !== matrix.window.minimumHeight) {
  failures.push("Tauri minimum window size does not match the release capability matrix");
}
for (const platform of matrix.artifactPlatforms) {
  if (!workflow.includes(platform.runner)) {
    failures.push(`CI does not contain required runner ${platform.runner} for ${platform.id}`);
  }
  if (!supportDocument.includes(platform.minimumVersionClaim)) {
    failures.push(`support matrix document omits ${platform.minimumVersionClaim}`);
  }
}
for (const [providerType, claim] of Object.entries(matrix.providers)) {
  if (claim.visible && !frontendGate.includes("providerIsVisible")) {
    failures.push(`visible provider ${providerType} has no frontend visibility gate`);
  }
  const documentedName = providerType === "built_in" ? "Built-in" : providerType.replaceAll("_", " ");
  if (!supportDocument.toLowerCase().includes(documentedName.toLowerCase())) {
    failures.push(`support matrix document omits provider ${providerType}`);
  }
}
if (!readme.includes("docs/support-matrix.md")) {
  failures.push("README does not link the support matrix");
}
if (!supportDocument.includes(matrix.capabilitySet)) {
  failures.push("support matrix document does not identify the configured capability set");
}

if (failures.length > 0) {
  console.error(failures.map((failure) => `- ${failure}`).join("\n"));
  process.exitCode = 1;
} else {
  console.log(
    `Support matrix passed: ${matrix.capabilitySet}, schema ${matrix.schemaVersion}, review=${matrix.reviewStatus}.`,
  );
}
