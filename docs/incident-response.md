# Incident response

Right-sized for personal-use software with a single maintainer — this is a runbook one person
can actually follow, not a process written for an on-call rotation that doesn't exist. See
[SECURITY.md](../SECURITY.md) for how to report an issue in the first place.

## Dependency advisory

Handled by the existing process in
[docs/dependency-advisory-review.md](dependency-advisory-review.md): `cargo audit` /
`pnpm audit` run in CI on every push (see `.github/workflows/ci.yml`); a newly disclosed
vulnerability (not just an unmaintained-crate warning) fails the build immediately rather than
waiting for the monthly review cadence. If it fails:

1. Confirm whether Ark's own code path actually reaches the vulnerable function (the same
   reachability-analysis discipline already applied to the 17 existing warnings) — an advisory
   in an unreached code path is lower urgency than one that's actually exercised.
2. If reachable, upgrade the dependency immediately, following the same narrow-diff approach
   used for the SEC-003 `plist`/`quick-xml` upgrade (bump only the affected package and its
   direct chain, not an unrelated broad `cargo update`).
3. If no compatible fix exists yet, treat the affected feature as temporarily unavailable rather
   than shipping known-vulnerable code, and record the exception with an owner and recheck date
   per the existing review document's format.

## Credential exposure

Ark stores provider credentials only as opaque references in SQLite, with the real value in the
OS credential store (SEC-005). If a credential is suspected compromised (e.g. an API key
accidentally logged, committed, or shared):

1. Revoke/rotate the credential at the provider itself first — that is the actual fix; nothing
   Ark does locally un-compromises an already-exposed key.
2. Delete and re-enter the credential in Ark's Settings so the local opaque reference points at
   the new value.
3. Check whether the exposure path is a real Ark defect (e.g. a redaction gap in logs or
   diagnostics) rather than an external accident — if so, that is a real bug, not just an
   incident, and needs a fix plus a regression test (the existing `secret-boundary:check`
   pattern) before being considered closed.

## Workspace-encryption key or recovery-key loss

Documented behavior, not an incident to "respond to" in the traditional sense: per
[docs/data-at-rest.md](data-at-rest.md), Ark cannot recover an encrypted workspace without
either the OS-credential-store key or the recovery key — this is SQLCipher's authenticated
encryption working as intended, not a bug. The only "response" is restoring from an external
backup of the workspace, if one exists.

## Bad release

Per OPS-004's manual release process (no signing, no staged channels, no auto-update — see
`implementation-plan.md`'s OPS-002/OPS-004 entries for why that's the deliberate, right-sized
process for this project's personal-use scope):

1. Do not delete the bad GitHub Release — mark it clearly as broken in its own description so
   anyone who already has the link sees the warning, and to preserve the record of what
   happened.
2. Tell anyone you know installed it directly (this is personal-use software with a handful of
   named users, not a broad audience an announcement channel is needed for).
3. Publish a corrected release. Because OPS-002 keeps at least the previous release's installer
   available, "rollback" for an affected user is simply "install the previous attached file"
   rather than needing an update-channel revocation mechanism that doesn't exist here (Ark has
   no auto-updater to revoke).
4. If the bad release involved a schema migration, check
   `docs/data-at-rest.md`/`implementation-plan.md`'s ARC-005 migration-backup guarantee — the
   pre-migration `.bak` sibling file is the recovery path for anyone who already upgraded.

## Repository/account compromise

If the GitHub account or repository itself is ever compromised (not currently a designed-for
scenario, since there is no CI secret more sensitive than none — no signing key exists under
the current OPS-002 scope):

1. Revoke the compromised credential (GitHub personal access token, SSH key, or password) at
   the source.
2. Review recent commits/releases for anything not authored by the maintainer before trusting
   any artifact published during the compromise window.
3. If a signing key is ever introduced in the future (revisiting the OPS-002 scope decision),
   this section must be revisited at the same time — a real signing-key-compromise procedure
   (revocation, re-signing, user notification) does not exist yet because there is no key to
   compromise yet.
