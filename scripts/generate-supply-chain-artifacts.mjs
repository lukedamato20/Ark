import { createHash } from "node:crypto";
import { spawnSync } from "node:child_process";
import { lstat, mkdir, readFile, readdir, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const sbomPath = path.join(root, "artifacts", "sbom.cdx.json");
const noticesPath = path.join(root, "THIRD_PARTY_NOTICES.md");

function commandJson(command, args, cwd) {
  const result = spawnSync(command, args, {
    cwd,
    encoding: "utf8",
    maxBuffer: 128 * 1024 * 1024,
    windowsHide: true,
  });
  if (result.error) throw result.error;
  if (result.status !== 0) throw new Error(`${command} failed: ${result.stderr}`);
  return JSON.parse(result.stdout);
}

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

function licenseExpression(value) {
  if (!value) return [{ license: { name: "NOASSERTION" } }];
  return [{ expression: Array.isArray(value) ? value.join(" OR ") : String(value) }];
}

async function rustComponents() {
  const metadata = commandJson("cargo", ["metadata", "--locked", "--format-version", "1"], path.join(root, "src-tauri"));
  return metadata.packages
    .filter((pkg) => pkg.name !== "ark")
    .map((pkg) => ({
      type: "library",
      "bom-ref": `pkg:cargo/${pkg.name}@${pkg.version}`,
      name: pkg.name,
      version: pkg.version,
      purl: `pkg:cargo/${pkg.name}@${pkg.version}`,
      licenses: licenseExpression(pkg.license),
      externalReferences: pkg.repository ? [{ type: "vcs", url: pkg.repository }] : undefined,
      properties: [{ name: "ark:ecosystem", value: "cargo" }],
    }));
}

async function npmPackageDirectories() {
  const store = path.join(root, "node_modules", ".pnpm");
  const directories = [];
  for (const entry of await readdir(store, { withFileTypes: true })) {
    if (!entry.isDirectory()) continue;
    const modules = path.join(store, entry.name, "node_modules");
    let children;
    try {
      children = await readdir(modules, { withFileTypes: true });
    } catch (error) {
      if (error.code === "ENOENT") continue;
      throw error;
    }
    for (const child of children) {
      if (child.name.startsWith("@") && child.isDirectory()) {
        for (const scoped of await readdir(path.join(modules, child.name), { withFileTypes: true })) {
          if (scoped.isDirectory() || scoped.isSymbolicLink()) {
            directories.push(path.join(modules, child.name, scoped.name));
          }
        }
      } else if (child.isDirectory() || child.isSymbolicLink()) {
        directories.push(path.join(modules, child.name));
      }
    }
  }
  return directories;
}

async function npmComponents() {
  const packages = new Map();
  for (const directory of await npmPackageDirectories()) {
    try {
      const info = JSON.parse(await readFile(path.join(directory, "package.json"), "utf8"));
      if (!info.name || !info.version || info.name === "ark") continue;
      packages.set(`${info.name}@${info.version}`, info);
    } catch (error) {
      if (error.code !== "ENOENT") throw error;
    }
  }
  return [...packages.values()].map((pkg) => {
    const purlName = pkg.name.startsWith("@") ? `%40${pkg.name.slice(1)}` : pkg.name;
    const repository = typeof pkg.repository === "string" ? pkg.repository : pkg.repository?.url;
    return {
      type: "library",
      "bom-ref": `pkg:npm/${purlName}@${pkg.version}`,
      name: pkg.name,
      version: pkg.version,
      purl: `pkg:npm/${purlName}@${pkg.version}`,
      licenses: licenseExpression(pkg.license ?? pkg.licenses?.map((item) => item.type)),
      externalReferences: repository ? [{ type: "vcs", url: repository.replace(/^git\+/u, "") }] : undefined,
      properties: [{ name: "ark:ecosystem", value: "npm" }],
    };
  });
}

async function nativeComponents() {
  const manifest = JSON.parse(await readFile(path.join(root, "config", "native-artifacts.json"), "utf8"));
  return manifest.artifacts.map((artifact) => ({
    type: "file",
    "bom-ref": `native:llama.cpp:${manifest.runtime.version}:${artifact.platform}:${artifact.arch}`,
    name: artifact.fileName,
    version: manifest.runtime.version,
    hashes: [{ alg: "SHA-256", content: artifact.sha256 }],
    licenses: licenseExpression(manifest.runtime.license),
    externalReferences: [
      { type: "distribution", url: artifact.url },
      { type: "vcs", url: `${manifest.runtime.sourceRepository}/tree/${manifest.runtime.sourceCommit}` },
      { type: "license", url: manifest.runtime.licenseUrl },
    ],
    properties: [
      { name: "ark:ecosystem", value: "native-runtime" },
      { name: "ark:platform", value: artifact.platform },
      { name: "ark:architecture", value: artifact.arch },
      { name: "ark:sizeBytes", value: String(artifact.sizeBytes) },
    ],
  }));
}

async function bundledAssetComponents() {
  const candidates = [path.join(root, "src-tauri", "icons", "icon.ico")];
  const components = [];
  for (const filePath of candidates) {
    const info = await lstat(filePath);
    if (!info.isFile()) throw new Error(`Configured bundled asset is not a regular file: ${filePath}`);
    const bytes = await readFile(filePath);
    components.push({
      type: "file",
      "bom-ref": `asset:${path.relative(root, filePath).replaceAll("\\", "/")}`,
      name: path.relative(root, filePath).replaceAll("\\", "/"),
      hashes: [{ alg: "SHA-256", content: sha256(bytes) }],
      licenses: licenseExpression("LicenseRef-Ark-Project"),
      properties: [{ name: "ark:ecosystem", value: "bundled-asset" }],
    });
  }
  return components;
}

function noticeRows(components) {
  return components
    .filter((component) => component.properties?.some((item) => item.value === "cargo" || item.value === "npm"))
    .map((component) => ({
      ecosystem: component.properties[0].value,
      name: component.name,
      version: component.version,
      license: component.licenses[0].expression ?? component.licenses[0].license.name,
    }))
    .sort((left, right) =>
      left.ecosystem.localeCompare(right.ecosystem) ||
      left.name.localeCompare(right.name) ||
      left.version.localeCompare(right.version),
    );
}

export async function generate() {
  const components = [
    ...(await rustComponents()),
    ...(await npmComponents()),
    ...(await nativeComponents()),
    ...(await bundledAssetComponents()),
  ].sort((left, right) => left["bom-ref"].localeCompare(right["bom-ref"]));
  const sbom = {
    bomFormat: "CycloneDX",
    specVersion: "1.5",
    version: 1,
    metadata: {
      tools: [{ vendor: "Ark", name: "generate-supply-chain-artifacts.mjs", version: "1" }],
      component: { type: "application", name: "Ark", version: "0.1.0", "bom-ref": "pkg:generic/ark@0.1.0" },
      properties: [
        { name: "ark:scope", value: "Rust, JavaScript, reviewed native runtime artifacts, bundled assets" },
        { name: "ark:reproducible", value: "true" },
      ],
    },
    components,
  };

  const rows = noticeRows(components);
  const notices = [
    "# Third-party notices",
    "",
    "Generated from the locked Cargo/npm dependency trees and `config/native-artifacts.json`.",
    "Regenerate with `pnpm supply-chain:generate`; CI verifies this file and the CycloneDX SBOM.",
    "",
    "The optional llama.cpp development runtime is MIT licensed. Its exact source commit, release",
    "artifact hashes, license URL, and license hash are recorded in `config/native-artifacts.json`.",
    "",
    "| Ecosystem | Package | Version | Declared license |",
    "|---|---|---:|---|",
    ...rows.map((row) => `| ${row.ecosystem} | \`${row.name}\` | ${row.version} | ${row.license} |`),
    "",
    "`NOASSERTION` means upstream package metadata does not declare a machine-readable license; it",
    "must be reviewed before distribution rather than assumed to be permissive.",
    "",
  ].join("\n");
  return { sbom: `${JSON.stringify(sbom, null, 2)}\n`, notices };
}

async function main() {
  const generated = await generate();
  if (process.argv.includes("--check")) {
    const [existingSbom, existingNotices] = await Promise.all([readFile(sbomPath, "utf8"), readFile(noticesPath, "utf8")]);
    if (existingSbom !== generated.sbom || existingNotices !== generated.notices) {
      throw new Error("Supply-chain artifacts are stale; run pnpm supply-chain:generate.");
    }
    console.log(`Supply-chain artifacts are current (${JSON.parse(generated.sbom).components.length} components).`);
    return;
  }
  await mkdir(path.dirname(sbomPath), { recursive: true });
  await Promise.all([writeFile(sbomPath, generated.sbom), writeFile(noticesPath, generated.notices)]);
  console.log(`Wrote ${path.relative(root, sbomPath)} and ${path.relative(root, noticesPath)}.`);
}

main().catch((error) => {
  console.error(error.message);
  process.exitCode = 1;
});
