# Ark Code read-only Repository tools

CODE-004's first Ark Code slice is investigation-only. It uses the same capability vocabulary as
Ark Chat tools, but a separate registry whose six definitions are all
`repository_execution`/read-only, with no write, network, or secret axis:

- `list_directory`
- `read_file`
- `search`
- `git_status`
- `git_diff`
- `repository_map`

Every invocation names a Project, never an arbitrary root. The backend loads that Project's
canonical Repository binding, verifies it still exists as a real directory, applies the SEC-009
tier check, and resolves model-supplied relative paths through the CODE-003 containment boundary.
Ark Chat's `list_tools` response never includes these tools.

## Current app surface

Ark Code is a separate `code` view, reachable from the main sidebar and switchable back to Ark
Chat without merging their tool surfaces. A session belongs to an existing Project and persists in
the Workspace database. The current view exposes the bounded read-only Repository map, literal
search, file read, Git status, and Git diff operations as inspection cards, so CODE-004 can be
tested in the packaged application without granting write or command capability.

The provider-driven agent loop and resume/recovery UI are still CODE-007 work in progress. The UI
does not present a queued durable run as if it were executing; until that loop is connected, this
surface is a manual read-only inspector rather than a complete autonomous Ark Code agent.

## Enumeration and context limits

Traversal uses `.gitignore`/`.ignore` rules even before `git init`, never follows symlinks, and
never enters `.git`. Automatic context includes UTF-8 text files no larger than 1 MiB. Binary,
non-UTF-8, oversized, unreadable, ignored, and Git-metadata files are excluded rather than placed
in model context.

Directory listings return at most 500 direct entries. File reads return at most 400 lines and
128 KiB per call, with explicit `truncated`/`nextStartLine` fields and a raw-byte SHA-256 for later
staleness checks. Literal search returns at most 500 matches, scans at most 10,000 eligible files
and 32 MiB per call, bounds each line preview, and reports scanned/skipped counts and truncation.
Repository maps return at most 2,000 directories/context-eligible files and never include file
contents.

## Git inspection

Git status and staged/unstaged diff are read-only child processes with a stripped environment,
disabled hooks/fsmonitor/external diff/textconv behavior, optional locks disabled, null stdin,
10-second timeout, kill-on-drop, and bounded stdout/stderr. Ark pins `GIT_DIR` and `GIT_WORK_TREE`
to the validated Repository so Git cannot discover a parent repository.

V1 accepts only an ordinary `.git` directory inside the bound Repository. A `.git` file, symlink,
linked worktree whose metadata lives elsewhere, or parent-only repository fails closed. Supporting
external Git metadata would contradict CODE-004's strict Repository-root boundary and requires a
separate reviewed scope decision.

## Trust boundary

Repository content, search matches, file reads, and diffs are untrusted tool results. The agent
loop must place them only in `ProviderChatRequest.untrusted_context`; they never become Ark
system instructions and cannot grant another capability. CODE-005 write/edit/command tools are
not part of this surface.
