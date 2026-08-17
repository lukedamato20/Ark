import { createHash, randomUUID } from "node:crypto";
import { spawn, spawnSync } from "node:child_process";
import {
  chmod,
  copyFile,
  lstat,
  mkdir,
  mkdtemp,
  open,
  readFile,
  readdir,
  rename,
  rm,
  stat,
  writeFile,
} from "node:fs/promises";
import { constants as fsConstants } from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const manifestPath = path.join(repoRoot, "config", "native-artifacts.json");
const installDirectory = path.join(repoRoot, "src-tauri", "binaries", "llama");
const MAX_ARCHIVE_ENTRIES = 4_096;
const MAX_EXTRACTED_BYTES = 4 * 1024 * 1024 * 1024;
const MAX_ARCHIVE_EXPANSION_RATIO = 200;
const MAX_TOOL_OUTPUT_BYTES = 8 * 1024 * 1024;
const ARCHIVE_INSPECTION_TIMEOUT_MS = 5 * 60 * 1000;

export function verifyArtifactBytes(bytes, artifact) {
  if (!Number.isSafeInteger(artifact.sizeBytes) || artifact.sizeBytes <= 0) {
    throw new Error("Artifact metadata has an invalid sizeBytes value.");
  }
  if (bytes.byteLength !== artifact.sizeBytes) {
    throw new Error(`Artifact size mismatch: expected ${artifact.sizeBytes}, received ${bytes.byteLength}.`);
  }
  const digest = createHash("sha256").update(bytes).digest("hex");
  if (digest !== artifact.sha256) {
    throw new Error(`Artifact SHA-256 mismatch: expected ${artifact.sha256}, received ${digest}.`);
  }
  return digest;
}

export function validateArchiveEntries(entries) {
  if (!Array.isArray(entries) || entries.length === 0) throw new Error("Archive contains no entries.");
  if (entries.length > MAX_ARCHIVE_ENTRIES) {
    throw new Error(`Archive contains ${entries.length} entries; limit is ${MAX_ARCHIVE_ENTRIES}.`);
  }

  for (const entry of entries) {
    const rawPath = typeof entry === "string" ? entry : entry.path;
    const type = typeof entry === "string" ? "-" : entry.type;
    if (typeof rawPath !== "string" || rawPath.length === 0 || rawPath.includes("\0")) {
      throw new Error("Archive contains an empty or NUL-containing path.");
    }
    const normalized = rawPath.replaceAll("\\", "/");
    const segments = normalized.split("/");
    if (
      normalized.startsWith("/") ||
      /^[A-Za-z]:/.test(normalized) ||
      segments.includes("..") ||
      path.posix.normalize(normalized).startsWith("../")
    ) {
      throw new Error(`Archive path escapes the extraction root: ${rawPath}`);
    }
    if (type !== "-" && type !== "d") {
      throw new Error(`Archive entry type '${type}' is not a regular file or directory: ${rawPath}`);
    }
  }
}

export function validateArchiveExpansion(archiveBytes, extractedBytes) {
  if (!Number.isSafeInteger(archiveBytes) || archiveBytes <= 0) {
    throw new Error("Archive size is invalid for expansion validation.");
  }
  if (!Number.isSafeInteger(extractedBytes) || extractedBytes < 0) {
    throw new Error("Extracted archive size is invalid for expansion validation.");
  }
  const limit =
    archiveBytes > Math.floor(MAX_EXTRACTED_BYTES / MAX_ARCHIVE_EXPANSION_RATIO)
      ? MAX_EXTRACTED_BYTES
      : archiveBytes * MAX_ARCHIVE_EXPANSION_RATIO;
  if (extractedBytes > limit) {
    throw new Error(
      `Archive expansion exceeded its ${limit}-byte safety limit (${MAX_ARCHIVE_EXPANSION_RATIO}x ratio, ${MAX_EXTRACTED_BYTES}-byte absolute ceiling).`,
    );
  }
  return limit;
}

export function selectArtifact(manifest, platform = process.platform, arch = process.arch) {
  const artifact = manifest.artifacts?.find((candidate) => candidate.platform === platform && candidate.arch === arch);
  if (!artifact) throw new Error(`No reviewed llama.cpp artifact exists for ${platform}/${arch}.`);
  const parsed = new URL(artifact.url);
  if (
    parsed.protocol !== "https:" ||
    parsed.hostname !== "github.com" ||
    path.posix.basename(parsed.pathname) !== artifact.fileName ||
    !/^[a-f0-9]{64}$/.test(artifact.sha256)
  ) {
    throw new Error("Native artifact metadata failed its fail-closed URL/digest validation.");
  }
  return artifact;
}

function hostMatchesSuffix(host, suffix) {
  return host === suffix || host.endsWith(`.${suffix}`);
}

/** FTR-006 build-time audit of the checked-in model trust root, independent of Rust's runtime
 * validator. This deliberately verifies metadata only; model payloads are streamed and checked
 * by the native downloader when a user explicitly installs one. */
export function validateModelCatalog(catalog, nativeManifest, releaseCapabilities) {
  if (catalog?.schemaVersion !== 1 || !catalog.reviewedAt || !Array.isArray(catalog.models) || catalog.models.length === 0) {
    throw new Error("Managed model catalog schema/review metadata is invalid.");
  }
  const reviewedTargets = new Set(
    nativeManifest.artifacts.map((artifact) => `${artifact.platform}-${artifact.arch}`),
  );
  const qualifiedTargets = new Set(
    releaseCapabilities.artifactPlatforms.map((platform) => platform.runtimeTarget),
  );
  if (qualifiedTargets.size === 0 || [...qualifiedTargets].some((target) => !reviewedTargets.has(target))) {
    throw new Error("Qualified packaged targets drift from reviewed runtime artifacts.");
  }
  const ids = new Set();
  for (const model of catalog.models) {
    if (
      ids.has(model.id) ||
      !/^[A-Za-z0-9._:-]{1,128}$/u.test(model.id) ||
      !/^[a-f0-9]{40}$/u.test(model.sourceCommit) ||
      !/^[a-f0-9]{64}$/u.test(model.sha256) ||
      !Number.isSafeInteger(model.sizeBytes) ||
      model.sizeBytes < 32 ||
      !Number.isSafeInteger(model.contextWindow) ||
      model.contextWindow <= 0 ||
      path.basename(model.fileName) !== model.fileName ||
      !model.fileName.endsWith(".gguf") ||
      model.compatibility?.runtime !== nativeManifest.runtime.name ||
      model.compatibility?.runtimeVersion !== nativeManifest.runtime.version
    ) {
      throw new Error(`Managed model '${model.id ?? "unknown"}' metadata is invalid.`);
    }
    ids.add(model.id);
    const targets = new Set(model.compatibility.platforms);
    if (targets.size !== qualifiedTargets.size || [...targets].some((target) => !qualifiedTargets.has(target))) {
      throw new Error(`Managed model '${model.id}' platform claims drift from qualified packaged targets.`);
    }
    if (!Array.isArray(model.allowedDownloadHostSuffixes) || model.allowedDownloadHostSuffixes.length === 0) {
      throw new Error(`Managed model '${model.id}' has no reviewed download hosts.`);
    }
    for (const rawUrl of [model.sourceRepository, model.downloadUrl, model.licenseUrl]) {
      const parsed = new URL(rawUrl);
      if (parsed.protocol !== "https:" || parsed.username || parsed.password || !rawUrl.includes(model.sourceCommit)) {
        throw new Error(`Managed model '${model.id}' contains a floating or non-HTTPS source.`);
      }
    }
    const download = new URL(model.downloadUrl);
    if (!model.allowedDownloadHostSuffixes.some((suffix) => hostMatchesSuffix(download.hostname, suffix))) {
      throw new Error(`Managed model '${model.id}' download host is not reviewed.`);
    }
  }
  return catalog.models.length;
}

async function hashFile(filePath) {
  const handle = await open(filePath, "r");
  const hash = createHash("sha256");
  const buffer = Buffer.allocUnsafe(1024 * 1024);
  let size = 0;
  try {
    for (;;) {
      const { bytesRead } = await handle.read(buffer, 0, buffer.length, null);
      if (bytesRead === 0) break;
      size += bytesRead;
      hash.update(buffer.subarray(0, bytesRead));
    }
  } finally {
    await handle.close();
  }
  return { size, sha256: hash.digest("hex") };
}

async function verifyRuntimeDirectory(directory, manifest, artifact) {
  const provenancePath = path.join(directory, "runtime-provenance.json");
  const provenanceMetadata = await stat(provenancePath);
  if (!provenanceMetadata.isFile() || provenanceMetadata.size > 256 * 1024) {
    throw new Error("Installed runtime provenance is not a bounded regular file.");
  }
  const provenance = JSON.parse(await readFile(provenancePath, "utf8"));
  for (const [actual, expected, label] of [
    [provenance.schemaVersion, 1, "schema"],
    [provenance.runtime, manifest.runtime.name, "runtime"],
    [provenance.version, manifest.runtime.version, "version"],
    [provenance.sourceCommit, manifest.runtime.sourceCommit, "source commit"],
    [provenance.license, manifest.runtime.license, "license"],
    [provenance.artifactFileName, artifact.fileName, "artifact filename"],
    [provenance.artifactUrl, artifact.url, "artifact URL"],
    [provenance.artifactSha256, artifact.sha256, "artifact digest"],
    [provenance.platform, artifact.platform, "platform"],
    [provenance.arch, artifact.arch, "architecture"],
  ]) {
    if (actual !== expected) throw new Error(`Installed runtime ${label} disagrees with the reviewed manifest.`);
  }
  if (!Array.isArray(provenance.installedFiles) || provenance.installedFiles.length === 0) {
    throw new Error("Installed runtime provenance contains no files.");
  }
  const expectedFiles = new Map();
  for (const file of provenance.installedFiles) {
    if (
      expectedFiles.has(file.name) ||
      path.basename(file.name) !== file.name ||
      !Number.isSafeInteger(file.sizeBytes) ||
      file.sizeBytes <= 0 ||
      !/^[a-f0-9]{64}$/u.test(file.sha256)
    ) {
      throw new Error("Installed runtime file provenance is invalid.");
    }
    expectedFiles.set(file.name, file);
  }
  const actualNames = (await readdir(directory))
    .filter((name) => name !== ".gitkeep" && name !== "runtime-provenance.json")
    .sort();
  if (
    actualNames.length !== expectedFiles.size ||
    actualNames.some((name) => !expectedFiles.has(name))
  ) {
    throw new Error("Installed runtime contains an unreviewed, missing, or extra file.");
  }
  for (const name of actualNames) {
    const filePath = path.join(directory, name);
    const metadata = await lstat(filePath);
    if (!metadata.isFile() || metadata.isSymbolicLink()) {
      throw new Error(`Installed runtime entry is not a regular file: ${name}`);
    }
    const actual = await hashFile(filePath);
    const expected = expectedFiles.get(name);
    if (actual.size !== expected.sizeBytes || actual.sha256 !== expected.sha256) {
      throw new Error(`Installed runtime file failed verification: ${name}`);
    }
  }
  const serverName = process.platform === "win32" ? "llama-server.exe" : "llama-server";
  const executable = path.join(directory, serverName);
  const executableRecord = expectedFiles.get(serverName);
  if (!executableRecord || executableRecord.sha256 !== provenance.runtimeSha256) {
    throw new Error("Installed runtime executable provenance is missing or inconsistent.");
  }
  const version = spawnSync(executable, ["--version"], {
    encoding: "utf8",
    timeout: 30_000,
    windowsHide: true,
    maxBuffer: 1024 * 1024,
  });
  if (version.error || version.status !== 0) {
    throw new Error(`Installed runtime executable smoke test failed: ${version.error?.message ?? version.stderr}`);
  }
  const output = `${version.stdout}\n${version.stderr}`;
  if (!output.includes(`version: ${manifest.runtime.version.replace(/^b/u, "")}`) || !output.includes(manifest.runtime.sourceCommit.slice(0, 9))) {
    throw new Error("Installed runtime executable reports an unexpected version or source commit.");
  }
  return provenance;
}

export async function verifyInstalledRuntime() {
  const manifest = JSON.parse(await readFile(manifestPath, "utf8"));
  const artifact = selectArtifact(manifest);
  return verifyRuntimeDirectory(installDirectory, manifest, artifact);
}

async function downloadVerified(url, destination, expectedSize, expectedSha256) {
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), 5 * 60 * 1000);
  const handle = await open(destination, "wx");
  const hash = createHash("sha256");
  let size = 0;
  try {
    const response = await fetch(url, { redirect: "follow", signal: controller.signal });
    if (!response.ok || !response.body) throw new Error(`Download failed with HTTP ${response.status}.`);
    if (new URL(response.url).protocol !== "https:") throw new Error("Download redirected to a non-HTTPS URL.");
    for await (const chunk of response.body) {
      const bytes = Buffer.from(chunk);
      size += bytes.length;
      if (size > expectedSize) throw new Error("Download exceeded the reviewed artifact size.");
      hash.update(bytes);
      await handle.write(bytes);
    }
    await handle.sync();
  } finally {
    clearTimeout(timeout);
    await handle.close();
  }
  const sha256 = hash.digest("hex");
  if (size !== expectedSize || sha256 !== expectedSha256) {
    throw new Error(
      `Downloaded artifact verification failed (size ${size}/${expectedSize}, SHA-256 ${sha256}/${expectedSha256}).`,
    );
  }
}

function runTar(args) {
  const result = spawnSync("tar", args, { encoding: "utf8", maxBuffer: MAX_TOOL_OUTPUT_BYTES, windowsHide: true });
  if (result.error) throw new Error(`Could not run the system archive tool: ${result.error.message}`);
  if (result.status !== 0) throw new Error(`Archive tool rejected the artifact: ${result.stderr.trim()}`);
  return result.stdout;
}

function inspectArchive(archivePath) {
  const paths = runTar(["-tf", archivePath]).split(/\r?\n/u).filter(Boolean);
  const types = runTar(["-tvf", archivePath])
    .split(/\r?\n/u)
    .filter(Boolean)
    .map((line) => line.trimStart()[0]);
  if (types.length !== paths.length) throw new Error("Archive listing was internally inconsistent.");
  validateArchiveEntries(paths.map((entryPath, index) => ({ path: entryPath, type: types[index] })));
}

// Streams every regular archive member to stdout without writing it to disk, counting bytes and
// terminating the archive tool as soon as the reviewed absolute/expansion-ratio ceiling is
// crossed. This happens before the real extraction, so a valid-hash but accidentally hostile
// reviewed artifact cannot fill the user's disk before the post-extraction walk notices.
export function measureArchivePayload(archivePath, archiveBytes) {
  validateArchiveExpansion(archiveBytes, 0);
  return new Promise((resolve, reject) => {
    const child = spawn("tar", ["-xOf", archivePath], {
      stdio: ["ignore", "pipe", "pipe"],
      windowsHide: true,
    });
    let extractedBytes = 0;
    let stderrBytes = 0;
    let stderr = "";
    let failure = null;
    const failAndStop = (error) => {
      if (failure) return;
      failure = error;
      child.kill();
    };
    const timeout = setTimeout(
      () => failAndStop(new Error("Archive expansion inspection timed out.")),
      ARCHIVE_INSPECTION_TIMEOUT_MS,
    );

    child.stdout.on("data", (chunk) => {
      extractedBytes += chunk.length;
      try {
        validateArchiveExpansion(archiveBytes, extractedBytes);
      } catch (error) {
        failAndStop(error);
      }
    });
    child.stderr.on("data", (chunk) => {
      stderrBytes += chunk.length;
      if (stderrBytes > MAX_TOOL_OUTPUT_BYTES) {
        failAndStop(new Error("Archive tool produced excessive diagnostic output."));
        return;
      }
      stderr += chunk.toString("utf8");
    });
    child.on("error", (error) => failAndStop(new Error(`Could not run the system archive tool: ${error.message}`)));
    child.on("close", (status) => {
      clearTimeout(timeout);
      if (failure) {
        reject(failure);
      } else if (status !== 0) {
        reject(new Error(`Archive tool rejected the artifact: ${stderr.trim()}`));
      } else {
        resolve(extractedBytes);
      }
    });
  });
}

async function walk(root, current = root, collected = []) {
  for (const entry of await readdir(current, { withFileTypes: true })) {
    const absolute = path.join(current, entry.name);
    const info = await lstat(absolute);
    if (info.isSymbolicLink()) throw new Error(`Extracted archive contains a symbolic link: ${entry.name}`);
    if (info.isDirectory()) await walk(root, absolute, collected);
    else if (info.isFile()) collected.push({ path: absolute, size: info.size });
    else throw new Error(`Extracted archive contains a non-regular filesystem entry: ${entry.name}`);
    if (collected.length > MAX_ARCHIVE_ENTRIES) throw new Error("Extracted archive exceeds the entry limit.");
  }
  return collected;
}

function isRuntimeFile(filePath) {
  const name = path.basename(filePath);
  return (
    name === (process.platform === "win32" ? "llama-server.exe" : "llama-server") ||
    name.endsWith(".dll") ||
    name.endsWith(".dylib") ||
    name.endsWith(".so") ||
    name.includes(".so.")
  );
}

async function replaceInstallDirectory(nextDirectory) {
  const parent = path.dirname(installDirectory);
  const previous = path.join(parent, `llama.install-previous-${randomUUID()}`);
  let hadExisting = false;
  try {
    await stat(installDirectory);
    hadExisting = true;
  } catch (error) {
    if (error.code !== "ENOENT") throw error;
  }
  if (hadExisting) await rename(installDirectory, previous);
  try {
    await rename(nextDirectory, installDirectory);
  } catch (error) {
    if (hadExisting) await rename(previous, installDirectory);
    throw error;
  }
  if (hadExisting) await rm(previous, { recursive: true, force: false });
}

export async function installRuntime() {
  const manifest = JSON.parse(await readFile(manifestPath, "utf8"));
  if (manifest.schemaVersion !== 1) throw new Error("Unsupported native-artifact manifest schema.");
  const artifact = selectArtifact(manifest);
  const temporary = await mkdtemp(path.join(os.tmpdir(), "ark-llama-install-"));
  const partial = path.join(temporary, `${artifact.fileName}.partial`);
  const archive = path.join(temporary, artifact.fileName);
  const extracted = path.join(temporary, "extracted");
  const nextDirectory = path.join(path.dirname(installDirectory), `llama.install-next-${randomUUID()}`);

  try {
    console.log(
      `Downloading reviewed ${manifest.runtime.name} ${manifest.runtime.version} artifact for ${process.platform}/${process.arch}…`,
    );
    await downloadVerified(artifact.url, partial, artifact.sizeBytes, artifact.sha256);
    await rename(partial, archive);
    inspectArchive(archive);
    await measureArchivePayload(archive, artifact.sizeBytes);
    await mkdir(extracted);
    runTar(["-xf", archive, "-C", extracted]);

    const files = await walk(extracted);
    const totalBytes = files.reduce((sum, file) => sum + file.size, 0);
    if (totalBytes > MAX_EXTRACTED_BYTES) throw new Error("Extracted runtime exceeds the 4 GiB safety limit.");
    const runtimeFiles = files.filter((file) => isRuntimeFile(file.path));
    const serverName = process.platform === "win32" ? "llama-server.exe" : "llama-server";
    const servers = runtimeFiles.filter((file) => path.basename(file.path) === serverName);
    if (servers.length !== 1) throw new Error(`Expected exactly one ${serverName}; found ${servers.length}.`);
    const names = new Set();
    for (const file of runtimeFiles) {
      const name = path.basename(file.path);
      if (names.has(name)) throw new Error(`Runtime archive contains duplicate install name: ${name}`);
      names.add(name);
    }

    await mkdir(nextDirectory, { recursive: false });
    try {
      await copyFile(
        path.join(installDirectory, ".gitkeep"),
        path.join(nextDirectory, ".gitkeep"),
        fsConstants.COPYFILE_EXCL,
      );
    } catch (error) {
      if (error.code !== "ENOENT") throw error;
      await writeFile(path.join(nextDirectory, ".gitkeep"), "");
    }
    for (const file of runtimeFiles) {
      const destination = path.join(nextDirectory, path.basename(file.path));
      await copyFile(file.path, destination, fsConstants.COPYFILE_EXCL);
      if (process.platform !== "win32") await chmod(destination, 0o755);
    }

    const licensePartial = path.join(temporary, "LICENSE.llama.cpp.partial");
    await downloadVerified(
      manifest.runtime.licenseUrl,
      licensePartial,
      manifest.runtime.licenseSizeBytes,
      manifest.runtime.licenseSha256,
    );
    await copyFile(licensePartial, path.join(nextDirectory, "LICENSE.llama.cpp"), fsConstants.COPYFILE_EXCL);
    const installedFiles = [];
    for (const name of (await readdir(nextDirectory)).filter((name) => name !== ".gitkeep").sort()) {
      const verified = await hashFile(path.join(nextDirectory, name));
      installedFiles.push({ name, sizeBytes: verified.size, sha256: verified.sha256 });
    }
    const runtimeHash = installedFiles.find((item) => item.name === serverName);
    if (!runtimeHash) throw new Error("Installed runtime executable was not included in provenance.");
    const provenance = {
      schemaVersion: 1,
      runtime: manifest.runtime.name,
      version: manifest.runtime.version,
      sourceRepository: manifest.runtime.sourceRepository,
      sourceCommit: manifest.runtime.sourceCommit,
      license: manifest.runtime.license,
      licenseUrl: manifest.runtime.licenseUrl,
      artifactFileName: artifact.fileName,
      artifactUrl: artifact.url,
      artifactSha256: artifact.sha256,
      runtimeSha256: runtimeHash.sha256,
      platform: artifact.platform,
      arch: artifact.arch,
      verifiedAt: new Date().toISOString(),
      installedFiles,
    };
    await writeFile(path.join(nextDirectory, "runtime-provenance.json"), `${JSON.stringify(provenance, null, 2)}\n`, {
      flag: "wx",
    });
    await verifyRuntimeDirectory(nextDirectory, manifest, artifact);
    await replaceInstallDirectory(nextDirectory);
    console.log(`Installed verified ${serverName}; SHA-256 ${runtimeHash.sha256}.`);
  } catch (error) {
    await rm(nextDirectory, { recursive: true, force: true });
    throw error;
  } finally {
    await rm(temporary, { recursive: true, force: true });
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) {
  if (!new Set(["install", "verify"]).has(process.argv[2])) {
    console.error("Usage: node scripts/runtime-supply-chain.mjs <install|verify>");
    process.exitCode = 2;
  } else {
    const operation = process.argv[2] === "install" ? installRuntime() : verifyInstalledRuntime();
    operation.then(() => {
      if (process.argv[2] === "verify") console.log("Installed runtime provenance, files, and executable are verified.");
    }).catch((error) => {
      console.error(`Runtime ${process.argv[2]} failed closed: ${error.message}`);
      process.exitCode = 1;
    });
  }
}
