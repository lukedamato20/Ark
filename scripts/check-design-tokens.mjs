// UX: guards against the exact bug class found in ChatView's hand-rolled dropdowns — a
// `bg-<token>`/`text-<token>-foreground` class referencing a design-system color token that was
// never actually registered in tailwind.config.ts's `theme.extend.colors`, so Tailwind's JIT
// compiler silently emits no CSS for it (an opaque-looking class that renders as fully
// transparent). Deliberately narrow rather than exhaustive: it only checks the `bg-X` and
// `text-X-foreground` patterns, since those are the only two shapes this codebase's design
// system actually uses for semantic surface/foreground tokens (see every `DEFAULT`/`foreground`
// pair in tailwind.config.ts) — checking every Tailwind color utility (border-, ring-, from-,
// via-, etc.) would require classifying far more non-color utility keywords and risk false
// positives on genuinely unrelated classes; this narrower check has none of that risk while still
// covering the real failure mode.
import fs from "node:fs";
import path from "node:path";
import process from "node:process";

const root = process.cwd();
const sourceRoot = path.join(root, "src");
const sourceExtensions = new Set([".tsx"]);
const files = [];

function walk(directory) {
  for (const entry of fs.readdirSync(directory, { withFileTypes: true })) {
    const absolute = path.join(directory, entry.name);
    if (entry.isDirectory()) walk(absolute);
    else if (sourceExtensions.has(path.extname(entry.name))) files.push(absolute);
  }
}

walk(sourceRoot);

function relative(file) {
  return path.relative(root, file).replaceAll("\\", "/");
}

// Extract the custom token names registered in tailwind.config.ts's `theme.extend.colors` block
// (top-level keys only — both `key: "hsl(...)"` and `key: { DEFAULT: ..., foreground: ... }`
// shapes).
const tailwindConfigSource = fs.readFileSync(path.join(root, "tailwind.config.ts"), "utf8");
const colorsBlockStart = tailwindConfigSource.indexOf("colors: {");
if (colorsBlockStart === -1) {
  console.error("check-design-tokens: could not find `colors: {` in tailwind.config.ts");
  process.exit(1);
}
let depth = 0;
let colorsBlockEnd = -1;
for (let index = colorsBlockStart + "colors: {".length - 1; index < tailwindConfigSource.length; index++) {
  const char = tailwindConfigSource[index];
  if (char === "{") depth++;
  else if (char === "}") {
    depth--;
    if (depth === 0) {
      colorsBlockEnd = index;
      break;
    }
  }
}
const colorsBlock = tailwindConfigSource.slice(colorsBlockStart, colorsBlockEnd);
const definedTokens = new Set([...colorsBlock.matchAll(/^\s{8}(\w+):/gm)].map((match) => match[1]));

// Tailwind's default palette family names — any of these plus an optional shade (e.g.
// `bg-red-500`) is a real, always-defined utility, not a project-specific token.
const defaultPaletteFamilies = new Set([
  "slate", "gray", "zinc", "neutral", "stone", "red", "orange", "amber", "yellow", "lime",
  "green", "emerald", "teal", "cyan", "sky", "blue", "indigo", "violet", "purple", "fuchsia",
  "pink", "rose",
]);
const universalKeywords = new Set(["transparent", "current", "inherit", "white", "black"]);

// Non-color `bg-*` utility keywords (background-attachment/position/size/repeat/clip/origin/
// blend/gradient-direction/legacy-opacity) — real Tailwind utilities this check must not flag.
const backgroundUtilityKeywords = new Set([
  "fixed", "local", "scroll", "clip", "repeat", "no", "origin", "auto", "cover", "contain",
  "bottom", "center", "left", "right", "top", "none", "gradient", "blend", "opacity",
]);

function isKnownToken(name) {
  if (definedTokens.has(name)) return true;
  if (universalKeywords.has(name)) return true;
  const [family] = name.split("-");
  return defaultPaletteFamilies.has(family);
}

const bgPattern = /\bbg-([a-zA-Z][a-zA-Z0-9]*(?:-\d{2,3})?)\b/g;
const textForegroundPattern = /\btext-([a-zA-Z][a-zA-Z0-9]*)-foreground\b/g;

const violations = [];

for (const file of files) {
  const source = fs.readFileSync(file, "utf8");

  for (const match of source.matchAll(bgPattern)) {
    const name = match[1];
    if (backgroundUtilityKeywords.has(name.split("-")[0])) continue;
    if (!isKnownToken(name)) {
      violations.push(`${relative(file)}: \`bg-${name}\` references an undefined design token`);
    }
  }

  for (const match of source.matchAll(textForegroundPattern)) {
    const name = match[1];
    if (!isKnownToken(name)) {
      violations.push(
        `${relative(file)}: \`text-${name}-foreground\` references an undefined design token`,
      );
    }
  }
}

if (violations.length > 0) {
  console.error(
    [...new Set(violations)].map((violation) => `- ${violation}`).join("\n") +
      "\n\nAdd the missing token to tailwind.config.ts's theme.extend.colors and src/styles.css's :root/.dark blocks, or fix the typo.",
  );
  process.exitCode = 1;
} else {
  console.log(`Design tokens check passed: ${files.length} .tsx files, ${definedTokens.size} known tokens.`);
}
