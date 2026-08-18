//! CODE-005 Git isolation and lifecycle primitives.
//!
//! Ark Code never edits a Project's bound checkout directly. Each session receives a private
//! clone under Ark's workspace and a dedicated branch. Later checkpoints, rollbacks, and command
//! execution are therefore unable to alter the user's branch, index, or dirty working tree.

use crate::code_tools::RepositoryContext;
use crate::errors::AppError;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;
use tokio::io::AsyncReadExt;

const CLONE_TIMEOUT: Duration = Duration::from_secs(120);
const CLONE_OUTPUT_LIMIT: usize = 64 * 1024;
const MANAGED_REPOSITORIES_DIRECTORY: &str = "ark-code-repositories";
pub const CHECKPOINT_TOOL_ID: &str = "git_checkpoint";
pub const ROLLBACK_TOOL_ID: &str = "git_rollback";

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GitCheckpointArguments {
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct GitCheckpointPreview {
    pub arguments_json: String,
    pub content: String,
    pub call_hash: String,
    pub preview_hash: String,
    pub precondition_hash: String,
    pub head_oid: String,
    pub tree_oid: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitCheckpointOutcome {
    pub commit_oid: String,
    pub parent_commit_oid: String,
    pub tree_oid: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitCheckpointVerification {
    pub outcome: crate::code_sessions::CodeRecoveryOutcome,
    pub observed_head_oid: Option<String>,
    pub observed_tree_oid: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GitRollbackArguments {
    pub checkpoint_id: String,
}

#[derive(Debug, Clone)]
pub struct GitRollbackPreview {
    pub arguments_json: String,
    pub content: String,
    pub call_hash: String,
    pub preview_hash: String,
    pub precondition_hash: String,
    pub checkpoint_id: String,
    pub target_commit_oid: String,
    pub before_head_oid: String,
    pub before_tree_oid: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitRollbackOutcome {
    pub checkpoint_id: String,
    pub restored_commit_oid: String,
    pub previous_head_oid: String,
}

pub async fn preview_checkpoint(
    context: &RepositoryContext,
    arguments: GitCheckpointArguments,
) -> Result<GitCheckpointPreview, AppError> {
    let message = arguments.message.trim();
    if message.is_empty() || message.chars().count() > 200 || message.contains(['\r', '\n']) {
        return Err(AppError::invalid_input(
            "Checkpoint message must be a single line between 1 and 200 characters.",
        ));
    }
    let arguments = GitCheckpointArguments {
        message: message.to_string(),
    };
    let arguments_json = crate::code_sessions::serialize_json(&arguments)?;
    let head_oid = git_oid(context, &["rev-parse", "--verify", "HEAD"]).await?;
    let status = run_managed_git(
        context,
        &["status", "--short", "--untracked-files=all"],
        Duration::from_secs(15),
    )
    .await?;
    if status.trim().is_empty() {
        return Err(AppError::new(
            "git_checkpoint_no_changes",
            "There are no Repository changes to checkpoint.",
        ));
    }
    let tree_oid = build_proposed_tree(context).await?;
    let diff = run_managed_git(
        context,
        &[
            "diff-tree",
            "-p",
            "--binary",
            "--no-ext-diff",
            &head_oid,
            &tree_oid,
        ],
        Duration::from_secs(20),
    )
    .await?;
    let content = format!(
        "Checkpoint: {message}\nParent: {head_oid}\nProposed tree: {tree_oid}\n\nChanged files:\n{status}\nDiff:\n{diff}"
    );
    let call_hash = crate::code_sessions::compute_call_hash(&arguments)?;
    let preview_hash = crate::code_sessions::compute_preview_hash(&content);
    let precondition_hash = crate::code_sessions::compute_precondition_hash(&serde_json::json!({
        "headOid": head_oid,
        "treeOid": tree_oid,
    }))?;
    Ok(GitCheckpointPreview {
        arguments_json,
        content,
        call_hash,
        preview_hash,
        precondition_hash,
        head_oid,
        tree_oid,
        message: message.to_string(),
    })
}

pub async fn execute_checkpoint(
    context: &RepositoryContext,
    approved: &GitCheckpointPreview,
) -> Result<GitCheckpointOutcome, AppError> {
    let fresh = preview_checkpoint(
        context,
        GitCheckpointArguments {
            message: approved.message.clone(),
        },
    )
    .await?;
    if fresh.call_hash != approved.call_hash
        || fresh.preview_hash != approved.preview_hash
        || fresh.precondition_hash != approved.precondition_hash
        || fresh.head_oid != approved.head_oid
        || fresh.tree_oid != approved.tree_oid
    {
        return Err(AppError::new(
            "git_checkpoint_approval_stale",
            "Repository state changed after this checkpoint was reviewed.",
        ));
    }
    let commit_oid = run_managed_git_with_env(
        context,
        &[
            "commit-tree",
            &approved.tree_oid,
            "-p",
            &approved.head_oid,
            "-m",
            &approved.message,
        ],
        Duration::from_secs(20),
        &[
            ("GIT_AUTHOR_NAME", "Ark Code"),
            ("GIT_AUTHOR_EMAIL", "ark-code@local.invalid"),
            ("GIT_COMMITTER_NAME", "Ark Code"),
            ("GIT_COMMITTER_EMAIL", "ark-code@local.invalid"),
        ],
    )
    .await?
    .trim()
    .to_string();
    validate_oid(&commit_oid)?;
    let branch = run_managed_git(context, &["symbolic-ref", "HEAD"], Duration::from_secs(10))
        .await?
        .trim()
        .to_string();
    run_managed_git(
        context,
        &["update-ref", &branch, &commit_oid, &approved.head_oid],
        Duration::from_secs(10),
    )
    .await?;
    // The ref move above is the externally meaningful atomic operation. Aligning the private
    // clone's index afterward never affects the user's checkout.
    run_managed_git(
        context,
        &["reset", "--mixed", &commit_oid],
        Duration::from_secs(20),
    )
    .await?;
    let observed_head = git_oid(context, &["rev-parse", "--verify", "HEAD"]).await?;
    let observed_tree = git_oid(context, &["rev-parse", "--verify", "HEAD^{tree}"]).await?;
    if observed_head != commit_oid || observed_tree != approved.tree_oid {
        return Err(AppError::new(
            "git_checkpoint_verification_failed",
            "Ark could not verify the approved Git checkpoint.",
        ));
    }
    Ok(GitCheckpointOutcome {
        commit_oid,
        parent_commit_oid: approved.head_oid.clone(),
        tree_oid: approved.tree_oid.clone(),
        message: approved.message.clone(),
    })
}

pub async fn verify_checkpoint(
    context: &RepositoryContext,
    before_head_oid: &str,
    expected_tree_oid: &str,
) -> GitCheckpointVerification {
    let observed_head_oid = git_oid(context, &["rev-parse", "--verify", "HEAD"])
        .await
        .ok();
    let observed_tree_oid = git_oid(context, &["rev-parse", "--verify", "HEAD^{tree}"])
        .await
        .ok();
    let outcome = if observed_head_oid.as_deref() == Some(before_head_oid) {
        crate::code_sessions::CodeRecoveryOutcome::NotApplied
    } else if observed_tree_oid.as_deref() == Some(expected_tree_oid) {
        crate::code_sessions::CodeRecoveryOutcome::Applied
    } else if observed_head_oid.is_some() && observed_tree_oid.is_some() {
        crate::code_sessions::CodeRecoveryOutcome::Diverged
    } else {
        crate::code_sessions::CodeRecoveryOutcome::Unknown
    };
    GitCheckpointVerification {
        outcome,
        observed_head_oid,
        observed_tree_oid,
    }
}

pub async fn preview_rollback(
    context: &RepositoryContext,
    arguments: GitRollbackArguments,
    target_commit_oid: &str,
    base_commit_oid: &str,
    ark_checkpoint_oids: &[String],
) -> Result<GitRollbackPreview, AppError> {
    crate::validation::validate_entity_id(&arguments.checkpoint_id, "Git checkpoint ID")?;
    validate_oid(target_commit_oid)?;
    validate_oid(base_commit_oid)?;
    let before_head_oid = git_oid(context, &["rev-parse", "--verify", "HEAD"]).await?;
    let history = run_managed_git(
        context,
        &["rev-list", "--first-parent", "HEAD"],
        Duration::from_secs(15),
    )
    .await?;
    let commits: Vec<&str> = history.lines().collect();
    let base_position = commits
        .iter()
        .position(|oid| *oid == base_commit_oid)
        .ok_or_else(|| {
            AppError::new(
                "git_rollback_history_diverged",
                "Ark Code's isolated branch no longer descends from its recorded baseline.",
            )
        })?;
    let target_position = commits
        .iter()
        .position(|oid| *oid == target_commit_oid)
        .ok_or_else(|| {
            AppError::new(
                "git_rollback_target_unreachable",
                "The selected Ark Code checkpoint is not in the current branch history.",
            )
        })?;
    if target_position >= base_position {
        return Err(AppError::new(
            "git_rollback_target_invalid",
            "Rollback may target only a recorded Ark Code checkpoint after the session baseline.",
        ));
    }
    let allowed: std::collections::HashSet<&str> =
        ark_checkpoint_oids.iter().map(String::as_str).collect();
    if commits[..base_position]
        .iter()
        .any(|commit| !allowed.contains(commit))
    {
        return Err(AppError::new(
            "git_rollback_unowned_commit",
            "The isolated branch contains a commit Ark Code did not create; rollback was refused.",
        ));
    }
    let before_tree_oid = build_proposed_tree(context).await?;
    let target_tree_oid = git_oid(
        context,
        &[
            "rev-parse",
            "--verify",
            &format!("{target_commit_oid}^{{tree}}"),
        ],
    )
    .await?;
    if before_head_oid == target_commit_oid && before_tree_oid == target_tree_oid {
        return Err(AppError::new(
            "git_rollback_no_changes",
            "The Repository already matches this checkpoint.",
        ));
    }
    let status = run_managed_git(
        context,
        &["status", "--short", "--untracked-files=all"],
        Duration::from_secs(15),
    )
    .await?;
    let diff = run_managed_git(
        context,
        &[
            "diff-tree",
            "-p",
            "--binary",
            "--no-ext-diff",
            &target_tree_oid,
            &before_tree_oid,
        ],
        Duration::from_secs(20),
    )
    .await?;
    let arguments_json = crate::code_sessions::serialize_json(&arguments)?;
    let content = format!(
        "Rollback to checkpoint {}\nTarget commit: {target_commit_oid}\nCurrent commit: {before_head_oid}\n\nUncommitted status:\n{status}\nChanges that will be removed:\n{diff}",
        arguments.checkpoint_id
    );
    let call_hash = crate::code_sessions::compute_call_hash(&arguments)?;
    let preview_hash = crate::code_sessions::compute_preview_hash(&content);
    let precondition_hash = crate::code_sessions::compute_precondition_hash(&serde_json::json!({
        "headOid": before_head_oid,
        "treeOid": before_tree_oid,
        "targetCommitOid": target_commit_oid,
    }))?;
    Ok(GitRollbackPreview {
        arguments_json,
        content,
        call_hash,
        preview_hash,
        precondition_hash,
        checkpoint_id: arguments.checkpoint_id,
        target_commit_oid: target_commit_oid.to_string(),
        before_head_oid,
        before_tree_oid,
    })
}

pub async fn execute_rollback(
    context: &RepositoryContext,
    approved: &GitRollbackPreview,
    base_commit_oid: &str,
    ark_checkpoint_oids: &[String],
) -> Result<GitRollbackOutcome, AppError> {
    let fresh = preview_rollback(
        context,
        GitRollbackArguments {
            checkpoint_id: approved.checkpoint_id.clone(),
        },
        &approved.target_commit_oid,
        base_commit_oid,
        ark_checkpoint_oids,
    )
    .await?;
    if fresh.call_hash != approved.call_hash
        || fresh.preview_hash != approved.preview_hash
        || fresh.precondition_hash != approved.precondition_hash
    {
        return Err(AppError::new(
            "git_rollback_approval_stale",
            "Repository state changed after this rollback was reviewed.",
        ));
    }
    run_managed_git(
        context,
        &["reset", "--hard", &approved.target_commit_oid],
        Duration::from_secs(30),
    )
    .await?;
    run_managed_git(context, &["clean", "-fdx", "--"], Duration::from_secs(30)).await?;
    let observed_head = git_oid(context, &["rev-parse", "--verify", "HEAD"]).await?;
    let status = run_managed_git(
        context,
        &["status", "--porcelain=v1", "--untracked-files=all"],
        Duration::from_secs(15),
    )
    .await?;
    if observed_head != approved.target_commit_oid || !status.is_empty() {
        return Err(AppError::new(
            "git_rollback_verification_failed",
            "Ark could not verify the approved rollback exactly.",
        ));
    }
    Ok(GitRollbackOutcome {
        checkpoint_id: approved.checkpoint_id.clone(),
        restored_commit_oid: observed_head,
        previous_head_oid: approved.before_head_oid.clone(),
    })
}

pub async fn verify_rollback(
    context: &RepositoryContext,
    before_head_oid: &str,
    target_commit_oid: &str,
) -> crate::code_sessions::CodeRecoveryOutcome {
    match git_oid(context, &["rev-parse", "--verify", "HEAD"]).await {
        Ok(observed) if observed == target_commit_oid => {
            crate::code_sessions::CodeRecoveryOutcome::Applied
        }
        Ok(observed) if observed == before_head_oid => {
            crate::code_sessions::CodeRecoveryOutcome::NotApplied
        }
        Ok(_) => crate::code_sessions::CodeRecoveryOutcome::Diverged,
        Err(_) => crate::code_sessions::CodeRecoveryOutcome::Unknown,
    }
}

pub fn verify_checkpoint_sync(
    repository_root: &str,
    before_head_oid: &str,
    expected_tree_oid: &str,
) -> GitCheckpointVerification {
    let context = RepositoryContext::from_run_snapshot(repository_root);
    let (observed_head_oid, observed_tree_oid) = match context {
        Ok(context) => (
            run_managed_git_sync(&context, &["rev-parse", "--verify", "HEAD"]).ok(),
            run_managed_git_sync(&context, &["rev-parse", "--verify", "HEAD^{tree}"]).ok(),
        ),
        Err(_) => (None, None),
    };
    let outcome = if observed_head_oid.as_deref() == Some(before_head_oid) {
        crate::code_sessions::CodeRecoveryOutcome::NotApplied
    } else if observed_tree_oid.as_deref() == Some(expected_tree_oid) {
        crate::code_sessions::CodeRecoveryOutcome::Applied
    } else if observed_head_oid.is_some() && observed_tree_oid.is_some() {
        crate::code_sessions::CodeRecoveryOutcome::Diverged
    } else {
        crate::code_sessions::CodeRecoveryOutcome::Unknown
    };
    GitCheckpointVerification {
        outcome,
        observed_head_oid,
        observed_tree_oid,
    }
}

pub fn verify_rollback_sync(
    repository_root: &str,
    before_head_oid: &str,
    target_commit_oid: &str,
) -> crate::code_sessions::CodeRecoveryOutcome {
    let observed = RepositoryContext::from_run_snapshot(repository_root)
        .and_then(|context| run_managed_git_sync(&context, &["rev-parse", "--verify", "HEAD"]));
    match observed {
        Ok(oid) if oid == target_commit_oid => crate::code_sessions::CodeRecoveryOutcome::Applied,
        Ok(oid) if oid == before_head_oid => crate::code_sessions::CodeRecoveryOutcome::NotApplied,
        Ok(_) => crate::code_sessions::CodeRecoveryOutcome::Diverged,
        Err(_) => crate::code_sessions::CodeRecoveryOutcome::Unknown,
    }
}

fn run_managed_git_sync(context: &RepositoryContext, args: &[&str]) -> Result<String, AppError> {
    let git_directory = validate_source_git_directory(context)?;
    let null_device = if cfg!(windows) { "NUL" } else { "/dev/null" };
    let mut command = std::process::Command::new("git");
    crate::process_window::hide_std_process_window(&mut command);
    command
        .arg("--no-pager")
        .arg("--literal-pathspecs")
        .arg("-c")
        .arg("core.fsmonitor=false")
        .arg("-c")
        .arg(format!("core.hooksPath={null_device}"))
        .arg("-C")
        .arg(context.root())
        .args(args)
        .env_clear()
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", null_device)
        .env("GIT_DIR", git_directory)
        .env("GIT_WORK_TREE", context.root())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    for name in ["PATH", "SystemRoot", "WINDIR", "TEMP", "TMP"] {
        if let Some(value) = std::env::var_os(name) {
            command.env(name, value);
        }
    }
    let mut child = command.spawn().map_err(|_| git_failed())?;
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    let status = loop {
        if let Some(status) = child.try_wait().map_err(|_| git_failed())? {
            break status;
        }
        if std::time::Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(AppError::new(
                "git_timeout",
                "Git recovery verification timed out.",
            ));
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    if !status.success() {
        return Err(git_failed());
    }
    let mut output = String::new();
    child
        .stdout
        .take()
        .ok_or_else(git_failed)?
        .take(256)
        .read_to_string(&mut output)
        .map_err(|_| git_failed())?;
    let output = output.trim().to_string();
    validate_oid(&output)?;
    Ok(output)
}

async fn build_proposed_tree(context: &RepositoryContext) -> Result<String, AppError> {
    let index = context
        .root()
        .join(".git")
        .join(format!("ark-preview-index-{}", uuid::Uuid::new_v4()));
    let index_text = path_text(&index)?.to_string();
    let env = [("GIT_INDEX_FILE", index_text.as_str())];
    let result = async {
        run_managed_git_with_env(
            context,
            &["read-tree", "HEAD"],
            Duration::from_secs(10),
            &env,
        )
        .await?;
        run_managed_git_with_env(
            context,
            &["add", "--all", "--", "."],
            Duration::from_secs(30),
            &env,
        )
        .await?;
        let oid = run_managed_git_with_env(context, &["write-tree"], Duration::from_secs(20), &env)
            .await?
            .trim()
            .to_string();
        validate_oid(&oid)?;
        Ok(oid)
    }
    .await;
    let _ = fs::remove_file(index);
    result
}

async fn git_oid(context: &RepositoryContext, args: &[&str]) -> Result<String, AppError> {
    let oid = run_managed_git(context, args, Duration::from_secs(10))
        .await?
        .trim()
        .to_string();
    validate_oid(&oid)?;
    Ok(oid)
}

fn validate_oid(oid: &str) -> Result<(), AppError> {
    if !(40..=64).contains(&oid.len()) || !oid.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(AppError::new(
            "git_object_invalid",
            "Git returned an invalid object identifier.",
        ));
    }
    Ok(())
}

pub async fn provision_session_repository(
    source: &RepositoryContext,
    workspace_root: &Path,
    session_id: &str,
) -> Result<RepositoryContext, AppError> {
    validate_source_git_directory(source)?;
    let workspace_root = crate::validation::canonicalize_for_use(workspace_root, "Workspace")?;
    let parent = workspace_root.join(MANAGED_REPOSITORIES_DIRECTORY);
    fs::create_dir_all(&parent).map_err(|_| storage_error())?;
    let parent = crate::validation::canonicalize_for_use(&parent, "Ark Code Repository storage")?;
    if !parent.starts_with(&workspace_root) {
        return Err(storage_error());
    }
    let destination = parent.join(session_id);
    if destination.exists() {
        let context = RepositoryContext::from_run_snapshot(path_text(&destination)?)?;
        validate_managed_repository(&context, &parent, session_id).await?;
        return Ok(context);
    }

    run_clone(source.root(), &destination).await?;
    let context = match RepositoryContext::from_run_snapshot(path_text(&destination)?) {
        Ok(context) => context,
        Err(error) => {
            remove_failed_clone(&destination, &parent);
            return Err(error);
        }
    };
    let branch = session_branch(session_id);
    if let Err(error) = run_managed_git(
        &context,
        &["checkout", "-b", &branch, "HEAD"],
        CLONE_TIMEOUT,
    )
    .await
    {
        remove_failed_clone(&destination, &parent);
        return Err(error);
    }
    // G1/RC-01: apply the source working tree's uncommitted modifications to the new managed
    // clone so tools see the full project state, not just the committed HEAD. Failures here are
    // best-effort and non-fatal — the agent can still read committed files.
    let _ = materialize_working_tree(source, &context).await;
    validate_managed_repository(&context, &parent, session_id).await?;
    Ok(context)
}

pub async fn initialize_project_repository(source: &RepositoryContext) -> Result<(), AppError> {
    let git_path = source.root().join(".git");
    if git_path.exists() {
        validate_source_git_directory(source)?;
        return Ok(());
    }
    run_git_process(
        Some(source.root()),
        &["init", "--", "."],
        Duration::from_secs(30),
        &[],
    )
    .await?;
    validate_source_git_directory(source)?;
    Ok(())
}

pub fn validate_run_repository(
    snapshot: &str,
    workspace_root: &Path,
    session_id: &str,
    expected_identity_hash: &str,
) -> Result<RepositoryContext, AppError> {
    let workspace_root = crate::validation::canonicalize_for_use(workspace_root, "Workspace")?;
    let parent = workspace_root.join(MANAGED_REPOSITORIES_DIRECTORY);
    if !parent.exists() {
        return Err(AppError::new(
            "code_repository_not_isolated",
            "This run predates Ark Code Repository isolation. Start a new session before allowing writes.",
        ));
    }
    let parent = crate::validation::canonicalize_for_use(&parent, "Ark Code Repository storage")?;
    let context = RepositoryContext::from_run_snapshot(snapshot)?;
    if !context.root().starts_with(&parent) || context.root() == parent {
        return Err(AppError::new(
            "code_repository_not_isolated",
            "This run predates Ark Code Repository isolation. Start a new session before allowing writes.",
        ));
    }
    validate_source_git_directory(&context)?;
    let (_, observed_identity_hash) = crate::code_sessions::repository_snapshot(context.root())?;
    if observed_identity_hash != expected_identity_hash {
        return Err(AppError::new(
            "repository_identity_changed",
            "Ark Code's isolated Repository identity changed. Start a new coding session.",
        ));
    }
    let head = fs::read_to_string(context.root().join(".git").join("HEAD")).map_err(|_| {
        AppError::new(
            "code_repository_branch_changed",
            "Ark Code could not verify its dedicated session branch.",
        )
    })?;
    let expected = format!("ref: refs/heads/{}\n", session_branch(session_id));
    if head.replace("\r\n", "\n") != expected {
        return Err(AppError::new(
            "code_repository_branch_changed",
            "Ark Code's managed Repository is no longer on its dedicated session branch.",
        ));
    }
    Ok(context)
}

async fn validate_managed_repository(
    context: &RepositoryContext,
    parent: &Path,
    session_id: &str,
) -> Result<(), AppError> {
    if !context.root().starts_with(parent) || context.root() == parent {
        return Err(storage_error());
    }
    validate_source_git_directory(context)?;
    let expected = session_branch(session_id);
    let branch = run_managed_git(
        context,
        &["branch", "--show-current"],
        Duration::from_secs(10),
    )
    .await?;
    if branch.trim() != expected {
        return Err(AppError::new(
            "code_repository_branch_changed",
            "Ark Code's managed Repository is no longer on its dedicated session branch.",
        ));
    }
    Ok(())
}

fn validate_source_git_directory(context: &RepositoryContext) -> Result<PathBuf, AppError> {
    let git_directory = context.root().join(".git");
    let metadata = fs::symlink_metadata(&git_directory).map_err(|_| {
        AppError::new(
            "git_repository_required",
            "Ark Code requires an initialized Git repository with at least one commit.",
        )
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(AppError::new(
            "git_metadata_outside_repository",
            "Ark Code requires Git metadata to be a real directory inside the Repository.",
        ));
    }
    let canonical = crate::validation::canonicalize_for_use(&git_directory, "Git metadata")?;
    if !canonical.starts_with(context.root()) {
        return Err(AppError::new(
            "git_metadata_outside_repository",
            "Git metadata resolves outside the Repository.",
        ));
    }
    Ok(canonical)
}

/// G1/RC-01: copy uncommitted working-tree state from `source` into `managed` so that
/// read-only tools see the full project state, not just the committed HEAD.
///
/// Called once per new session immediately after `git checkout -b ark/session/<id> HEAD`. On
/// session resume (managed directory already exists) this function is NOT called — the agent's
/// own prior edits are the authoritative working tree state by that point.
async fn materialize_working_tree(
    source: &RepositoryContext,
    managed: &RepositoryContext,
) -> Result<(), AppError> {
    const MAX_FILE_BYTES: u64 = 50 * 1024 * 1024; // skip individual files > 50 MB
    let timeout = Duration::from_secs(60);

    // Modified or added tracked files (M/A) and deleted tracked files (D) vs HEAD.
    let tracked_status = run_git_process(
        Some(source.root()),
        &["diff", "--name-status", "--diff-filter=MACD", "HEAD"],
        timeout,
        &[],
    )
    .await
    .unwrap_or_default();

    // Untracked files not excluded by .gitignore.
    let untracked = run_git_process(
        Some(source.root()),
        &["ls-files", "--others", "--exclude-standard"],
        timeout,
        &[],
    )
    .await
    .unwrap_or_default();

    // Process tracked modifications/deletions.
    for line in tracked_status.lines() {
        let line = line.trim();
        if line.len() < 3 {
            continue;
        }
        let status = line.chars().next().unwrap_or(' ');
        // name-status format: "<X>\t<path>" or "<X>\t<old>\t<new>" for renames (R/C).
        let path_part = if let Some(tab_pos) = line.find('\t') {
            let rest = &line[tab_pos + 1..];
            // For renames, take the rightmost tab-separated segment (the new name).
            rest.rsplit('\t').next().unwrap_or(rest).trim()
        } else {
            continue;
        };
        if !is_safe_relative_path(path_part) {
            continue;
        }
        if status == 'D' {
            let dst = managed.root().join(path_part);
            if dst.is_file() {
                let _ = fs::remove_file(&dst);
            }
        } else {
            let src = source.root().join(path_part);
            let dst = managed.root().join(path_part);
            copy_if_safe(&src, &dst, MAX_FILE_BYTES);
        }
    }

    // Process untracked files.
    for path_part in untracked.lines() {
        let path_part = path_part.trim();
        if path_part.is_empty() || !is_safe_relative_path(path_part) {
            continue;
        }
        let src = source.root().join(path_part);
        let dst = managed.root().join(path_part);
        copy_if_safe(&src, &dst, MAX_FILE_BYTES);
    }

    Ok(())
}

/// Returns `true` when `path` is safe to use as a relative suffix: no leading slash, no `..`
/// components, and no null bytes that could confuse the OS path layer.
fn is_safe_relative_path(path: &str) -> bool {
    if path.starts_with('/') || path.starts_with('\\') {
        return false;
    }
    for component in std::path::Path::new(path).components() {
        if component == std::path::Component::ParentDir {
            return false;
        }
    }
    !path.contains('\0')
}

/// Copy `src` to `dst`, creating parent directories as needed. Skips:
/// - non-regular-file sources (symlinks, directories)
/// - files larger than `max_bytes`
/// - any error (best-effort; materialization failures are non-fatal)
fn copy_if_safe(src: &Path, dst: &Path, max_bytes: u64) {
    let meta = match fs::symlink_metadata(src) {
        Ok(m) => m,
        Err(_) => return,
    };
    if !meta.is_file() {
        return;
    }
    if meta.len() > max_bytes {
        return;
    }
    if let Some(parent) = dst.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::copy(src, dst);
}

async fn run_clone(source: &Path, destination: &Path) -> Result<(), AppError> {
    // `--local --no-hardlinks` copies objects without invoking a source-side upload-pack hook and
    // avoids sharing mutable object storage with the user's checkout. `--no-checkout` ensures the
    // clone step itself cannot invoke working-tree filters. The subsequent checkout runs with an
    // empty global/system configuration; repository-local filter programs are not copied by clone.
    let source = path_text(source)?;
    let destination = path_text(destination)?;
    let args = [
        "clone",
        "--local",
        "--no-hardlinks",
        "--no-checkout",
        "--no-tags",
        "--no-recurse-submodules",
        source,
        destination,
    ];
    run_git_process(None, &args, CLONE_TIMEOUT, &[])
        .await
        .map(|_| ())
}

pub(crate) async fn run_managed_git(
    context: &RepositoryContext,
    args: &[&str],
    timeout: Duration,
) -> Result<String, AppError> {
    validate_source_git_directory(context)?;
    run_git_process(Some(context.root()), args, timeout, &[]).await
}

async fn run_managed_git_with_env(
    context: &RepositoryContext,
    args: &[&str],
    timeout: Duration,
    extra_env: &[(&str, &str)],
) -> Result<String, AppError> {
    validate_source_git_directory(context)?;
    run_git_process(Some(context.root()), args, timeout, extra_env).await
}

async fn run_git_process(
    repository: Option<&Path>,
    args: &[&str],
    timeout: Duration,
    extra_env: &[(&str, &str)],
) -> Result<String, AppError> {
    let null_device = if cfg!(windows) { "NUL" } else { "/dev/null" };
    let mut command = tokio::process::Command::new("git");
    crate::process_window::hide_tokio_process_window(&mut command);
    command
        .arg("--no-pager")
        .arg("--literal-pathspecs")
        .arg("-c")
        .arg("core.fsmonitor=false")
        .arg("-c")
        .arg(format!("core.hooksPath={null_device}"))
        .arg("-c")
        .arg("diff.external=")
        .arg("-c")
        .arg("core.quotePath=true");
    if let Some(repository) = repository {
        command.arg("-C").arg(repository);
    }
    command
        .args(args)
        .env_clear()
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", null_device)
        .env("GIT_PAGER", "cat")
        .env("LC_ALL", "C")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    for name in ["PATH", "SystemRoot", "WINDIR", "TEMP", "TMP"] {
        if let Some(value) = std::env::var_os(name) {
            command.env(name, value);
        }
    }
    for (name, value) in extra_env {
        command.env(name, value);
    }
    let mut child = command.spawn().map_err(|_| {
        AppError::new(
            "git_unavailable",
            "Git is not installed or could not be started safely.",
        )
    })?;
    let stdout = child.stdout.take().ok_or_else(git_failed)?;
    let stderr = child.stderr.take().ok_or_else(git_failed)?;
    let stdout_task = tokio::spawn(read_bounded(stdout));
    let stderr_task = tokio::spawn(read_bounded(stderr));
    let status = match tokio::time::timeout(timeout, child.wait()).await {
        Ok(result) => result.map_err(|_| git_failed())?,
        Err(_) => {
            let _ = child.kill().await;
            let _ = child.wait().await;
            return Err(AppError::new(
                "git_timeout",
                "Git exceeded Ark Code's bounded execution time.",
            ));
        }
    };
    let (stdout, stdout_overflow) = stdout_task.await.map_err(|_| git_failed())??;
    let (stderr, stderr_overflow) = stderr_task.await.map_err(|_| git_failed())??;
    if stdout_overflow || stderr_overflow {
        return Err(AppError::new(
            "git_output_too_large",
            "Git output exceeded Ark Code's bounded result limit.",
        ));
    }
    if !status.success() {
        let detail = String::from_utf8_lossy(&stderr);
        let message = if detail.contains("does not have any commits yet")
            || detail.contains("not a valid object name: 'HEAD'")
            || detail.contains("ambiguous argument 'HEAD'")
        {
            "Ark Code requires the Repository to have at least one commit before a session can start."
        } else {
            "Git could not prepare Ark Code's isolated Repository."
        };
        return Err(AppError::new("git_operation_failed", message));
    }
    String::from_utf8(stdout).map_err(|_| {
        AppError::new(
            "git_non_utf8_output",
            "Git returned output Ark Code could not represent safely.",
        )
    })
}

async fn read_bounded<R: tokio::io::AsyncRead + Unpin>(
    reader: R,
) -> Result<(Vec<u8>, bool), AppError> {
    let mut bytes = Vec::new();
    reader
        .take((CLONE_OUTPUT_LIMIT + 1) as u64)
        .read_to_end(&mut bytes)
        .await
        .map_err(|_| git_failed())?;
    let overflow = bytes.len() > CLONE_OUTPUT_LIMIT;
    bytes.truncate(CLONE_OUTPUT_LIMIT);
    Ok((bytes, overflow))
}

fn session_branch(session_id: &str) -> String {
    format!("ark/session/{session_id}")
}

fn path_text(path: &Path) -> Result<&str, AppError> {
    path.to_str().ok_or_else(|| {
        AppError::invalid_input("Ark Code Repository paths must contain valid Unicode.")
    })
}

fn remove_failed_clone(destination: &Path, parent: &Path) {
    // The exact destination is computed from a validated entity ID and rechecked beneath Ark's
    // dedicated storage directory. Cleanup is best-effort; a later run will fail closed if a
    // partial directory remains.
    if destination.starts_with(parent) && destination != parent {
        let _ = fs::remove_dir_all(destination);
    }
}

fn git_failed() -> AppError {
    AppError::new(
        "git_operation_failed",
        "Git could not prepare Ark Code's isolated Repository.",
    )
}

fn storage_error() -> AppError {
    AppError::new(
        "code_repository_storage_failed",
        "Ark could not prepare private storage for this Code session.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn git(repository: &Path, args: &[&str]) {
        let status = std::process::Command::new("git")
            .arg("-C")
            .arg(repository)
            .args(args)
            .status()
            .expect("git starts");
        assert!(status.success(), "git command failed: {args:?}");
    }

    #[tokio::test]
    async fn managed_clone_materializes_working_tree_and_uses_dedicated_branch() {
        let fixture = std::env::temp_dir().join(format!("ark-code-git-{}", Uuid::new_v4()));
        let source_root = fixture.join("source");
        let workspace_root = fixture.join("workspace");
        fs::create_dir_all(&source_root).expect("source created");
        fs::create_dir_all(&workspace_root).expect("workspace created");
        fs::write(source_root.join("tracked.txt"), "committed\n").expect("tracked file");
        git(&source_root, &["init"]);
        git(&source_root, &["add", "tracked.txt"]);
        git(
            &source_root,
            &[
                "-c",
                "user.name=Ark Test",
                "-c",
                "user.email=ark-test@local.invalid",
                "commit",
                "-m",
                "fixture",
            ],
        );
        fs::write(source_root.join("tracked.txt"), "dirty\n").expect("dirty tracked file");
        fs::write(source_root.join("untracked.txt"), "private\n").expect("untracked file");

        let source = RepositoryContext::from_run_snapshot(
            source_root.to_str().expect("source path Unicode"),
        )
        .expect("source context");
        let session_id = Uuid::new_v4().to_string();
        let managed = provision_session_repository(&source, &workspace_root, &session_id)
            .await
            .expect("managed clone");

        // G1/RC-01: the managed clone must reflect the source working tree (not just HEAD).
        assert_eq!(
            fs::read_to_string(managed.root().join("tracked.txt")).expect("managed tracked file"),
            "dirty\n",
            "working-tree modification must be materialized into the managed clone"
        );
        assert_eq!(
            fs::read_to_string(managed.root().join("untracked.txt"))
                .expect("untracked materialized"),
            "private\n",
            "untracked source files must be materialized into the managed clone"
        );
        let branch = run_managed_git(
            &managed,
            &["branch", "--show-current"],
            Duration::from_secs(10),
        )
        .await
        .expect("branch read");
        assert_eq!(branch.trim(), format!("ark/session/{session_id}"));
        assert_eq!(
            fs::read_to_string(source_root.join("tracked.txt")).expect("source remains"),
            "dirty\n"
        );

        fs::write(managed.root().join("tracked.txt"), "checkpoint one\n").expect("managed edit");
        fs::write(managed.root().join("new.txt"), "new file\n").expect("managed new file");
        let first_preview = preview_checkpoint(
            &managed,
            GitCheckpointArguments {
                message: "First Ark checkpoint".to_string(),
            },
        )
        .await
        .expect("checkpoint preview");
        assert!(first_preview.content.contains("new.txt"));
        let first = execute_checkpoint(&managed, &first_preview)
            .await
            .expect("checkpoint executes");

        fs::write(managed.root().join("tracked.txt"), "checkpoint two\n")
            .expect("second managed edit");
        let second_preview = preview_checkpoint(
            &managed,
            GitCheckpointArguments {
                message: "Second Ark checkpoint".to_string(),
            },
        )
        .await
        .expect("second preview");
        let second = execute_checkpoint(&managed, &second_preview)
            .await
            .expect("second checkpoint");

        fs::write(
            managed.root().join("unowned.txt"),
            "not an Ark checkpoint\n",
        )
        .expect("unowned file");
        git(managed.root(), &["add", "unowned.txt"]);
        git(
            managed.root(),
            &[
                "-c",
                "user.name=Outside Process",
                "-c",
                "user.email=outside@local.invalid",
                "commit",
                "-m",
                "unowned commit",
            ],
        );
        let unowned_error = preview_rollback(
            &managed,
            GitRollbackArguments {
                checkpoint_id: Uuid::new_v4().to_string(),
            },
            &first.commit_oid,
            &first.parent_commit_oid,
            &[first.commit_oid.clone(), second.commit_oid.clone()],
        )
        .await
        .expect_err("rollback must not cross a commit Ark did not record");
        assert_eq!(unowned_error.code, "git_rollback_unowned_commit");
        git(managed.root(), &["reset", "--hard", &second.commit_oid]);

        let rollback_preview = preview_rollback(
            &managed,
            GitRollbackArguments {
                checkpoint_id: Uuid::new_v4().to_string(),
            },
            &first.commit_oid,
            &first.parent_commit_oid,
            &[first.commit_oid.clone(), second.commit_oid.clone()],
        )
        .await
        .expect("rollback preview");
        execute_rollback(
            &managed,
            &rollback_preview,
            &first.parent_commit_oid,
            &[first.commit_oid, second.commit_oid],
        )
        .await
        .expect("rollback executes");
        assert_eq!(
            fs::read_to_string(managed.root().join("tracked.txt")).expect("restored file"),
            "checkpoint one\n"
        );
        assert_eq!(
            fs::read_to_string(source_root.join("tracked.txt")).expect("source still remains"),
            "dirty\n"
        );

        let _ = fs::remove_dir_all(fixture);
    }

    #[test]
    fn legacy_run_snapshot_outside_managed_storage_is_rejected() {
        let fixture = std::env::temp_dir().join(format!("ark-code-git-{}", Uuid::new_v4()));
        let source_root = fixture.join("source");
        let workspace_root = fixture.join("workspace");
        fs::create_dir_all(source_root.join(".git")).expect("git metadata");
        fs::create_dir_all(&workspace_root).expect("workspace");
        let error = validate_run_repository(
            source_root.to_str().expect("source path Unicode"),
            &workspace_root,
            &Uuid::new_v4().to_string(),
            &"0".repeat(64),
        )
        .expect_err("legacy snapshot must fail closed");
        assert_eq!(error.code, "code_repository_not_isolated");
        let _ = fs::remove_dir_all(fixture);
    }

    #[tokio::test]
    async fn explicit_git_initialization_creates_only_repository_metadata() {
        if std::process::Command::new("git")
            .arg("--version")
            .output()
            .is_err()
        {
            return;
        }
        let fixture = std::env::temp_dir().join(format!("ark-code-git-init-{}", Uuid::new_v4()));
        fs::create_dir_all(&fixture).expect("fixture created");
        fs::write(fixture.join("existing.txt"), "user work\n").expect("user file");
        let context = RepositoryContext::from_run_snapshot(
            fixture.to_str().expect("fixture path is Unicode"),
        )
        .expect("context created");

        initialize_project_repository(&context)
            .await
            .expect("explicit initialization succeeds");

        assert!(fixture.join(".git").is_dir());
        assert_eq!(
            fs::read_to_string(fixture.join("existing.txt")).expect("user file remains"),
            "user work\n"
        );
        let status = run_managed_git(
            &context,
            &["status", "--porcelain"],
            Duration::from_secs(10),
        )
        .await
        .expect("repository status");
        assert!(status.contains("existing.txt"));
        let _ = fs::remove_dir_all(fixture);
    }
}
