import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import test from "node:test";
import { selectArtifact, validateArchiveEntries, verifyArtifactBytes } from "./runtime-supply-chain.mjs";

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

test("archive validation rejects traversal, absolute paths, links, and device entries", () => {
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
    { path: "safe/link", type: "l" },
    { path: "safe/device", type: "b" },
  ]) {
    assert.throws(() => validateArchiveEntries([entry]));
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
