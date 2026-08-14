# Diagnostics logs and crash capture

Ark keeps a small local log of its own lifecycle events, separate from any conversation data, to
make support conversations possible without collecting anything sensitive by default. This
document is the retention/consent/revocation disclosure referenced from the app's diagnostics
settings.

## What is logged

Ark logs stable, structural events only: things like "the managed runtime became healthy," "a
generation stream failed (error code X)," or "the workspace database failed to open, falling back
to an in-memory database." Every log line also passes through a redaction pass that strips known
credential/token/cookie shapes, query strings, and absolute filesystem paths before it is ever
written.

Ark never logs:

- Prompt or message content, or any model output.
- Attachment content.
- The raw workspace path (shown redacted, if at all).
- Any secret, API key, or credential value.

This is an architectural guarantee, not just a redaction one — the code that writes log lines is
only ever given stable identifiers (error codes, category names, counts), never user content, so
there is nothing for the redaction pass to need to catch in normal operation. Redaction still runs
as defense in depth.

## Where it lives and how long it's kept

Logs are written to `<app config directory>/logs/ark.log` (the same per-user, per-OS location
`device_settings.json` lives in). The file is capped at 2 MB; once it reaches that size, it is
rotated to `ark.log.1` (overwriting any previous rotation) and a fresh file is started. There is no
longer-term archive — once a rotated file is itself replaced, that history is gone. A separate,
smaller in-memory copy of the most recent entries is also kept for the current session only and is
lost on restart; the on-disk file is what survives a restart or a crash.

## Crash capture (opt-in, off by default)

Settings → Diagnostics bundle has a checkbox: "Capture crash details locally." It is **off by
default**. When off, an uncaught panic is only ever printed to the terminal Ark was launched from
(standard Rust panic behavior) — nothing is written to the log file.

When turned on, an uncaught panic is additionally recorded — redacted, the same as every other log
line — to the local log file described above, so it can be included in a diagnostics bundle after
the app restarts. Turning the checkbox back off takes effect on the very next panic; there is no
delay or separate "confirm" step, matching how the rest of Ark's opt-in toggles work.

**Nothing described on this page is ever transmitted anywhere automatically.** There is no crash
reporting *service* — no Sentry, no telemetry endpoint, no background upload. The only way any of
this data ever leaves the device is the diagnostics bundle export below, which is a manual,
reviewed, user-initiated action every time.

## Diagnostics bundle export

Settings → Diagnostics bundle → "Generate diagnostics bundle" assembles a single text file:
app version, OS/CPU/memory, the managed runtime's current status and recent (already-redacted)
runtime output, and recent (already-redacted) app log lines. The **exact text** is shown in a
read-only box before you can save it anywhere — there is no separate "what actually gets saved"
step that could differ from what you reviewed. Saving writes that exact text, byte for byte, to a
file path you choose. Sharing that file with anyone (e.g. attaching it to a support message) is
always something you do yourself, never something Ark does on your behalf.
