import fs from "node:fs";
import path from "node:path";
import process from "node:process";

const root = process.cwd();
const sourceRoot = path.join(root, "src");
const sourceExtensions = new Set([".ts", ".tsx"]);
const files = [];

function walk(directory) {
  for (const entry of fs.readdirSync(directory, { withFileTypes: true })) {
    const absolute = path.join(directory, entry.name);
    if (entry.isDirectory()) walk(absolute);
    else if (sourceExtensions.has(path.extname(entry.name))) files.push(absolute);
  }
}

walk(sourceRoot);

const importPattern =
  /\b(?:import|export)\s+(?:type\s+)?(?:[^"'();]*?\s+from\s+)?["']([^"']+)["']|\bimport\(\s*["']([^"']+)["']\s*\)/g;
const graph = new Map(files.map((file) => [file, []]));
const violations = [];

function relative(file) {
  return path.relative(root, file).replaceAll("\\", "/");
}

function resolveSourceImport(importer, specifier) {
  const unresolved = path.resolve(path.dirname(importer), specifier);
  const candidates = path.extname(unresolved)
    ? [unresolved]
    : [...sourceExtensions].flatMap((extension) => [
        `${unresolved}${extension}`,
        path.join(unresolved, `index${extension}`),
      ]);
  return candidates.find((candidate) => fs.existsSync(candidate));
}

function topLevelArea(file) {
  const parts = relative(file).split("/");
  return parts[1] ?? parts[0];
}

function checkBoundary(importer, imported, specifier) {
  const from = topLevelArea(importer);
  const to = topLevelArea(imported);
  const importerName = relative(importer);

  if (specifier.startsWith("@tauri-apps/") && importerName !== "src/lib/ArkClient.ts") {
    violations.push(`${importerName}: Tauri APIs are restricted to src/lib/ArkClient.ts`);
  }
  if (from === "types" && to !== "types") {
    violations.push(`${importerName}: types may only depend on types (found ${relative(imported)})`);
  }
  if (from === "lib" && ["app", "components", "features", "state", "ui"].includes(to)) {
    violations.push(`${importerName}: lib cannot depend on ${to} (found ${relative(imported)})`);
  }
  if (from === "ui" && !["lib", "types", "ui"].includes(to)) {
    violations.push(`${importerName}: ui primitives cannot depend on ${to} (found ${relative(imported)})`);
  }
  if (from === "state" && ["app", "components", "features", "ui"].includes(to)) {
    violations.push(`${importerName}: state cannot depend on ${to} (found ${relative(imported)})`);
  }
  if (from === "app" && ["components", "features", "ui"].includes(to)) {
    violations.push(`${importerName}: application orchestration cannot depend on ${to} (found ${relative(imported)})`);
  }
  if (from === "features" && ["app", "components"].includes(to)) {
    violations.push(`${importerName}: features cannot depend on ${to} (found ${relative(imported)})`);
  }
  if (from === "components" && ["app", "features", "state"].includes(to)) {
    violations.push(`${importerName}: shell components cannot depend on ${to} (found ${relative(imported)})`);
  }
}

for (const file of files) {
  const source = fs.readFileSync(file, "utf8");
  for (const match of source.matchAll(importPattern)) {
    const specifier = match[1] ?? match[2];
    if (specifier.startsWith("@tauri-apps/")) {
      checkBoundary(file, file, specifier);
      continue;
    }
    if (!specifier.startsWith(".")) continue;
    const resolved = resolveSourceImport(file, specifier);
    if (!resolved) {
      violations.push(`${relative(file)}: cannot resolve relative import "${specifier}"`);
      continue;
    }
    if (graph.has(resolved)) {
      graph.get(file).push(resolved);
      checkBoundary(file, resolved, specifier);
    }
  }
}

const visiting = new Set();
const visited = new Set();
const stack = [];

function visit(file) {
  if (visiting.has(file)) {
    const cycleStart = stack.indexOf(file);
    const cycle = [...stack.slice(cycleStart), file].map(relative).join(" -> ");
    violations.push(`Circular frontend dependency: ${cycle}`);
    return;
  }
  if (visited.has(file)) return;
  visiting.add(file);
  stack.push(file);
  for (const dependency of graph.get(file)) visit(dependency);
  stack.pop();
  visiting.delete(file);
  visited.add(file);
}

for (const file of files) visit(file);

if (violations.length > 0) {
  console.error(violations.map((violation) => `- ${violation}`).join("\n"));
  process.exitCode = 1;
} else {
  console.log(`Module boundaries passed: ${files.length} frontend modules, no cycles.`);
}
