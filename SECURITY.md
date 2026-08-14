# Security policy

Ark is local-first, personal-use software. This policy is right-sized for that scope — it is
honest about what does and doesn't exist rather than describing a process for an audience this
project doesn't have.

## Supported versions

There is one supported version: the latest commit on `main`. Ark does not maintain parallel
release branches or backport fixes to older versions — there is no version-support matrix to
consult, because there is no prior-version user base to support.

## Reporting a vulnerability

Please use [GitHub's private vulnerability reporting](https://docs.github.com/en/code-security/security-advisories/guidance-on-reporting-and-writing/privately-reporting-a-security-vulnerability)
for this repository (Security tab → "Report a vulnerability") rather than a public issue. This
keeps the report private between you and the maintainer until a fix is available, without
requiring a separate email address or contact channel that this document would otherwise have
to invent.

If private vulnerability reporting is not yet enabled on this repository, open a regular issue
stating only that you have a security report to make, without details — the maintainer will
follow up to arrange a private channel.

There is no bug bounty and no formal SLA. As personal-use software with a single maintainer,
response time is best-effort.

## What Ark protects, and what it explicitly does not claim to

See [docs/data-at-rest.md](docs/data-at-rest.md) for the honest threat model around local data
(what full-disk encryption covers vs. Ark's optional workspace encryption, and what neither
covers — malware already running as you, or a fully privileged local account). See
[docs/secrets-and-backups.md](docs/secrets-and-backups.md) for how provider credentials are
stored and what happens to them on export/restore. Ark does not claim protection it cannot
actually provide — if a document above states a limitation, that limitation is real, not
overly cautious phrasing.

## Advisory and dependency review

Rust/npm dependency advisories are reviewed on the cadence and process documented in
[docs/dependency-advisory-review.md](docs/dependency-advisory-review.md) — every accepted
warning has a named owner, a reachability analysis, and a recheck date; there are no blanket
exceptions.

## Incident response

See [docs/incident-response.md](docs/incident-response.md) for what happens if a dependency
advisory, a credential exposure, or a bad release needs to be handled after the fact.
