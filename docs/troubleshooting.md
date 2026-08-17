# Troubleshooting

This page covers the errors you're most likely to actually run into, what causes them, and what
to do about it. Every error Ark shows has a stable internal code even when this page doesn't
mention it explicitly — if you hit something not covered here, see
[If nothing here matches](#if-nothing-here-matches) at the bottom.

Unless a section below says otherwise, an error appears as a dismissible toast at the bottom of
the window with the message text quoted here.

## Ark won't open, or opens to a recovery screen

Ark falls back to a temporary in-memory database rather than refusing to launch, so you'll usually
see either a full recovery screen (nothing else loaded yet) or a banner across the top of an
otherwise-working app. Both offer the same three actions: **Retry**, **Choose workspace** (jumps
to Settings so you can point at a different folder), and **Copy diagnostics**.

- **"The workspace folder is read-only."** — The folder Ark is trying to use isn't writable:
  wrong OS permissions, a read-only mount, or similar. Fix the folder's permissions, or choose a
  different workspace folder.
- **"The local database is locked."** — Another Ark window (or another process) already has the
  database file open. Close other running copies of Ark and retry.
- **"The local database appears to be corrupted or is not a valid Ark database file."** — The
  SQLite file itself is damaged, usually from an unclean shutdown or a disk fault. There's no
  in-app repair tool for this; restore from a backup (Settings → Backup & Restore) or choose a
  different workspace.
- **"The disk is full."** — Free up space on the drive the workspace lives on and retry.
- **"The configured workspace folder no longer exists."** — The workspace was on removable or
  network storage that's now disconnected, or the folder was deleted. Reconnect the drive, or
  choose a different workspace.
- **"A workspace change was interrupted before the new selection was committed."** — Ark was
  closed (crash, power loss) mid-way through switching workspaces. Ark always keeps the *previous*
  selection safe in this case and never guesses — choose a workspace explicitly to clear it.

## The built-in (local) model runtime won't start

Settings → Provider → the built-in runtime card shows a red alert box with a category and message
whenever it fails, plus (for a crash) a short excerpt of the runtime's own recent, redacted log
output.

- **Runtime not installed in a development build** — run
  `scripts/setup-llama.ps1` (Windows) or `scripts/setup-llama.sh` (macOS/Linux) from the repo root,
  then reopen Settings. A qualified packaged release includes its target's verified runtime; this
  message there means the installation is incomplete or damaged and should be reinstalled.
- **Verification failed** — The installed binary or its supporting files don't match Ark's
  reviewed hash manifest. This blocks Start entirely rather than running unverified files —
  re-run the setup script to get a clean install.
- **Model file no longer available / not a valid GGUF file** — The `.gguf` path you selected was
  moved, deleted, or isn't actually a GGUF model. Pick the file again in the "Model file" field.
  If you don't have a model yet, see the [MVP Scope](../README.md#mvp-scope) section.
- **Runtime exited with status code N / after a platform signal** — The process crashed after
  starting, most commonly because the model is too large for available RAM/VRAM. Try a smaller
  model, or check the recent log excerpt shown in the alert for a more specific reason.
- **Did not become ready within 30 seconds / health endpoint unreachable** — The process started
  but never finished loading in time. A very large model on slow storage can genuinely take longer
  than that; also check the log excerpt for a port conflict or an actual crash.
- **Built-in provider absent in a release candidate** — the authenticated isolating proxy is
  complete, but the UI capability remains hidden until Windows/macOS/Linux packaged-build jobs
  have all installed, executed, and bundled the exact reviewed runtime for that release.

## A provider (Ollama or another server) won't connect

- **"Provider is unreachable. Check that Ollama is running."** / **"Provider request timed
  out."** — Ark couldn't reach the configured Base URL at all. Confirm the server is actually
  running and that the URL/port in Settings → Provider is correct.
- **"The provider tried to redirect this request, which Ark blocks..."** — The Base URL points at
  something that issues an HTTP redirect (a misconfigured reverse proxy is a common cause). Ark
  blocks redirects for privacy/security rather than silently following them — fix the Base URL to
  point directly at the real endpoint.
- **A confirmation box appears instead of saving** — Saving a provider pointed at a non-local
  address always asks you to explicitly confirm first (and, for local-only providers, to convert
  the provider to the "Remote" class). This isn't a failure — it's Ark making sure you know
  prompts and conversation history would leave your machine before it saves that configuration.
  Plain HTTP (not HTTPS) to a remote address additionally requires an explicit "development mode"
  acknowledgment, since it would send that data unencrypted.
- **A chat generation fails partway through** — Streaming timeouts or a malformed/incomplete
  response from the provider end the generation with an error status on that message. Use the
  message's own Retry action in the chat view.

## Credentials / API keys won't save, or show "reconnection required"

Settings → Provider → "Operating-system credential storage" shows the health of Ark's connection
to your OS's credential store (Windows Credential Manager, macOS Keychain, or Linux Secret
Service) independently of any single provider.

- **"...credential store is unavailable" / "...is locked or access was denied"** — The OS
  keychain service itself isn't reachable — it may need to be unlocked, or (on Linux) a secret
  service daemon may not be running. Unlock/start it at the OS level, then click **Retry**. The
  credential field for any provider that needs authentication is disabled until this clears.
- **"The credential is no longer present. Reconnect this provider."** — The stored reference is
  still in Ark's database, but the actual credential was deleted from the OS keychain outside of
  Ark (e.g. you cleared your keychain manually). Re-enter the credential and save again.
- Ark never stores raw credential values itself — only an opaque reference to what your OS
  keychain holds — so a restored backup or a workspace copied to another machine will always show
  this "reconnect" state for any authenticated provider. See
  [docs/secrets-and-backups.md](secrets-and-backups.md).

## Workspace encryption won't enable, rotate, or unlock

Settings → Storage → "Workspace encryption" shows a plain error line (no dedicated button) for
anything that fails during Encrypt/Rotate/Decrypt, plus a **Recovery key** field with a **Restore
and unlock** button when the workspace is in a locked state.

- **"The encrypted workspace key is unavailable..."** — Ark can't read the OS-keychain entry that
  holds the current encryption key. Unlock your OS credential store first; if that doesn't resolve
  it, use the recovery key you were shown when encryption was enabled or last rotated.
- **"That recovery key does not unlock this workspace."** — Either the key is wrong, or it's a
  *previous* recovery key that stopped working after a later rotation. Only the most recently
  issued recovery key works. A forgotten key with no working OS-keychain entry is genuinely
  unrecoverable — this is a property of the encryption itself, not a product limitation. See
  [docs/data-at-rest.md](data-at-rest.md).
- **"An interrupted protection change was detected..."** — A previous enable/rotate/disable was
  interrupted (e.g. a crash) partway through. Ark always keeps the pre-change file safe and never
  guesses which state to trust — simply reopen Ark, which reconciles this automatically on
  startup.
- **"Could not install the verified workspace copy... The original was restored."** — The
  copy-verify-swap sequence Ark uses for every protection change failed partway (disk error,
  interrupted write). Your original workspace is untouched and still in its previous mode; retry
  once the underlying issue (usually disk space) is resolved.

## Backup and restore

Settings → Backup & Restore. Both directions verify the resulting file independently before
trusting it — a failed verification deletes the bad copy rather than leaving a silently corrupt
one behind — and neither direction ever touches your live workspace.

- **"...already exists. Choose a different backup destination."** / **"...already contains a
  workspace database."** — Ark never overwrites an existing file at a backup or restore
  destination. Pick an empty or different folder.
- **"...failed its integrity check... and was removed."** — The backup or restored copy didn't
  pass its own post-write verification, so Ark deleted it rather than leaving something that
  looks fine but isn't. Retry — this is usually transient (disk error, interrupted write).
- **Restore preview shows "unsupported schema"** — The backup was made by a newer version of Ark
  than this build understands. The Restore button and target-folder field are hidden in this case
  on purpose; update Ark before restoring that backup.

## Importing a conversation fails

- **"Import file is too large" / "Invalid conversation JSON"** — The file exceeds Ark's import
  size limit, or isn't valid JSON in the expected shape. See
  [docs/import-format.md](import-format.md) for the exact format and limits.
- **Clicking Cancel during an import** — This is not an error: the whole import rolls back
  cleanly and Ark confirms "Import cancelled. No conversation data was written." with no partial
  conversation left behind.

## If nothing here matches

Settings → Diagnostics bundle → **Generate diagnostics bundle** assembles a plain-text summary —
app version, hardware, the managed runtime's status and recent (already redacted) log output, and
recent (already redacted) app log lines — and shows you the *exact* text before you can save it
anywhere. It never includes prompts, model output, attachment content, or your literal workspace
path. Nothing in it is sent anywhere automatically; saving and sharing the file is always
something you choose to do yourself. See [docs/diagnostics-and-logs.md](diagnostics-and-logs.md)
for exactly what is and isn't in it.

If you can reproduce the problem, that bundle plus a short description of what you were doing is
the most useful thing to attach when asking for help.
