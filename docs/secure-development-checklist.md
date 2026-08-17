# Secure development checklist

A practical checklist for the maintainer to actually run through before merging a
security-relevant change or cutting a release — not a compliance document nobody reads. Most of
these are already enforced by CI checks; this page exists so the *reasoning* behind each one is
in one place, not just the automated gate.

## Before merging any change that touches...

**File paths or user-supplied paths**
- Does it canonicalize/validate the path the same way `validation.rs`'s existing checks do
  (absolute path; no `.`/`..` or NUL; canonical existing ancestor; expected file/directory type)?
- Does it use the matching shared validator rather than open the IPC string directly? Existing
  input files and output-file leaves reject symlinks via `symlink_metadata`; directory aliases are
  resolved to and persisted as their canonical target so normal platform aliases remain usable.
- If the operation is root-scoped (for example a future repository tool), does it compare the
  canonical result to that intended root? Canonicalization alone does not establish authorization.

**Anything that could reach the network**
- Does the destination go through SEC-001's Rust-side classification (`security::classify_destination`)
  rather than trusting a provider's own "is this local" claim?
- If it's a new local HTTP surface (like the companion API), does it authenticate via a custom
  header — never a cookie — per SEC-010's reasoning?

**Secrets or credentials**
- Does `pnpm secret-boundary:check` still pass? (Run it locally — it's fast.) If you're adding a
  new place a credential could leak (a new log line, a new export field, a new diagnostics
  field), that script needs a new assertion, not just a mental note.
- Is the raw value ever `Debug`- or `Serialize`-derived anywhere in its type? It shouldn't be.

**Model files or imported content**
- Does it go through the existing GGUF header/size/symlink validation, or the existing
  import-size/depth/schema-version bounds (`import_export.rs`)? A new ingestion path needs the
  same category of check, not a fresh design.

**Markdown or any rendered model output**
- Does `pnpm markdown-safety:check` still pass? If you're adding a new place that renders
  HTML-ish content, it needs the same hostile-fixture test treatment `highlightCode.test.ts`
  already established, not an assumption that a library "probably escapes it."

**The Content-Security-Policy or Tauri capabilities**
- Does `pnpm csp:check` still pass? Any new capability grant in `src-tauri/capabilities/*.json`
  should be the narrowest one that actually works (see how the opener plugin was granted
  `allow-open-url` + `allow-default-urls` specifically, not the broader `default` permission
  set) — and should have a one-line reason in the commit for why it's needed.

**Anything that could become a tool a model can call**
- Does it declare a capability scope per `docs/adr/0002-tool-capability-and-prompt-injection-policy.md`
  (read/write/network/secret/data, chat-safe vs. repository-execution tier)? Building a tool
  without going through `tool_policy.rs`'s types is the exact drift that ADR exists to prevent.

## Before cutting a release (OPS-004)

- Does `cargo audit` / `pnpm audit` report zero unreviewed vulnerabilities (not just zero
  exceptions — actually re-read `docs/dependency-advisory-review.md`'s current state)?
- Has anything changed in `src-tauri/capabilities/`, `tauri.conf.json`'s CSP, or the companion
  API's auth surface since the last release? If so, it needs a one-line note in the release
  description — a "security delta" review doesn't need to be a formal document at this scale,
  but it does need to actually happen and be visible, not skipped because nothing *seemed* to
  change.
- Does `pnpm supply-chain:check` still report a current, non-stale SBOM?
