import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import tailwindConfig from "../../tailwind.config.ts";

test("named typography tiers define size, line height, and weight", () => {
  const tiers = tailwindConfig.theme.extend.fontSize;
  assert.deepEqual(Object.keys(tiers), ["caption", "metadata", "body", "emphasis", "section", "view"]);
  for (const [name, value] of Object.entries(tiers)) {
    const [size, options] = value;
    assert.match(size, /^var\(--text-[a-z-]+\)$/i, `${name} size`);
    assert.match(options.lineHeight, /^\d+(?:\.\d+)?rem$/, `${name} line height`);
    assert.match(options.fontWeight, /^(400|500|600)$/, `${name} weight`);
  }
});

test("font stacks and dark semantic surfaces stay explicit, local, and neutral", () => {
  const css = readFileSync(new URL("../styles.css", import.meta.url), "utf8");
  assert.match(css, /--font-ui:[\s\S]*Segoe UI Variable[\s\S]*Inter Variable/);
  assert.match(css, /--font-code:[\s\S]*Cascadia Code[\s\S]*Liberation Mono/);
  assert.doesNotMatch(css, /@import\s+url|https?:\/\//i);
  const dark = css.match(/\.dark\s*\{([\s\S]*?)\n\}/)?.[1] ?? "";
  for (const token of ["background", "card", "popover", "muted", "border"]) {
    const saturation = Number(dark.match(new RegExp(`--${token}:\\s*\\d+(?:\\.\\d+)?\\s+(\\d+(?:\\.\\d+)?)%`))?.[1]);
    assert.ok(Number.isFinite(saturation) && saturation <= 2, `${token} must remain visually neutral`);
  }
});
