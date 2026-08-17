-- CODE-003: a Project may bind one user-selected code Repository. This path is deliberately
-- separate from Ark's storage Workspace, is optional, and is persisted only after the command
-- layer validates, probes, and canonicalizes it.
ALTER TABLE projects ADD COLUMN repository_path TEXT;
