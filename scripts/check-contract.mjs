// ARC-002: verifies every TypeScript interface in `src/types/ark.ts` that corresponds to a
// shared DTO still declares exactly the field set recorded in `contract/schema.json` — the same
// fixture `src-tauri/src/contract.rs` independently checks the Rust structs against. Neither
// side reads the other's source; either one drifting from the shared fixture fails its own
// language's check. Run via `pnpm run contract:check` (wired into CI).
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import path from "node:path";
import ts from "typescript";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const schemaPath = path.join(repoRoot, "contract", "schema.json");
const typesPath = path.join(repoRoot, "src", "types", "ark.ts");

const schema = JSON.parse(readFileSync(schemaPath, "utf8"));
const expectedByType = schema.types;

const sourceText = readFileSync(typesPath, "utf8");
const sourceFile = ts.createSourceFile(typesPath, sourceText, ts.ScriptTarget.Latest, true, ts.ScriptKind.TS);

/** @type {Map<string, string[]>} */
const declaredByType = new Map();

for (const statement of sourceFile.statements) {
  if (!ts.isInterfaceDeclaration(statement)) {
    continue;
  }
  const name = statement.name.text;
  const properties = statement.members
    .filter((member) => ts.isPropertySignature(member) && member.name && ts.isIdentifier(member.name))
    .map((member) => member.name.text);
  declaredByType.set(name, properties);
}

let failed = false;

for (const [typeName, expectedFields] of Object.entries(expectedByType)) {
  const declaredFields = declaredByType.get(typeName);
  if (!declaredFields) {
    console.error(`✗ ${typeName}: contract/schema.json has an entry, but no "interface ${typeName}" was found in src/types/ark.ts.`);
    failed = true;
    continue;
  }

  const expectedSet = new Set(expectedFields);
  const declaredSet = new Set(declaredFields);
  const missing = expectedFields.filter((field) => !declaredSet.has(field));
  const unexpected = declaredFields.filter((field) => !expectedSet.has(field));

  if (missing.length > 0 || unexpected.length > 0) {
    console.error(
      `✗ ${typeName} has drifted from contract/schema.json — missing fields: [${missing.join(", ")}], ` +
        `unexpected fields: [${unexpected.join(", ")}]. If this is an intentional change, update ` +
        `contract/schema.json (and the corresponding Rust struct) in the same change — see ` +
        `docs/protocol-versioning.md.`,
    );
    failed = true;
  } else {
    console.log(`✓ ${typeName}`);
  }
}

if (failed) {
  console.error("\nContract check failed: TypeScript/contract drift detected.");
  process.exit(1);
}

console.log(`\nContract check passed: ${Object.keys(expectedByType).length} types match contract/schema.json.`);
