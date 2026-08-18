import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const css = readFileSync(new URL("../styles.css", import.meta.url), "utf8");
const palettes = ["blue", "violet", "teal", "amber", "graphite"] as const;

test("every approved accent palette meets text and focus contrast in light and dark themes", () => {
  for (const theme of ["light", "dark"] as const) {
    const baseSelector = theme === "dark" ? ".dark" : ":root";
    const base = variables(baseSelector);
    for (const palette of palettes) {
      const overrides =
        palette === "blue" ? {} : variables(`${theme === "dark" ? ".dark" : ":root"}[data-accent="${palette}"]`);
      const tokens = { ...base, ...overrides };
      assert.ok(
        contrast(tokens.primary, tokens["primary-foreground"]) >= 4.5,
        `${theme}/${palette} primary text must meet WCAG AA`,
      );
      assert.ok(
        contrast(tokens.ring, tokens.background) >= 3,
        `${theme}/${palette} focus ring must have 3:1 contrast against the background`,
      );
    }
  }
});

function variables(selector: string): Record<string, string> {
  const escaped = selector.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const match = css.match(new RegExp(`${escaped}\\s*\\{([^}]*)\\}`));
  assert.ok(match, `missing CSS selector ${selector}`);
  return Object.fromEntries(
    [...match[1].matchAll(/--([a-z-]+):\s*([^;]+);/g)].map((entry) => [entry[1], entry[2].trim()]),
  );
}

function contrast(a: string, b: string): number {
  const first = luminance(a);
  const second = luminance(b);
  return (Math.max(first, second) + 0.05) / (Math.min(first, second) + 0.05);
}

function luminance(hsl: string): number {
  const [hue, saturation, lightness] = hsl.match(/[\d.]+/g)?.map(Number) ?? [];
  assert.ok(Number.isFinite(hue) && Number.isFinite(saturation) && Number.isFinite(lightness), `invalid HSL ${hsl}`);
  const s = saturation / 100;
  const l = lightness / 100;
  const chroma = (1 - Math.abs(2 * l - 1)) * s;
  const segment = (((hue % 360) + 360) % 360) / 60;
  const x = chroma * (1 - Math.abs((segment % 2) - 1));
  const [r1, g1, b1] =
    segment < 1
      ? [chroma, x, 0]
      : segment < 2
        ? [x, chroma, 0]
        : segment < 3
          ? [0, chroma, x]
          : segment < 4
            ? [0, x, chroma]
            : segment < 5
              ? [x, 0, chroma]
              : [chroma, 0, x];
  const offset = l - chroma / 2;
  return [r1 + offset, g1 + offset, b1 + offset]
    .map((channel) => (channel <= 0.03928 ? channel / 12.92 : ((channel + 0.055) / 1.055) ** 2.4))
    .reduce((sum, channel, index) => sum + channel * [0.2126, 0.7152, 0.0722][index], 0);
}
