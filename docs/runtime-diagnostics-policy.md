# Managed-runtime diagnostic data policy

Ark keeps managed-runtime stdout/stderr only in memory. The rotating buffer retains at most 200
lines and 128 KiB, truncates any one line to 2,048 characters, and is destroyed when the app
process exits.

Before a line enters that buffer Ark replaces:

- the per-launch runtime bearer token;
- the configured model path and managed binary path, including normalized slash variants;
- values following common authorization, bearer, API-key, and token markers; and
- other whitespace-delimited absolute Windows or Unix path tokens.

Runtime status and readiness failures may show up to five already-redacted lines when they are
needed to explain why launch failed. General diagnostics omit log lines by default. The user must
select “Include recent managed-runtime log lines” before a diagnostic result can contain up to 50
redacted lines. Structured diagnostics expose only whether a model is configured, never its path
or the bearer token.

Redaction reduces accidental disclosure but cannot prove that arbitrary native-runtime output is
non-sensitive. The consent control therefore remains off by default, and future diagnostic
export/crash-report work must preserve that choice rather than enabling logs implicitly.

This page covers only the managed runtime's own stdout/stderr buffer. Ark's separate, app-level
structured log (lifecycle events, error codes — never runtime output or user content) and its
opt-in crash capture are documented in
[docs/diagnostics-and-logs.md](diagnostics-and-logs.md); both share the same redaction pass this
page describes (`redaction.rs`).
