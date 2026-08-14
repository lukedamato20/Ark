import assert from "node:assert/strict";
import test from "node:test";
import { checkExternalLink } from "./externalLinks.ts";

test("accepts http, https, mailto, and tel links", () => {
  assert.equal(checkExternalLink("https://example.com/docs").safe, true);
  assert.equal(checkExternalLink("http://example.com").safe, true);
  assert.equal(checkExternalLink("mailto:someone@example.com").safe, true);
  assert.equal(checkExternalLink("tel:+15555550100").safe, true);
});

test("rejects javascript, data, file, and vbscript schemes", () => {
  for (const href of [
    "javascript:alert(document.cookie)",
    "JavaScript:alert(1)",
    "data:text/html,<script>alert(1)</script>",
    "file:///etc/passwd",
    "vbscript:msgbox(1)",
  ]) {
    const result = checkExternalLink(href);
    assert.equal(result.safe, false, `expected "${href}" to be rejected`);
    assert.equal(result.reason, "not a supported link");
  }
});

test("rejects relative paths and malformed URLs, which have no meaning for an external-open action", () => {
  assert.equal(checkExternalLink("/local/path").safe, false);
  assert.equal(checkExternalLink("../escape").safe, false);
  assert.equal(checkExternalLink("not a url at all").safe, false);
  assert.equal(checkExternalLink("").safe, false);
});

test("re-serializes the URL rather than trusting the original string verbatim", () => {
  // A regression guard: the caller should use the parsed/re-serialized `url`, not the raw input,
  // so any parsing normalization (e.g. a trailing default port) is what actually gets opened.
  const result = checkExternalLink("https://example.com");
  assert.equal(result.safe, true);
  assert.equal(result.url, "https://example.com/");
});
