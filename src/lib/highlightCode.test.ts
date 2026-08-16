import assert from "node:assert/strict";
import test from "node:test";
import { escapeHtml, highlightCode } from "./highlightCode.ts";

const HOSTILE_SNIPPETS = [
  "<script>alert(document.cookie)</script>",
  "<img src=x onerror=alert(1)>",
  "</code></pre><script>alert(1)</script>",
  '"><svg onload=alert(1)>',
  "<style>*{color:red}</style>",
];

test("escapeHtml neutralizes every hostile fixture", () => {
  for (const snippet of HOSTILE_SNIPPETS) {
    const escaped = escapeHtml(snippet);
    assert.ok(!escaped.includes("<script"), `escapeHtml left a live <script> in: ${escaped}`);
    assert.ok(!escaped.includes("<img"), `escapeHtml left a live <img> in: ${escaped}`);
    assert.ok(!escaped.includes("<svg"), `escapeHtml left a live <svg> in: ${escaped}`);
    assert.ok(!escaped.includes("<style"), `escapeHtml left a live <style> in: ${escaped}`);
  }
});

test("highlightCode keeps hostile content inert for a known language (javascript)", () => {
  for (const snippet of HOSTILE_SNIPPETS) {
    const html = highlightCode(snippet, "javascript");
    // The actual safety property is "no unescaped '<' starts a live tag" — not "the substring
    // 'onerror=' never appears anywhere," which highlight.js's own tokenizing can legitimately
    // produce as inert escaped text (e.g. wrapping `alert` in a <span> inside otherwise-escaped
    // `&lt;img ... onerror=...&gt;` text). Assert against the tag-open sequence specifically.
    assert.ok(!html.includes("<script"), `highlightCode(javascript) left a live <script> in: ${html}`);
    assert.ok(!html.includes("<img"), `highlightCode(javascript) left a live <img> in: ${html}`);
    assert.ok(!html.includes("<svg"), `highlightCode(javascript) left a live <svg> in: ${html}`);
  }
});

test("highlightCode keeps hostile content inert for known non-script languages (html/xml, css)", () => {
  for (const snippet of HOSTILE_SNIPPETS) {
    for (const language of ["html", "xml", "css"]) {
      const html = highlightCode(snippet, language);
      assert.ok(!html.includes("<script"), `highlightCode(${language}) left a live <script> in: ${html}`);
      assert.ok(!html.includes("<img"), `highlightCode(${language}) left a live <img> in: ${html}`);
      assert.ok(!html.includes("<svg"), `highlightCode(${language}) left a live <svg> in: ${html}`);
    }
  }
});

test("highlightCode falls back to plain escaping for an unknown/unregistered language rather than passing content through raw", () => {
  const html = highlightCode("<script>alert(1)</script>", "not-a-real-language");
  assert.equal(html, escapeHtml("<script>alert(1)</script>"));
  assert.ok(!html.includes("<script"));
});

// PERF-005: a real regression budget, not a security assertion. `CodeBlock` in
// `MarkdownMessage.tsx` calls this on every code block a message contains once it stops
// streaming (and, while streaming, is now skipped entirely — see that component's own
// `isStreaming` handling) — an accidental algorithmic regression here (e.g. quadratic
// tokenizing behavior) would degrade every long code-bearing response. 2,000 lines is larger
// than any code block a real chat response is likely to contain; the budget is generous
// specifically to avoid CI hardware-variance flakiness while still catching a real regression
// (a correct implementation finishes this in low tens of milliseconds locally).
test("highlightCode stays within a bounded time budget for a large code block", () => {
  const lines: string[] = [];
  for (let index = 0; index < 2000; index += 1) {
    lines.push(`function handler_${index}(value: number): number {`);
    lines.push(`  const doubled = value * 2; // line ${index}`);
    lines.push(`  return doubled + ${index};`);
    lines.push(`}`);
  }
  const code = lines.join("\n");

  const started = performance.now();
  const html = highlightCode(code, "typescript");
  const elapsedMs = performance.now() - started;

  assert.ok(html.length > 0);
  assert.ok(elapsedMs < 2000, `highlightCode took ${elapsedMs.toFixed(1)}ms for a 2,000-line block, expected <2000ms`);
});
