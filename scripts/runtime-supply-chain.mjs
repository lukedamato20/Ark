import { createHash, randomUUID } from "node:crypto";
import { spawnSync } from "node:child_process";
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
const MAX_TOOL_OUTPUT_BYTES = 8 * 1024 * 1024;

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
  const paths = runTar(["-tf", archivePath])
    .split(/\r?\n/u)
    .filter(Boolean);
  const types = runTar(["-tvf", archivePath])
    .split(/\r?\n/u)
    .filter(Boolean)
    .map((line) => line.trimStart()[0]);
  if (types.length !== paths.length) throw new Error("Archive listing was internally inconsistent.");
  validateArchiveEntries(paths.map((entryPath, index) => ({ path: entryPath, type: types[index] })));
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
    console.log(`Downloading reviewed ${manifest.runtime.name} ${manifest.runtime.version} artifact for ${process.platform}/${process.arch}…`);
    await downloadVerified(artifact.url, partial, artifact.sizeBytes, artifact.sha256);
    await rename(partial, archive);
    inspectArchive(archive);
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
      await copyFile(path.join(installDirectory, ".gitkeep"), path.join(nextDirectory, ".gitkeep"), fsConstants.COPYFILE_EXCL);
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
  if (process.argv[2] !== "install") {
    console.error("Usage: node scripts/runtime-supply-chain.mjs install");
    process.exitCode = 2;
  } else {
    installRuntime().catch((error) => {
      console.error(`Runtime installation failed closed: ${error.message}`);
      process.exitCode = 1;
    });
  }
}
