import { readFileSync } from "node:fs";
import { join } from "node:path";

const root = join(import.meta.dirname, "..");
const read = (path) => readFileSync(join(root, path), "utf8");

function assert(condition, message) {
  if (!condition) throw new Error(`Markdown safety check failed: ${message}`);
}

// SEC-008: model output and imported content are rendered as Markdown. The single strongest
// guarantee against script/hostile-HTML execution in that path is structural, not behavioral:
// react-markdown does not render raw HTML unless a plugin (rehype-raw) explicitly opts in. This
// check exists so a future dependency bump or "let's support richer output" change can't
// silently add that plugin back without a reviewer noticing — it fails loudly instead.
const packageJson = JSON.parse(read("package.json"));
const allDependencies = { ...packageJson.dependencies, ...packageJson.devDependencies };
assert(!("rehype-raw" in allDependencies), "rehype-raw must not be a dependency — it would let Markdown content render as live HTML");

const markdownMessage = read("src/features/chat/MarkdownMessage.tsx");
assert(!markdownMessage.includes("rehype-raw"), "MarkdownMessage.tsx must not import rehype-raw");
assert(!/rehypePlugins\s*=\s*\{\s*\[/.test(markdownMessage), "MarkdownMessage.tsx must not configure rehype plugins that could enable raw HTML");

// dangerouslySetInnerHTML is the other real injection surface. Exactly one use is expected —
// the syntax-highlighted code block, whose safety is independently covered by
// src/lib/highlightCode.test.ts's hostile-fixture regression tests. A second occurrence
// anywhere in the frontend means a new raw-HTML sink was added without equivalent scrutiny.
const frontendSourceFiles = ["src/features/chat/MarkdownMessage.tsx"];
const dangerousUsageCount = frontendSourceFiles.reduce(
  (count, path) => count + (read(path).match(/dangerouslySetInnerHTML/g)?.length ?? 0),
  0,
);
assert(
  dangerousUsageCount === 1,
  `expected exactly 1 dangerouslySetInnerHTML usage (the tested syntax-highlighted code block), found ${dangerousUsageCount} — a new raw-HTML sink needs the same hostile-fixture test treatment as highlightCode.test.ts before this count changes`,
);

// External links must render through the safety wrapper, not react-markdown's default <a>.
assert(
  markdownMessage.includes("a({ href, children, ...props }) {") && markdownMessage.includes("<MarkdownLink"),
  "MarkdownMessage.tsx must override react-markdown's default link rendering with the validated MarkdownLink component",
);
const externalLinks = read("src/lib/externalLinks.ts");
assert(
  /ALLOWED_EXTERNAL_LINK_SCHEMES\s*=\s*new Set\(\[[^\]]*"http:"[^\]]*"https:"[^\]]*\]\)|ALLOWED_EXTERNAL_LINK_SCHEMES/.test(externalLinks),
  "src/lib/externalLinks.ts must retain its scheme allowlist",
);
assert(!externalLinks.includes('"javascript:"'), "the external-link scheme allowlist must never include javascript:");
assert(!externalLinks.includes('"data:"'), "the external-link scheme allowlist must never include data:");
assert(!externalLinks.includes('"file:"'), "the external-link scheme allowlist must never include file:");

console.log("Markdown safety check passed: no rehype-raw, exactly one reviewed dangerouslySetInnerHTML sink, external links route through the validated MarkdownLink/checkExternalLink path.");
