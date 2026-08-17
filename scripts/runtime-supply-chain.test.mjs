import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdtemp, readFile, rm, stat, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import {
  measureArchivePayload,
  selectArtifact,
  validateModelCatalog,
  validateArchiveEntries,
  validateArchiveExpansion,
  verifyArtifactBytes,
} from "./runtime-supply-chain.mjs";

test("managed model catalog stays pinned to reviewed runtime targets and immutable sources", async () => {
  const catalog = JSON.parse(await readFile(new URL("../config/model-catalog.json", import.meta.url), "utf8"));
  const nativeManifest = JSON.parse(
    await readFile(new URL("../config/native-artifacts.json", import.meta.url), "utf8"),
  );
  const releaseCapabilities = JSON.parse(
    await readFile(new URL("../config/release-capabilities.json", import.meta.url), "utf8"),
  );
  assert.equal(validateModelCatalog(catalog, nativeManifest, releaseCapabilities), 1);

  const floating = structuredClone(catalog);
  floating.models[0].downloadUrl = "https://huggingface.co/Qwen/model/resolve/main/model.gguf";
  assert.throws(
    () => validateModelCatalog(floating, nativeManifest, releaseCapabilities),
    /floating or non-HTTPS/u,
  );

  const unsupported = structuredClone(catalog);
  unsupported.models[0].compatibility.platforms.push("freebsd-x64");
  assert.throws(
    () => validateModelCatalog(unsupported, nativeManifest, releaseCapabilities),
    /platform claims drift/u,
  );
});

function fixtureArtifact(bytes) {
  return {
    sizeBytes: bytes.length,
    sha256: createHash("sha256").update(bytes).digest("hex"),
  };
}

test("verified artifacts reject tampered and truncated payloads before extraction", () => {
  const original = Buffer.from("reviewed archive bytes");
  const metadata = fixtureArtifact(original);
  assert.equal(verifyArtifactBytes(original, metadata), metadata.sha256);
  assert.throws(() => verifyArtifactBytes(Buffer.from("tampered archive bytes"), metadata), /SHA-256 mismatch/u);
  assert.throws(() => verifyArtifactBytes(original.subarray(0, original.length - 1), metadata), /size mismatch/u);
});

test("archive validation rejects traversal, absolute paths, unresolvable links, and device entries", () => {
  validateArchiveEntries([
    { path: "llama-b9859/llama-server", type: "-" },
    { path: "llama-b9859/libggml.so", type: "-" },
    { path: "llama-b9859/", type: "d" },
  ]);
  for (const entry of [
    { path: "../outside", type: "-" },
    { path: "safe/../../outside", type: "-" },
    { path: "/absolute", type: "-" },
    { path: "C:\\outside", type: "-" },
    { path: "safe/link", type: "l" }, // symlink with no known target fails closed
    { path: "safe/device", type: "b" },
  ]) {
    assert.throws(() => validateArchiveEntries([entry]));
  }
});

test("archive validation accepts a versioned-library symlink only when its target stays inside the archive root", () => {
  // The real-world case this exists for: llama.cpp's own macOS/Linux release archives contain
  // entries like "llama-b9859/libmtmd.so.0 -> libmtmd.so" -- a same-directory sibling symlink.
  validateArchiveEntries([
    { path: "llama-b9859/libmtmd.so.0", type: "l", linkTarget: "libmtmd.so" },
    { path: "llama-b9859/nested/lib.so.0", type: "l", linkTarget: "../lib.so" },
  ]);
  for (const entry of [
    { path: "safe/link", type: "l", linkTarget: "../../outside" },
    { path: "safe/link", type: "l", linkTarget: "/absolute/outside" },
    { path: "safe/link", type: "l", linkTarget: "C:\\outside" },
    { path: "safe/link", type: "l", linkTarget: "" },
  ]) {
    assert.throws(() => validateArchiveEntries([entry]), /escapes the extraction root|no safe, known target/u);
  }
});

test("archive expansion limits reject decompression bombs before filesystem extraction", () => {
  assert.equal(validateArchiveExpansion(1_024, 200 * 1_024), 200 * 1_024);
  assert.throws(() => validateArchiveExpansion(1_024, 200 * 1_024 + 1), /expansion exceeded/u);
  assert.equal(validateArchiveExpansion(100 * 1024 * 1024, 4 * 1024 * 1024 * 1024), 4 * 1024 * 1024 * 1024);
  assert.throws(() => validateArchiveExpansion(100 * 1024 * 1024, 4 * 1024 * 1024 * 1024 + 1), /safety limit/u);
  assert.throws(() => validateArchiveExpansion(0, 0), /Archive size is invalid/u);
});

test("archive payload inspection streams and counts members without extracting to disk", async () => {
  const directory = await mkdtemp(path.join(os.tmpdir(), "ark-archive-measure-"));
  const payload = Buffer.alloc(64 * 1024, 0x61);
  const archive = path.join(directory, "fixture.tar");
  try {
    await writeFile(path.join(directory, "payload.bin"), payload);
    const created = spawnSync("tar", ["-cf", archive, "-C", directory, "payload.bin"], {
      encoding: "utf8",
      windowsHide: true,
    });
    assert.equal(created.status, 0, created.stderr);
    const archiveBytes = (await stat(archive)).size;

    assert.equal(await measureArchivePayload(archive, archiveBytes), payload.length);
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});

test("artifact selection fails closed for unsupported targets and altered URLs", () => {
  const manifest = {
    artifacts: [
      {
        platform: "win32",
        arch: "x64",
        fileName: "runtime.zip",
        url: "https://github.com/example/project/releases/download/v1/runtime.zip",
        sizeBytes: 1,
        sha256: "a".repeat(64),
      },
    ],
  };
  assert.equal(selectArtifact(manifest, "win32", "x64").fileName, "runtime.zip");
  assert.throws(() => selectArtifact(manifest, "freebsd", "x64"), /No reviewed/u);
  manifest.artifacts[0].url = "http://github.com/example/runtime.zip";
  assert.throws(() => selectArtifact(manifest, "win32", "x64"), /fail-closed/u);
});
