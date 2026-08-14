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
