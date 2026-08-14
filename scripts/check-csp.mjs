import { readFileSync } from "node:fs";
import { join } from "node:path";

const root = join(import.meta.dirname, "..");
const read = (path) => readFileSync(join(root, path), "utf8");

function assert(condition, message) {
  if (!condition) throw new Error(`CSP check failed: ${message}`);
}

// SEC-008: locks in the production Content-Security-Policy so a future change to
// tauri.conf.json cannot silently weaken it — this check exists specifically because the plan
// treats the current CSP/webview behavior as an audited strength that later features could
// accidentally regress, not because tauri.conf.json changes often on its own.
const tauriConfig = JSON.parse(read("src-tauri/tauri.conf.json"));
const csp = tauriConfig.app?.security?.csp;
assert(typeof csp === "string" && csp.length > 0, "app.security.csp must be a non-empty string");

const directives = new Map(
  csp
    .split(";")
    .map((part) => part.trim())
    .filter(Boolean)
    .map((part) => {
      const [name, ...values] = part.split(/\s+/);
      return [name, values];
    }),
);

function directive(name) {
  const values = directives.get(name);
  assert(values !== undefined, `CSP must declare a "${name}" directive`);
  return values;
}

// script-src must never regress to allow inline/eval'd script or an external host — this is the
// one directive standing between untrusted model/imported content and script execution.
const scriptSrc = directive("script-src");
assert(scriptSrc.includes("'self'"), "script-src must include 'self'");
assert(!scriptSrc.includes("'unsafe-inline'"), "script-src must never include 'unsafe-inline'");
assert(!scriptSrc.includes("'unsafe-eval'"), "script-src must never include 'unsafe-eval'");
assert(scriptSrc.every((value) => value === "'self'"), "script-src must be limited to 'self' only, no external hosts");

// style-src's 'unsafe-inline' is a documented, deliberate retention (SEC-008), not an oversight:
// framer-motion (used throughout the UI, e.g. the sidebar collapse/expand animation) applies
// animated values as direct inline `style` property writes, which CSP's `unsafe-inline` is what
// permits. Removing it would break that library's core animation mechanism; a nonce-based
// approach doesn't fit a static Tauri CSP the way it would a server-rendered page with a
// per-request nonce. If this ever changes, this assertion (and the comment above it) must change
// together with a real replacement mechanism, not be silently dropped.
const styleSrc = directive("style-src");
assert(styleSrc.includes("'self'"), "style-src must include 'self'");
assert(styleSrc.includes("'unsafe-inline'"), "style-src's 'unsafe-inline' retention must stay intentional and documented, not silently removed");
assert(!styleSrc.includes("'unsafe-eval'"), "style-src must never include 'unsafe-eval'");

// Defense-in-depth directives Ark does not need loosened for any current feature: no <object>/
// <embed>/<applet> plugin content, no <base> tag rewriting, no form submission anywhere in the
// app. Cheap to hold at their strictest value; regressing any of them needs a real feature reason.
assert(directive("object-src").includes("'none'"), "object-src must be 'none' — Ark never uses <object>/<embed>/<applet>");
assert(directive("base-uri").every((value) => value === "'self'"), "base-uri must be 'self' only");
assert(directive("form-action").every((value) => value === "'self'"), "form-action must be 'self' only");

// connect-src is intentionally loopback-only (local providers/sidecar); a public host here would
// mean the webview itself — not just Rust's SEC-001 destination policy — could reach the network.
const connectSrc = directive("connect-src");
assert(
  connectSrc.every((value) => /^https?:\/\/(127\.0\.0\.1|localhost):\*$/.test(value)),
  "connect-src must stay limited to loopback hosts; the webview itself must never be allowed a public network destination",
);

console.log(`CSP check passed: ${directives.size} directives verified, script-src has no unsafe-inline/unsafe-eval, style-src's unsafe-inline retention is intentional and documented.`);
