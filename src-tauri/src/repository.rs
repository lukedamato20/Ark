//! CODE-003: the filesystem boundary for a Project's optional Ark Code Repository.
//!
//! Ark's storage Workspace is private application data. A Repository is a user-selected
//! codebase and is deliberately kept separate. Every Ark Code filesystem tool must resolve
//! user/model-provided paths through this module and then operate on the returned canonical
//! path; accepting an unchecked path again after resolution would defeat this boundary.

use crate::errors::AppError;
use std::path::{Component, Path, PathBuf};
use uuid::Uuid;

const REPOSITORY_PROBE_PREFIX: &str = ".ark-repository-probe-";

/// Validates a new Project-to-Repository binding and returns the canonical path to persist.
///
/// The directory must already exist and be writable. Ark never creates a Repository from a
/// typo. A Repository may not equal, contain, or be contained by the storage Workspace: this
/// prevents a future repository-scoped tool from reaching Ark's database, backups, or private
/// attachment data through an overly broad binding.
pub fn validate_repository_root(
    raw_path: &str,
    storage_workspace_root: &Path,
) -> Result<PathBuf, AppError> {
    let repository_root =
        crate::validation::validate_existing_directory_path(raw_path, "Repository path")?;
    let storage_workspace_root =
        crate::validation::canonicalize_for_use(storage_workspace_root, "Workspace path")?;

    if repository_root.starts_with(&storage_workspace_root)
        || storage_workspace_root.starts_with(&repository_root)
    {
        return Err(AppError::new(
            "repository_workspace_overlap",
            "Repository must be separate from Ark's storage Workspace and may not contain it.",
        ));
    }

    let probe_name = format!("{REPOSITORY_PROBE_PREFIX}{}", Uuid::new_v4());
    crate::validation::probe_writable_directory(
        &repository_root,
        &probe_name,
        "Repository",
        "repository",
    )?;
    Ok(repository_root)
}

/// Resolves an existing relative path beneath a canonical Repository root.
///
/// Model output is untrusted. Absolute paths, traversal syntax, NULs, and symlinks escaping the
/// Repository all fail closed. `""` and `"."` explicitly mean the Repository root, which keeps
/// root directory-listing calls ergonomic without allowing `./` components in arbitrary paths.
pub fn resolve_existing_repository_path(
    repository_root: &Path,
    relative_path: &str,
) -> Result<PathBuf, AppError> {
    // The root was canonical and non-symlinked when bound. Recheck that identity boundary on
    // every operation so replacing the directory with a symlink does not silently widen access.
    let root_metadata = std::fs::symlink_metadata(repository_root).map_err(|_| {
        AppError::new(
            "repository_unavailable",
            "The bound Repository is no longer available.",
        )
    })?;
    if root_metadata.file_type().is_symlink() || !root_metadata.is_dir() {
        return Err(AppError::new(
            "repository_unavailable",
            "The bound Repository is no longer a real directory. Rebind it before using Ark Code.",
        ));
    }
    let canonical_root =
        crate::validation::canonicalize_for_use(repository_root, "Repository path")?;
    if relative_path.contains('\0') {
        return Err(invalid_repository_path());
    }
    if relative_path.is_empty() || relative_path == "." {
        return Ok(canonical_root);
    }

    let relative = Path::new(relative_path);
    if relative.is_absolute()
        || relative
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(invalid_repository_path());
    }

    let candidate = canonical_root.join(relative);
    let canonical_candidate =
        crate::validation::canonicalize_for_use(&candidate, "Repository path").map_err(
            |error| {
                if !candidate.exists() {
                    AppError::new(
                        "repository_path_not_found",
                        "The requested Repository path was not found.",
                    )
                } else {
                    error
                }
            },
        )?;
    if !canonical_candidate.starts_with(&canonical_root) {
        return Err(AppError::new(
            "repository_path_escape",
            "The requested path resolves outside the bound Repository.",
        ));
    }
    Ok(canonical_candidate)
}

fn invalid_repository_path() -> AppError {
    AppError::new(
        "invalid_repository_path",
        "Repository paths must be relative and may not contain traversal components.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_dir(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("ark-repository-{name}-{}", Uuid::new_v4()))
    }

    #[test]
    fn validates_real_directory_and_leaves_no_probe() {
        let repository = temp_dir("valid");
        let workspace = temp_dir("workspace");
        fs::create_dir_all(&repository).expect("repository created");
        fs::create_dir_all(&workspace).expect("workspace created");

        let validated =
            validate_repository_root(repository.to_str().expect("unicode path"), &workspace)
                .expect("repository validates");
        assert_eq!(
            validated,
            crate::validation::canonicalize_for_use(&repository, "Repository").unwrap()
        );
        assert!(fs::read_dir(&repository)
            .expect("repository readable")
            .all(|entry| !entry
                .expect("entry readable")
                .file_name()
                .to_string_lossy()
                .starts_with(REPOSITORY_PROBE_PREFIX)));

        fs::remove_dir_all(repository).expect("repository removed");
        fs::remove_dir_all(workspace).expect("workspace removed");
    }

    #[test]
    fn rejects_missing_file_and_storage_workspace_overlap() {
        let root = temp_dir("overlap");
        let workspace = root.join("ark-data");
        fs::create_dir_all(&workspace).expect("directories created");

        let missing = root.join("missing");
        let error = validate_repository_root(missing.to_str().expect("unicode path"), &workspace)
            .expect_err("missing repository rejected");
        assert_eq!(error.code, "directory_not_found");

        let error = validate_repository_root(root.to_str().expect("unicode path"), &workspace)
            .expect_err("repository containing workspace rejected");
        assert_eq!(error.code, "repository_workspace_overlap");

        let error = validate_repository_root(workspace.to_str().expect("unicode path"), &workspace)
            .expect_err("workspace itself rejected");
        assert_eq!(error.code, "repository_workspace_overlap");

        fs::remove_dir_all(root).expect("root removed");
    }

    #[test]
    fn resolves_only_existing_paths_inside_repository() {
        let repository = temp_dir("resolve");
        let nested = repository.join("src");
        fs::create_dir_all(&nested).expect("nested directory created");
        let file = nested.join("lib.rs");
        fs::write(&file, "fn main() {}\n").expect("file written");

        let resolved = resolve_existing_repository_path(&repository, "src/lib.rs")
            .expect("inside file resolves");
        assert_eq!(
            resolved,
            crate::validation::canonicalize_for_use(&file, "File").unwrap()
        );
        assert_eq!(
            resolve_existing_repository_path(&repository, ".").expect("root resolves"),
            crate::validation::canonicalize_for_use(&repository, "Repository").unwrap()
        );

        for path in ["../outside", "src/../src/lib.rs"] {
            let error = resolve_existing_repository_path(&repository, path)
                .expect_err("traversal syntax rejected");
            assert_eq!(error.code, "invalid_repository_path");
        }
        for path in [
            temp_dir("absolute").to_string_lossy().into_owned(),
            "src\0lib.rs".to_string(),
        ] {
            let error = resolve_existing_repository_path(&repository, &path)
                .expect_err("absolute or NUL path rejected");
            assert_eq!(error.code, "invalid_repository_path");
        }
        let error = resolve_existing_repository_path(&repository, "missing.rs")
            .expect_err("missing path rejected");
        assert_eq!(error.code, "repository_path_not_found");

        fs::remove_dir_all(repository).expect("repository removed");
    }

    #[cfg(unix)]
    #[test]
    fn rejects_a_symlink_that_escapes_repository() {
        use std::os::unix::fs::symlink;

        let repository = temp_dir("symlink-root");
        let outside = temp_dir("symlink-outside");
        fs::create_dir_all(&repository).expect("repository created");
        fs::create_dir_all(&outside).expect("outside created");
        fs::write(outside.join("secret.txt"), "secret").expect("outside file written");
        symlink(&outside, repository.join("escape")).expect("symlink created");

        let error = resolve_existing_repository_path(&repository, "escape/secret.txt")
            .expect_err("symlink escape rejected");
        assert_eq!(error.code, "repository_path_escape");

        let replaced_root = temp_dir("replaced-root");
        fs::create_dir_all(&replaced_root).expect("replaceable root created");
        fs::remove_dir(&replaced_root).expect("replaceable root removed");
        symlink(&outside, &replaced_root).expect("root replacement symlink created");
        let error = resolve_existing_repository_path(&replaced_root, ".")
            .expect_err("replaced root rejected");
        assert_eq!(error.code, "repository_unavailable");

        fs::remove_dir_all(repository).expect("repository removed");
        fs::remove_file(replaced_root).expect("replacement symlink removed");
        fs::remove_dir_all(outside).expect("outside removed");
    }
}
