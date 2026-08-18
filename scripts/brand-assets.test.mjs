import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import test from "node:test";
import { resolve } from "node:path";

const root = resolve(import.meta.dirname, "..");
const sourcePath = resolve(root, "src/assets/brand/ark-mark.svg");
const iconRoot = resolve(root, "src-tauri/icons");

test("the canonical Ark SVG is local, bounded, and declarative", () => {
  const source = readFileSync(sourcePath, "utf8");
  assert.ok(Buffer.byteLength(source) < 10_000);
  assert.match(source, /^<svg[\s\S]*<\/svg>\s*$/);
  assert.doesNotMatch(source, /<script|javascript:|data:|@font-face|(?:href|src)=["']https?:/i);
});

test("desktop and mobile package icons are generated from the canonical mark", () => {
  const pngs = [
    ["icon.png", 512, 512],
    ["32x32.png", 32, 32],
    ["128x128.png", 128, 128],
    ["128x128@2x.png", 256, 256],
    ["ios/AppIcon-512@2x.png", 1024, 1024],
    ["android/mipmap-mdpi/ic_launcher.png", 48, 48],
  ];
  for (const [relativePath, expectedWidth, expectedHeight] of pngs) {
    const bytes = readFileSync(resolve(iconRoot, relativePath));
    assert.equal(bytes.subarray(1, 4).toString("ascii"), "PNG", relativePath);
    assert.equal(bytes.readUInt32BE(16), expectedWidth, `${relativePath} width`);
    assert.equal(bytes.readUInt32BE(20), expectedHeight, `${relativePath} height`);
  }
  for (const relativePath of ["icon.ico", "icon.icns"]) {
    const path = resolve(iconRoot, relativePath);
    assert.ok(existsSync(path) && readFileSync(path).length > 1_000, `${relativePath} must be a non-empty derivative`);
  }
});
