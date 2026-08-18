//! CODE-005 user-configured, fixed-template verification commands.
//!
//! The model can select only a durable command-definition ID. It cannot supply an executable,
//! arguments, shell syntax, environment, cwd, timeout, or output destination.

use crate::code_sessions::{CodeCommandDefinition, CodeRecoveryOutcome};
use crate::code_tools::RepositoryContext;
use crate::errors::AppError;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Stdio;
use std::time::Duration;
use tokio::io::AsyncReadExt;

pub const RUN_COMMAND_TOOL_ID: &str = "run_verification_command";
const MAX_ARGUMENTS: usize = 64;
const MAX_ARGUMENT_CHARS: usize = 2_048;
const MAX_TOTAL_ARGUMENT_CHARS: usize = 16_000;
const MAX_OUTPUT_BYTES: usize = 48 * 1024;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RunCommandArguments {
    pub command_id: String,
}

#[derive(Debug, Clone)]
pub struct CommandPreview {
    pub arguments_json: String,
    pub content: String,
    pub call_hash: String,
    pub preview_hash: String,
    pub precondition_hash: String,
    pub definition: CodeCommandDefinition,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandOutcome {
    pub command_id: String,
    pub label: String,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub timed_out: bool,
    pub cancelled: bool,
    pub outcome: CodeRecoveryOutcome,
}

pub fn validate_command_definition(
    label: &str,
    program: &str,
    arguments: &[String],
    timeout_seconds: u32,
) -> Result<(), AppError> {
    let label = label.trim();
    let program = program.trim();
    if label.is_empty() || label.chars().count() > 80 {
        return Err(AppError::invalid_input(
            "Command label must be between 1 and 80 characters.",
        ));
    }
    if program.is_empty() || program.chars().count() > 128 {
        return Err(AppError::invalid_input(
            "Command program must be between 1 and 128 characters.",
        ));
    }
    if Path::new(program).components().count() != 1
        || program.contains(['/', '\\'])
        || program.contains('\0')
    {
        return Err(AppError::invalid_input(
            "Command program must be an executable name resolved from Ark's restricted PATH.",
        ));
    }
    let normalized = program
        .to_ascii_lowercase()
        .trim_end_matches(".exe")
        .to_string();
    const SHELLS: &[&str] = &[
        "cmd",
        "command",
        "powershell",
        "pwsh",
        "sh",
        "bash",
        "dash",
        "zsh",
        "fish",
        "wsl",
        "cscript",
        "wscript",
    ];
    if SHELLS.contains(&normalized.as_str())
        || program.to_ascii_lowercase().ends_with(".cmd")
        || program.to_ascii_lowercase().ends_with(".bat")
        || program.to_ascii_lowercase().ends_with(".ps1")
    {
        return Err(AppError::invalid_input(
            "Shells and shell scripts cannot be added to Ark Code's command allowlist.",
        ));
    }
    const VERIFICATION_RUNNERS: &[&str] = &[
        "biome",
        "bun",
        "cargo",
        "clang",
        "clang++",
        "cmake",
        "ctest",
        "deno",
        "dotnet",
        "eslint",
        "g++",
        "gcc",
        "go",
        "gradle",
        "java",
        "javac",
        "jest",
        "make",
        "mvn",
        "mypy",
        "ninja",
        "node",
        "npm",
        "npx",
        "pnpm",
        "prettier",
        "pytest",
        "python",
        "python3",
        "ruff",
        "rustc",
        "swift",
        "tsc",
        "uv",
        "vitest",
        "xcodebuild",
        "yarn",
        "zig",
    ];
    if !VERIFICATION_RUNNERS.contains(&normalized.as_str()) {
        return Err(AppError::invalid_input(
            "Ark Code allows only recognized test, build, and lint runners.",
        ));
    }
    if arguments.len() > MAX_ARGUMENTS
        || arguments.iter().any(|argument| {
            argument.contains('\0') || argument.chars().count() > MAX_ARGUMENT_CHARS
        })
        || arguments
            .iter()
            .map(|argument| argument.chars().count())
            .sum::<usize>()
            > MAX_TOTAL_ARGUMENT_CHARS
    {
        return Err(AppError::invalid_input(
            "Command arguments exceed Ark Code's fixed-template limits.",
        ));
    }
    if !(1..=1_800).contains(&timeout_seconds) {
        return Err(AppError::invalid_input(
            "Command timeout must be between 1 and 1800 seconds.",
        ));
    }
    Ok(())
}

pub fn preview_command(
    context: &RepositoryContext,
    arguments: RunCommandArguments,
    definition: CodeCommandDefinition,
) -> Result<CommandPreview, AppError> {
    crate::validation::validate_entity_id(&arguments.command_id, "Command definition ID")?;
    if arguments.command_id != definition.id || !definition.enabled {
        return Err(AppError::new(
            "code_command_not_allowed",
            "This verification command is not enabled in the user's allowlist.",
        ));
    }
    validate_command_definition(
        &definition.label,
        &definition.program,
        &definition.arguments,
        definition.timeout_seconds,
    )?;
    let arguments_json = crate::code_sessions::serialize_json(&arguments)?;
    let rendered = std::iter::once(definition.program.as_str())
        .chain(definition.arguments.iter().map(String::as_str))
        .map(render_argument)
        .collect::<Vec<_>>()
        .join(" ");
    let content = format!(
        "Run verification command: {}\nCommand: {}\nWorking directory: {}\nTimeout: {} seconds\nEnvironment: stripped (PATH/system/temp, home locator, allowlisted toolchain roots, CI/NO_COLOR only)",
        definition.label,
        rendered,
        context.root().display(),
        definition.timeout_seconds,
    );
    let call_hash = crate::code_sessions::compute_call_hash(&arguments)?;
    let preview_hash = crate::code_sessions::compute_preview_hash(&content);
    let precondition_hash = crate::code_sessions::compute_precondition_hash(&serde_json::json!({
        "definitionId": definition.id,
        "program": definition.program,
        "arguments": definition.arguments,
        "timeoutSeconds": definition.timeout_seconds,
        "enabled": definition.enabled,
        "repositoryRoot": context.root(),
    }))?;
    Ok(CommandPreview {
        arguments_json,
        content,
        call_hash,
        preview_hash,
        precondition_hash,
        definition,
    })
}

pub async fn execute_command(
    context: &RepositoryContext,
    approved: &CommandPreview,
    cancellation: &crate::code_agent::CodeRunCancellation,
) -> Result<CommandOutcome, AppError> {
    let fresh = preview_command(
        context,
        RunCommandArguments {
            command_id: approved.definition.id.clone(),
        },
        approved.definition.clone(),
    )?;
    if fresh.call_hash != approved.call_hash
        || fresh.preview_hash != approved.preview_hash
        || fresh.precondition_hash != approved.precondition_hash
    {
        return Err(AppError::new(
            "code_command_approval_stale",
            "The command definition changed after this execution was reviewed.",
        ));
    }
    let mut command = tokio::process::Command::new(&approved.definition.program);
    crate::process_window::hide_tokio_process_window(&mut command);
    command
        .args(&approved.definition.arguments)
        .current_dir(context.root())
        .env_clear()
        .env("CI", "1")
        .env("NO_COLOR", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    for name in [
        "PATH",
        "SystemRoot",
        "WINDIR",
        "TEMP",
        "TMP",
        "USERPROFILE",
        "HOME",
        "CARGO_HOME",
        "RUSTUP_HOME",
        "DOTNET_ROOT",
        "JAVA_HOME",
        "GOROOT",
    ] {
        if let Some(value) = std::env::var_os(name) {
            command.env(name, value);
        }
    }
    let mut child = command.spawn().map_err(|_| {
        AppError::new(
            "code_command_unavailable",
            "The approved verification command could not be started.",
        )
    })?;
    let stdout = child.stdout.take().ok_or_else(command_failed)?;
    let stderr = child.stderr.take().ok_or_else(command_failed)?;
    let stdout_task = tokio::spawn(read_bounded(stdout));
    let stderr_task = tokio::spawn(read_bounded(stderr));
    let timeout = tokio::time::sleep(Duration::from_secs(u64::from(
        approved.definition.timeout_seconds,
    )));
    tokio::pin!(timeout);
    let (status, timed_out, cancelled) = tokio::select! {
        biased;
        _ = cancellation.cancelled() => {
            let _ = child.kill().await;
            let status = child.wait().await.ok();
            (status, false, true)
        },
        _ = &mut timeout => {
            let _ = child.kill().await;
            let status = child.wait().await.ok();
            (status, true, false)
        },
        status = child.wait() => (Some(status.map_err(|_| command_failed())?), false, false),
    };
    let (stdout, stdout_overflow) = stdout_task.await.map_err(|_| command_failed())??;
    let (stderr, stderr_overflow) = stderr_task.await.map_err(|_| command_failed())??;
    if stdout_overflow || stderr_overflow {
        return Err(AppError::new(
            "code_command_output_too_large",
            "The verification command exceeded Ark Code's bounded output limit.",
        ));
    }
    let exit_code = status.and_then(|status| status.code());
    Ok(CommandOutcome {
        command_id: approved.definition.id.clone(),
        label: approved.definition.label.clone(),
        exit_code,
        stdout: String::from_utf8_lossy(&stdout).into_owned(),
        stderr: String::from_utf8_lossy(&stderr).into_owned(),
        timed_out,
        cancelled,
        // The process outcome was observed and is therefore durably classifiable even when the
        // verification itself failed, timed out, or was cancelled.
        outcome: CodeRecoveryOutcome::Applied,
    })
}

async fn read_bounded<R: tokio::io::AsyncRead + Unpin>(
    reader: R,
) -> Result<(Vec<u8>, bool), AppError> {
    let mut bytes = Vec::new();
    reader
        .take((MAX_OUTPUT_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .await
        .map_err(|_| command_failed())?;
    let overflow = bytes.len() > MAX_OUTPUT_BYTES;
    bytes.truncate(MAX_OUTPUT_BYTES);
    Ok((bytes, overflow))
}

fn render_argument(value: &str) -> String {
    if value
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || "-._/:=@".contains(character))
    {
        value.to_string()
    } else {
        format!("\"{}\"", value.replace('"', "\\\""))
    }
}

fn command_failed() -> AppError {
    AppError::new(
        "code_command_failed",
        "The approved verification command could not be observed safely.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_policy_rejects_shells_paths_and_unbounded_arguments() {
        assert!(validate_command_definition("Tests", "cargo", &["test".into()], 60).is_ok());
        for program in [
            "cmd",
            "powershell.exe",
            "../cargo",
            "check.cmd",
            "git",
            "curl",
        ] {
            assert!(validate_command_definition("Unsafe", program, &[], 60).is_err());
        }
        assert!(validate_command_definition(
            "Too large",
            "cargo",
            &["x".repeat(MAX_ARGUMENT_CHARS + 1)],
            60,
        )
        .is_err());
    }

    #[tokio::test]
    async fn approved_command_uses_fixed_template_and_is_killable() {
        let root =
            std::env::temp_dir().join(format!("ark-code-command-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).expect("repository root");
        let context =
            RepositoryContext::from_run_snapshot(root.to_str().expect("repository path Unicode"))
                .expect("context");
        let definition = CodeCommandDefinition {
            id: uuid::Uuid::new_v4().to_string(),
            label: "Cargo version".to_string(),
            program: "cargo".to_string(),
            arguments: vec!["--version".to_string()],
            timeout_seconds: 30,
            enabled: true,
            created_at: "2026-08-17T00:00:00Z".to_string(),
            updated_at: "2026-08-17T00:00:00Z".to_string(),
        };
        let preview = preview_command(
            &context,
            RunCommandArguments {
                command_id: definition.id.clone(),
            },
            definition,
        )
        .expect("preview");
        let cancellation = crate::code_agent::CodeRunCancellation::new();
        let result = execute_command(&context, &preview, &cancellation)
            .await
            .expect("command observed");
        assert_eq!(result.exit_code, Some(0), "stderr: {}", result.stderr);
        assert!(result.stdout.to_ascii_lowercase().contains("cargo"));

        let cancellation = crate::code_agent::CodeRunCancellation::new();
        cancellation.request();
        let cancelled = execute_command(&context, &preview, &cancellation)
            .await
            .expect("cancelled command observed");
        assert!(cancelled.cancelled);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn disabled_or_substituted_command_ids_cannot_be_previewed() {
        let root =
            std::env::temp_dir().join(format!("ark-code-command-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).expect("repository root");
        let context =
            RepositoryContext::from_run_snapshot(root.to_str().expect("repository path Unicode"))
                .expect("context");
        let definition = CodeCommandDefinition {
            id: uuid::Uuid::new_v4().to_string(),
            label: "Cargo check".to_string(),
            program: "cargo".to_string(),
            arguments: vec!["check".to_string()],
            timeout_seconds: 30,
            enabled: false,
            created_at: "2026-08-17T00:00:00Z".to_string(),
            updated_at: "2026-08-17T00:00:00Z".to_string(),
        };
        let substituted = preview_command(
            &context,
            RunCommandArguments {
                command_id: uuid::Uuid::new_v4().to_string(),
            },
            definition.clone(),
        )
        .expect_err("a different durable ID is not authorization");
        assert_eq!(substituted.code, "code_command_not_allowed");
        let disabled = preview_command(
            &context,
            RunCommandArguments {
                command_id: definition.id.clone(),
            },
            definition,
        )
        .expect_err("disabled templates are not callable");
        assert_eq!(disabled.code, "code_command_not_allowed");
        let _ = std::fs::remove_dir_all(root);
    }
}
