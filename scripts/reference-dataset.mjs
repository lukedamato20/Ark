import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import process from "node:process";

const baseTime = Date.parse("2026-01-01T00:00:00.000Z");

function json(value) {
  return `${JSON.stringify(value)}\n`;
}

const conversations = Array.from({ length: 1_000 }, (_, index) => ({
  id: `conversation-${index.toString().padStart(4, "0")}`,
  title: `Synthetic topic ${index.toString().padStart(4, "0")} résumé 東京`,
  createdAt: new Date(baseTime + index * 1_000).toISOString(),
  updatedAt: new Date(baseTime + index * 60_000).toISOString(),
  archived: index % 7 === 0,
  projectId: index % 3 === 0 ? `project-${index % 9}` : null,
}));

const thread = Array.from({ length: 100 }, (_, index) => ({
  id: `message-${index.toString().padStart(3, "0")}`,
  conversationId: "reference-thread",
  parentMessageId: index === 0 ? null : `message-${(index - 1).toString().padStart(3, "0")}`,
  revisionOfMessageId: index > 0 && index % 20 === 0 ? `message-${(index - 2).toString().padStart(3, "0")}` : null,
  pathIndex: index,
  role: index % 2 === 0 ? "user" : "assistant",
  content: `Synthetic message ${index}: no user content or secrets. Résumé 東京 مرحبا.`,
  status: "complete",
  createdAt: new Date(baseTime + index * 1_000).toISOString(),
  updatedAt: new Date(baseTime + index * 1_000).toISOString(),
}));

const longOutput = "0123456789abcdef".repeat(6_250);
const largeImport = {
  schemaVersion: 1,
  exportedAt: "2026-01-01T00:00:00.000Z",
  conversation: {
    id: "large-import-reference",
    title: "Synthetic large import",
    createdAt: "2026-01-01T00:00:00.000Z",
    updatedAt: "2026-01-01T00:00:00.000Z",
    archived: false,
  },
  messages: Array.from({ length: 20_000 }, (_, index) => ({
    id: `import-message-${index.toString().padStart(5, "0")}`,
    conversationId: "large-import-reference",
    parentMessageId: index === 0 ? null : `import-message-${(index - 1).toString().padStart(5, "0")}`,
    revisionOfMessageId: null,
    pathIndex: index,
    role: index % 2 === 0 ? "user" : "assistant",
    content: `Synthetic import message ${index}`,
    status: "complete",
    createdAt: new Date(baseTime + index).toISOString(),
    updatedAt: new Date(baseTime + index).toISOString(),
  })),
};

const outputs = new Map([
  ["conversations-1000.json", json(conversations)],
  ["thread-100.json", json(thread)],
  ["output-100000.txt", longOutput],
  ["large-import-20000.json", json(largeImport)],
]);

const manifest = {
  schemaVersion: 1,
  generatedBy: "scripts/reference-dataset.mjs",
  synthetic: true,
  files: Object.fromEntries(
    [...outputs].map(([name, content]) => [
      name,
      {
        bytes: Buffer.byteLength(content),
        sha256: crypto.createHash("sha256").update(content).digest("hex"),
      },
    ]),
  ),
};

if (process.argv.includes("--print-manifest")) {
  process.stdout.write(`${JSON.stringify(manifest, null, 2)}\n`);
} else if (process.argv.includes("--check")) {
  const expected = JSON.parse(
    fs.readFileSync(path.join(process.cwd(), "fixtures/reference-dataset-manifest.json"), "utf8"),
  );
  if (JSON.stringify(expected) !== JSON.stringify(manifest)) {
    console.error("Reference dataset manifest drifted. Run pnpm baseline:manifest and review the change.");
    process.exitCode = 1;
  } else {
    console.log(
      "Reference dataset passed: 1,000 conversations, 100 messages, 100,000 characters, branches, 20,000-message import.",
    );
  }
} else {
  const outputDirectory = path.join(process.cwd(), ".artifacts/reference-dataset");
  fs.mkdirSync(outputDirectory, { recursive: true });
  for (const [name, content] of outputs) {
    fs.writeFileSync(path.join(outputDirectory, name), content, { flag: "w" });
  }
  fs.writeFileSync(path.join(outputDirectory, "manifest.json"), `${JSON.stringify(manifest, null, 2)}\n`, {
    flag: "w",
  });
  console.log(`Wrote deterministic synthetic fixtures to ${path.relative(process.cwd(), outputDirectory)}.`);
}
