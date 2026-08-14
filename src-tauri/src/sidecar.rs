use crate::errors::AppError;
use reqwest::{Client, StatusCode};
use serde::Serialize;
use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const MAX_LOG_ENTRIES: usize = 200;
const MAX_LOG_BYTES: usize = 128 * 1024;
const MAX_LOG_LINE_CHARS: usize = 2_048;
const READINESS_ATTEMPTS: usize = 60;
const PORT_DISCOVERY_ATTEMPTS: usize = 60;
const READINESS_INTERVAL: Duration = Duration::from_millis(500);
const STOP_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeLifecycleState {
    Stopped,
    Starting,
    Healthy,
    Degraded,
    Stopping,
    Crashed,
    UnavailableBinary,
    UnavailableModel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeFailureCategory {
    BinaryUnavailable,
    ModelUnavailable,
    PortUnavailable,
    SpawnFailed,
    AuthenticationFailed,
    HealthRejected,
    HealthUnreachable,
    ReadinessTimeout,
    ProcessExited,
    ProcessMonitorFailed,
    StopFailed,
    StateUnavailable,
    SupplyChainVerificationFailed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeFailure {
    pub category: RuntimeFailureCategory,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeLogEntry {
    pub timestamp_ms: u128,
    pub stream: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeDiagnostics {
    pub state: RuntimeLifecycleState,
    pub pid: Option<u32>,
    pub port: Option<u16>,
    pub model_configured: bool,
    pub failure: Option<RuntimeFailure>,
    pub recent_logs: Vec<RuntimeLogEntry>,
}

#[derive(Debug, Default)]
struct RuntimeLogBuffer {
    entries: VecDeque<RuntimeLogEntry>,
    bytes: usize,
}

impl RuntimeLogBuffer {
    fn push(&mut self, entry: RuntimeLogEntry) {
        self.bytes = self.bytes.saturating_add(entry.message.len());
        self.entries.push_back(entry);
        while self.entries.len() > MAX_LOG_ENTRIES || self.bytes > MAX_LOG_BYTES {
            if let Some(removed) = self.entries.pop_front() {
                self.bytes = self.bytes.saturating_sub(removed.message.len());
            } else {
                break;
            }
        }
    }

    fn recent(&self, limit: usize) -> Vec<RuntimeLogEntry> {
        let skip = self.entries.len().saturating_sub(limit);
        self.entries.iter().skip(skip).cloned().collect()
    }
}

pub struct SidecarState {
    process: Option<Child>,
    port: Option<u16>,
    model_path: Option<String>,
    /// SEC-002: per-launch bearer secret; memory-only and never returned by diagnostics.
    api_key: Option<String>,
    state: RuntimeLifecycleState,
    failure: Option<RuntimeFailure>,
    logs: Arc<Mutex<RuntimeLogBuffer>>,
    assigned_port: Arc<Mutex<Option<u16>>>,
    redaction_values: Vec<String>,
    /// SEC-002: the authenticating proxy's own loopback port — this, not `port` (llama-server's
    /// raw internal port), is what `base_url` and `BuiltInRuntimeStatus.port` expose. `port`
    /// stays internal-only, used solely for Ark's own direct health checks.
    proxy_port: Option<u16>,
    /// Aborted on `stop`/`clear_process_metadata` so the proxy never outlives the runtime it
    /// fronts.
    proxy_task: Option<tokio::task::JoinHandle<()>>,
}

impl SidecarState {
    pub fn new() -> Self {
        Self {
            process: None,
            port: None,
            model_path: None,
            api_key: None,
            state: RuntimeLifecycleState::Stopped,
            failure: None,
            logs: Arc::new(Mutex::new(RuntimeLogBuffer::default())),
            assigned_port: Arc::new(Mutex::new(None)),
            redaction_values: Vec::new(),
            proxy_port: None,
            proxy_task: None,
        }
    }

    pub fn begin_start(&mut self, model_path: &str, binary: &Path) {
        self.state = RuntimeLifecycleState::Starting;
        self.failure = None;
        self.model_path = Some(model_path.to_string());
        self.redaction_values = vec![
            model_path.to_string(),
            model_path.replace('\\', "/"),
            binary.display().to_string(),
            binary.display().to_string().replace('\\', "/"),
        ];
        self.clear_logs();
        *lock_unpoisoned(&self.assigned_port) = None;
    }

    pub fn mark_unavailable_binary(&mut self, binary: &Path) {
        self.state = RuntimeLifecycleState::UnavailableBinary;
        self.failure = Some(RuntimeFailure {
            category: RuntimeFailureCategory::BinaryUnavailable,
            message: "The managed runtime binary is not installed or is not accessible."
                .to_string(),
        });
        self.redaction_values = vec![
            binary.display().to_string(),
            binary.display().to_string().replace('\\', "/"),
        ];
    }

    pub fn mark_unavailable_model(&mut self, model_path: &str, message: &str) {
        self.state = RuntimeLifecycleState::UnavailableModel;
        self.model_path = Some(model_path.to_string());
        self.redaction_values = vec![model_path.to_string(), model_path.replace('\\', "/")];
        self.failure = Some(RuntimeFailure {
            category: RuntimeFailureCategory::ModelUnavailable,
            message: redact_runtime_log(message, &self.redaction_values),
        });
    }

    pub fn mark_failure(&mut self, category: RuntimeFailureCategory, message: impl Into<String>) {
        let safe_message = redact_runtime_log(&message.into(), &self.redaction_values);
        self.failure = Some(RuntimeFailure {
            category,
            message: safe_message,
        });
        self.state = match category {
            RuntimeFailureCategory::BinaryUnavailable => RuntimeLifecycleState::UnavailableBinary,
            RuntimeFailureCategory::ModelUnavailable => RuntimeLifecycleState::UnavailableModel,
            RuntimeFailureCategory::ProcessExited => RuntimeLifecycleState::Crashed,
            _ => RuntimeLifecycleState::Degraded,
        };
    }

    pub fn attach_process(
        &mut self,
        mut child: Child,
        port: Option<u16>,
        model_path: String,
        api_key: String,
    ) {
        self.redaction_values.push(api_key.clone());
        *lock_unpoisoned(&self.assigned_port) = port;
        if let Some(stdout) = child.stdout.take() {
            spawn_log_reader(
                stdout,
                "stdout",
                Arc::clone(&self.logs),
                Arc::clone(&self.assigned_port),
                self.redaction_values.clone(),
            );
        }
        if let Some(stderr) = child.stderr.take() {
            spawn_log_reader(
                stderr,
                "stderr",
                Arc::clone(&self.logs),
                Arc::clone(&self.assigned_port),
                self.redaction_values.clone(),
            );
        }
        self.process = Some(child);
        self.port = port;
        self.model_path = Some(model_path);
        self.api_key = Some(api_key);
        self.state = RuntimeLifecycleState::Starting;
        self.failure = None;
    }

    pub fn mark_healthy(&mut self) {
        self.state = RuntimeLifecycleState::Healthy;
        self.failure = None;
    }

    /// SEC-002: records the authenticating proxy's own loopback port and background task once
    /// it's confirmed listening in front of the (already-healthy) managed runtime. Aborts any
    /// stale previous proxy task first — defensive, since callers only invoke this once per
    /// launch after a successful `mark_healthy`, but a leaked background task would be a silent
    /// resource/security regression if that ever changed.
    pub fn attach_proxy(&mut self, proxy_port: u16, task: tokio::task::JoinHandle<()>) {
        if let Some(previous) = self.proxy_task.replace(task) {
            previous.abort();
        }
        self.proxy_port = Some(proxy_port);
    }

    pub fn proxy_port(&self) -> Option<u16> {
        self.proxy_port
    }

    pub fn reconcile_process(&mut self) -> bool {
        let Some(child) = &mut self.process else {
            return false;
        };
        match child.try_wait() {
            Ok(Some(status)) => {
                let message = match status.code() {
                    Some(code) => format!("The managed runtime exited with status code {code}."),
                    None => {
                        "The managed runtime exited after receiving a platform signal.".to_string()
                    }
                };
                self.process = None;
                self.clear_process_metadata();
                self.mark_failure(RuntimeFailureCategory::ProcessExited, message);
                false
            }
            Ok(None) => true,
            Err(error) => {
                self.mark_failure(
                    RuntimeFailureCategory::ProcessMonitorFailed,
                    format!("Ark could not query the managed runtime process: {error}"),
                );
                true
            }
        }
    }

    pub fn stop(&mut self) -> Result<(), RuntimeFailure> {
        self.state = RuntimeLifecycleState::Stopping;
        let Some(mut child) = self.process.take() else {
            self.clear_process_metadata();
            self.state = RuntimeLifecycleState::Stopped;
            self.failure = None;
            return Ok(());
        };

        if let Err(error) = child.kill() {
            if matches!(child.try_wait(), Ok(Some(_))) {
                self.clear_process_metadata();
                self.state = RuntimeLifecycleState::Stopped;
                self.failure = None;
                return Ok(());
            }
            let failure = RuntimeFailure {
                category: RuntimeFailureCategory::StopFailed,
                message: redact_runtime_log(
                    &format!("Ark could not terminate the managed runtime: {error}"),
                    &self.redaction_values,
                ),
            };
            self.process = Some(child);
            self.state = RuntimeLifecycleState::Degraded;
            self.failure = Some(failure.clone());
            return Err(failure);
        }

        let deadline = std::time::Instant::now() + STOP_TIMEOUT;
        loop {
            match child.try_wait() {
                Ok(Some(_)) => {
                    self.clear_process_metadata();
                    self.state = RuntimeLifecycleState::Stopped;
                    self.failure = None;
                    return Ok(());
                }
                Ok(None) if std::time::Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(20));
                }
                Ok(None) => {
                    let failure = RuntimeFailure {
                        category: RuntimeFailureCategory::StopFailed,
                        message:
                            "The managed runtime did not stop within the 2-second safety bound."
                                .to_string(),
                    };
                    self.process = Some(child);
                    self.state = RuntimeLifecycleState::Degraded;
                    self.failure = Some(failure.clone());
                    return Err(failure);
                }
                Err(error) => {
                    let failure = RuntimeFailure {
                        category: RuntimeFailureCategory::StopFailed,
                        message: format!(
                            "Ark could not confirm that the managed runtime stopped: {error}"
                        ),
                    };
                    self.state = RuntimeLifecycleState::Degraded;
                    self.failure = Some(failure.clone());
                    return Err(failure);
                }
            }
        }
    }

    pub fn api_key(&self) -> Option<String> {
        self.api_key.clone()
    }

    pub fn state(&self) -> RuntimeLifecycleState {
        self.state
    }

    pub fn port(&self) -> Option<u16> {
        self.port.or(*lock_unpoisoned(&self.assigned_port))
    }

    pub fn model_path(&self) -> Option<String> {
        self.model_path.clone()
    }

    pub fn failure(&self) -> Option<RuntimeFailure> {
        self.failure.clone()
    }

    pub fn recent_logs(&self, limit: usize) -> Vec<RuntimeLogEntry> {
        lock_unpoisoned(&self.logs).recent(limit.min(MAX_LOG_ENTRIES))
    }

    pub fn safe_failure_with_excerpt(&self) -> Option<RuntimeFailure> {
        let mut failure = self.failure.clone()?;
        let excerpts = self.recent_logs(5);
        if !excerpts.is_empty() {
            let text = excerpts
                .iter()
                .map(|entry| format!("[{}] {}", entry.stream, entry.message))
                .collect::<Vec<_>>()
                .join("\n");
            failure.message = format!("{} Recent safe runtime output:\n{text}", failure.message);
        }
        Some(failure)
    }

    pub fn diagnostics(&mut self, include_logs: bool) -> RuntimeDiagnostics {
        self.reconcile_process();
        self.sync_assigned_port();
        RuntimeDiagnostics {
            state: self.state,
            pid: self.process.as_ref().map(Child::id),
            // SEC-002: report the authenticating proxy's port (the actual reachable front
            // door), falling back to llama-server's raw internal port only before the proxy
            // has attached (e.g. mid-`Starting`) — never expose a port that, once healthy,
            // bypasses the proxy's auth/CORS enforcement.
            port: self.proxy_port.or(self.port),
            model_configured: self.model_path.is_some(),
            failure: self.failure.clone(),
            recent_logs: if include_logs {
                self.recent_logs(50)
            } else {
                Vec::new()
            },
        }
    }

    fn clear_logs(&mut self) {
        *lock_unpoisoned(&self.logs) = RuntimeLogBuffer::default();
    }

    fn clear_process_metadata(&mut self) {
        self.port = None;
        *lock_unpoisoned(&self.assigned_port) = None;
        self.api_key = None;
        self.redaction_values.clear();
        self.proxy_port = None;
        if let Some(task) = self.proxy_task.take() {
            task.abort();
        }
    }

    fn sync_assigned_port(&mut self) -> Option<u16> {
        if self.port.is_none() {
            self.port = *lock_unpoisoned(&self.assigned_port);
        }
        self.port
    }
}

impl Default for SidecarState {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for SidecarState {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

fn lock_unpoisoned<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn spawn_log_reader<R: Read + Send + 'static>(
    reader: R,
    stream: &'static str,
    logs: Arc<Mutex<RuntimeLogBuffer>>,
    assigned_port: Arc<Mutex<Option<u16>>>,
    redaction_values: Vec<String>,
) {
    std::thread::spawn(move || {
        for line in BufReader::new(reader).lines() {
            let message = match line {
                Ok(line) => {
                    if let Some(port) = parse_assigned_loopback_port(&line) {
                        let mut discovered = lock_unpoisoned(&assigned_port);
                        if discovered.is_none() {
                            *discovered = Some(port);
                        }
                    }
                    redact_runtime_log(&line, &redaction_values)
                }
                Err(error) => format!("Runtime log reader stopped: {error}"),
            };
            let message = message.chars().take(MAX_LOG_LINE_CHARS).collect::<String>();
            lock_unpoisoned(&logs).push(RuntimeLogEntry {
                timestamp_ms: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis(),
                stream: stream.to_string(),
                message,
            });
        }
    });
}

fn parse_assigned_loopback_port(line: &str) -> Option<u16> {
    let suffix = line.split_once("http://127.0.0.1:")?.1;
    let digits = suffix
        .chars()
        .take_while(char::is_ascii_digit)
        .collect::<String>();
    let port = digits.parse::<u16>().ok()?;
    (port != 0).then_some(port)
}

pub fn redact_runtime_log(message: &str, sensitive_values: &[String]) -> String {
    let mut redacted = message.to_string();
    for value in sensitive_values {
        if !value.is_empty() {
            redacted = redacted.replace(value, "[REDACTED]");
        }
    }
    for marker in [
        "authorization: bearer ",
        "bearer ",
        "--api-key ",
        "api-key=",
        "api_key=",
        "token=",
    ] {
        redacted = redact_value_after_marker(redacted, marker);
    }
    redact_absolute_path_tokens(&redacted)
}

fn redact_absolute_path_tokens(text: &str) -> String {
    text.split_inclusive(char::is_whitespace)
        .map(|segment| {
            let token = segment.trim_end_matches(char::is_whitespace);
            let whitespace = &segment[token.len()..];
            let candidate = token.trim_matches(|character: char| {
                matches!(
                    character,
                    '"' | '\'' | '(' | ')' | '[' | ']' | '{' | '}' | ',' | ';'
                )
            });
            let bytes = candidate.as_bytes();
            let windows_absolute = bytes.len() >= 3
                && bytes[0].is_ascii_alphabetic()
                && bytes[1] == b':'
                && matches!(bytes[2], b'\\' | b'/');
            let assigned_absolute = candidate.split_once('=').is_some_and(|(_, value)| {
                let value = value.as_bytes();
                value.first() == Some(&b'/')
                    || (value.len() >= 3
                        && value[0].is_ascii_alphabetic()
                        && value[1] == b':'
                        && matches!(value[2], b'\\' | b'/'))
            });
            if candidate.starts_with('/') || windows_absolute || assigned_absolute {
                format!("[REDACTED_PATH]{whitespace}")
            } else {
                segment.to_string()
            }
        })
        .collect()
}

fn redact_value_after_marker(mut text: String, marker: &str) -> String {
    let marker_lower = marker.to_ascii_lowercase();
    let mut search_from = 0;
    loop {
        let lower = text.to_ascii_lowercase();
        let Some(relative_start) = lower[search_from..].find(&marker_lower) else {
            break;
        };
        let value_start = search_from + relative_start + marker.len();
        let value_end = text[value_start..]
            .find(|character: char| {
                character.is_whitespace() || matches!(character, ',' | ';' | '"' | '\'')
            })
            .map_or(text.len(), |offset| value_start + offset);
        if value_end == value_start {
            search_from = value_start;
            continue;
        }
        text.replace_range(value_start..value_end, "[REDACTED]");
        search_from = value_start + "[REDACTED]".len();
    }
    text
}

fn server_exe_name() -> &'static str {
    if cfg!(windows) {
        "llama-server.exe"
    } else {
        "llama-server"
    }
}

pub fn llama_server_binary(app: &tauri::AppHandle) -> PathBuf {
    #[cfg(debug_assertions)]
    {
        let _ = app;
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("binaries")
            .join("llama")
            .join(server_exe_name())
    }
    #[cfg(not(debug_assertions))]
    {
        use tauri::Manager;
        app.path()
            .resource_dir()
            .unwrap_or_default()
            .join("llama")
            .join(server_exe_name())
    }
}

pub fn spawn_llama_server(
    binary: &Path,
    model_path: &str,
    api_key: &str,
) -> Result<Child, AppError> {
    if !binary.exists() {
        return Err(AppError::provider(
            "Built-in runtime not installed. Run scripts/setup-llama.ps1 (or setup-llama.sh on Mac/Linux) from the repo root to download it.",
        ));
    }

    let mut command = Command::new(binary);
    command
        .args(llama_server_arguments(model_path, api_key))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    configure_process_isolation(&mut command);
    command
        .spawn()
        .map_err(|error| AppError::provider(format!("Failed to start built-in runtime: {error}")))
}

fn llama_server_arguments(model_path: &str, api_key: &str) -> Vec<String> {
    vec![
        "-m".to_string(),
        model_path.to_string(),
        "--host".to_string(),
        "127.0.0.1".to_string(),
        "--port".to_string(),
        "0".to_string(),
        "--no-ui".to_string(),
        "-c".to_string(),
        "4096".to_string(),
        "--api-key".to_string(),
        api_key.to_string(),
    ]
}

pub fn generate_runtime_api_key() -> String {
    uuid::Uuid::new_v4().to_string()
}

pub async fn wait_for_assigned_port(
    manager: Arc<Mutex<SidecarState>>,
) -> Result<u16, RuntimeFailure> {
    for _ in 0..PORT_DISCOVERY_ATTEMPTS {
        {
            let mut sidecar = manager.lock().map_err(|_| RuntimeFailure {
                category: RuntimeFailureCategory::StateUnavailable,
                message: "Ark could not access the managed runtime state.".to_string(),
            })?;
            if !sidecar.reconcile_process() {
                return Err(sidecar
                    .safe_failure_with_excerpt()
                    .unwrap_or(RuntimeFailure {
                        category: RuntimeFailureCategory::ProcessExited,
                        message: "The managed runtime exited before assigning a port.".to_string(),
                    }));
            }
            if let Some(port) = sidecar.sync_assigned_port() {
                return Ok(port);
            }
        }
        tokio::time::sleep(READINESS_INTERVAL).await;
    }

    Err(RuntimeFailure {
        category: RuntimeFailureCategory::PortUnavailable,
        message:
            "The managed runtime did not report its OS-assigned loopback port within 30 seconds."
                .to_string(),
    })
}

#[cfg(windows)]
fn configure_process_isolation(command: &mut Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    command.creation_flags(CREATE_NEW_PROCESS_GROUP);
}

#[cfg(unix)]
fn configure_process_isolation(command: &mut Command) {
    use std::os::unix::process::CommandExt;
    command.process_group(0);
}

#[cfg(not(any(windows, unix)))]
fn configure_process_isolation(_command: &mut Command) {}

pub async fn wait_for_ready(
    manager: Arc<Mutex<SidecarState>>,
    port: u16,
    api_key: &str,
) -> Result<(), RuntimeFailure> {
    let client = Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|error| RuntimeFailure {
            category: RuntimeFailureCategory::HealthUnreachable,
            message: format!("Ark could not construct the runtime health client: {error}"),
        })?;
    let url = format!("http://127.0.0.1:{port}/health");
    let mut last_failure = RuntimeFailure {
        category: RuntimeFailureCategory::ReadinessTimeout,
        message: "The managed runtime did not report ready within 30 seconds.".to_string(),
    };

    for _ in 0..READINESS_ATTEMPTS {
        tokio::time::sleep(READINESS_INTERVAL).await;
        {
            let mut sidecar = manager.lock().map_err(|_| RuntimeFailure {
                category: RuntimeFailureCategory::StateUnavailable,
                message: "Ark could not access the managed runtime state.".to_string(),
            })?;
            if !sidecar.reconcile_process() {
                return Err(sidecar
                    .safe_failure_with_excerpt()
                    .unwrap_or(RuntimeFailure {
                        category: RuntimeFailureCategory::ProcessExited,
                        message: "The managed runtime exited before becoming ready.".to_string(),
                    }));
            }
        }

        match client
            .get(&url)
            .bearer_auth(api_key)
            .timeout(Duration::from_secs(2))
            .send()
            .await
        {
            Ok(response) if response.status().is_success() => {
                let mut sidecar = manager.lock().map_err(|_| RuntimeFailure {
                    category: RuntimeFailureCategory::StateUnavailable,
                    message: "Ark could not access the managed runtime state.".to_string(),
                })?;
                sidecar.mark_healthy();
                return Ok(());
            }
            Ok(response) if response.status() == StatusCode::UNAUTHORIZED => {
                return Err(RuntimeFailure {
                    category: RuntimeFailureCategory::AuthenticationFailed,
                    message: "The managed runtime rejected Ark's per-launch authentication token."
                        .to_string(),
                });
            }
            Ok(response) => {
                last_failure = RuntimeFailure {
                    category: RuntimeFailureCategory::HealthRejected,
                    message: format!(
                        "The managed runtime health endpoint returned HTTP {}.",
                        response.status()
                    ),
                };
            }
            Err(error) => {
                last_failure = RuntimeFailure {
                    category: RuntimeFailureCategory::HealthUnreachable,
                    message: format!(
                        "The managed runtime health endpoint was not reachable: {error}"
                    ),
                };
            }
        }
    }

    last_failure.message = format!(
        "The managed runtime did not become ready within 30 seconds. Last health result: {}",
        last_failure.message
    );
    Err(last_failure)
}

pub async fn check_health(port: u16, api_key: &str) -> Result<(), RuntimeFailure> {
    let response = Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|error| RuntimeFailure {
            category: RuntimeFailureCategory::HealthUnreachable,
            message: format!("Ark could not construct the runtime health client: {error}"),
        })?
        .get(format!("http://127.0.0.1:{port}/health"))
        .bearer_auth(api_key)
        .timeout(Duration::from_secs(2))
        .send()
        .await
        .map_err(|error| RuntimeFailure {
            category: RuntimeFailureCategory::HealthUnreachable,
            message: format!("The managed runtime health endpoint was not reachable: {error}"),
        })?;
    if response.status().is_success() {
        Ok(())
    } else {
        Err(RuntimeFailure {
            category: if response.status() == StatusCode::UNAUTHORIZED {
                RuntimeFailureCategory::AuthenticationFailed
            } else {
                RuntimeFailureCategory::HealthRejected
            },
            message: format!(
                "The managed runtime health endpoint returned HTTP {}.",
                response.status()
            ),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn log_redaction_covers_known_paths_secrets_and_common_auth_shapes() {
        let sensitive = vec![
            "C:\\Users\\person\\Models\\private.gguf".to_string(),
            "launch-secret".to_string(),
        ];
        let source = "model=C:\\Users\\person\\Models\\private.gguf Authorization: Bearer launch-secret token=other /Users/person/cache";
        let redacted = redact_runtime_log(source, &sensitive);
        assert!(!redacted.contains("person"));
        assert!(!redacted.contains("launch-secret"));
        assert!(!redacted.contains("other"));
        assert!(!redacted.contains("/Users/person"));
        assert!(redacted.matches("[REDACTED]").count() >= 3);
    }

    #[test]
    fn rotating_log_buffer_is_bounded_by_entry_count_and_bytes() {
        let mut buffer = RuntimeLogBuffer::default();
        for index in 0..(MAX_LOG_ENTRIES + 20) {
            buffer.push(RuntimeLogEntry {
                timestamp_ms: index as u128,
                stream: "stdout".to_string(),
                message: "x".repeat(1_024),
            });
        }
        assert!(buffer.entries.len() <= MAX_LOG_ENTRIES);
        assert!(buffer.bytes <= MAX_LOG_BYTES);
        assert!(
            buffer
                .entries
                .front()
                .expect("has retained entries")
                .timestamp_ms
                > 0
        );
    }

    #[test]
    fn process_exit_reconciles_to_crashed_with_a_category() {
        let child = short_lived_child();
        let mut sidecar = SidecarState::new();
        sidecar.attach_process(
            child,
            Some(11_435),
            "test.gguf".to_string(),
            "secret".to_string(),
        );
        thread::sleep(Duration::from_millis(150));
        assert!(!sidecar.reconcile_process());
        assert_eq!(sidecar.state(), RuntimeLifecycleState::Crashed);
        assert_eq!(
            sidecar.failure().map(|failure| failure.category),
            Some(RuntimeFailureCategory::ProcessExited)
        );
        let safe_failure = sidecar
            .safe_failure_with_excerpt()
            .expect("failure with excerpt");
        assert!(safe_failure.message.contains("Recent safe runtime output"));
        assert!(safe_failure.message.contains("[REDACTED]"));
        assert!(!safe_failure.message.contains("secret"));
        assert!(sidecar.diagnostics(false).recent_logs.is_empty());
        assert!(!sidecar.diagnostics(true).recent_logs.is_empty());
    }

    #[test]
    fn lifecycle_state_machine_covers_availability_health_degradation_and_stop() {
        let mut sidecar = SidecarState::new();
        assert_eq!(sidecar.state(), RuntimeLifecycleState::Stopped);
        sidecar.mark_unavailable_binary(Path::new("missing-runtime"));
        assert_eq!(sidecar.state(), RuntimeLifecycleState::UnavailableBinary);
        sidecar.mark_unavailable_model("missing.gguf", "model missing");
        assert_eq!(sidecar.state(), RuntimeLifecycleState::UnavailableModel);
        sidecar.begin_start("model.gguf", Path::new("runtime"));
        assert_eq!(sidecar.state(), RuntimeLifecycleState::Starting);
        sidecar.mark_healthy();
        assert_eq!(sidecar.state(), RuntimeLifecycleState::Healthy);
        sidecar.mark_failure(RuntimeFailureCategory::HealthUnreachable, "offline");
        assert_eq!(sidecar.state(), RuntimeLifecycleState::Degraded);
        sidecar.stop().expect("idle manager stops");
        assert_eq!(sidecar.state(), RuntimeLifecycleState::Stopped);
    }

    #[tokio::test]
    async fn stopping_the_sidecar_aborts_its_attached_proxy_and_clears_the_proxy_port() {
        let mut sidecar = SidecarState::new();
        assert_eq!(sidecar.proxy_port(), None);

        let task = tokio::spawn(async {
            loop {
                tokio::time::sleep(Duration::from_secs(3_600)).await;
            }
        });
        sidecar.attach_proxy(54_321, task);
        assert_eq!(sidecar.proxy_port(), Some(54_321));

        sidecar.stop().expect("idle manager stops");
        assert_eq!(
            sidecar.proxy_port(),
            None,
            "stop must clear the proxy port so a stopped runtime never reports a stale front door"
        );
    }

    #[tokio::test]
    async fn attaching_a_new_proxy_aborts_a_previously_attached_one() {
        let mut sidecar = SidecarState::new();

        let first_task = tokio::spawn(async {
            loop {
                tokio::time::sleep(Duration::from_secs(3_600)).await;
            }
        });
        sidecar.attach_proxy(11_111, first_task);

        let second_task = tokio::spawn(async {
            loop {
                tokio::time::sleep(Duration::from_secs(3_600)).await;
            }
        });
        sidecar.attach_proxy(22_222, second_task);

        assert_eq!(
            sidecar.proxy_port(),
            Some(22_222),
            "the second attached proxy's port must replace the first, not accumulate"
        );
    }

    #[tokio::test]
    async fn readiness_reports_authentication_failure_and_success_updates_health() {
        use crate::providers::test_support::{start_mock_stream_server, MockChunk};

        let unauthorized_port =
            start_mock_stream_server("HTTP/1.1 401 Unauthorized", vec![MockChunk::new("denied")])
                .await;
        let unauthorized = manager_with_live_child(unauthorized_port);
        let failure = wait_for_ready(Arc::clone(&unauthorized), unauthorized_port, "secret")
            .await
            .expect_err("unauthorized health response fails");
        assert_eq!(
            failure.category,
            RuntimeFailureCategory::AuthenticationFailed
        );
        unauthorized
            .lock()
            .expect("manager lock")
            .stop()
            .expect("cleanup child");

        let ready_port =
            start_mock_stream_server("HTTP/1.1 200 OK", vec![MockChunk::new("ready")]).await;
        let ready = manager_with_live_child(ready_port);
        wait_for_ready(Arc::clone(&ready), ready_port, "secret")
            .await
            .expect("successful health response is ready");
        assert_eq!(
            ready.lock().expect("manager lock").state(),
            RuntimeLifecycleState::Healthy
        );
        ready
            .lock()
            .expect("manager lock")
            .stop()
            .expect("cleanup child");
    }

    #[tokio::test]
    async fn log_reader_reports_the_os_assigned_port_to_the_manager() {
        let child = port_reporting_child(49_152);
        let mut sidecar = SidecarState::new();
        sidecar.attach_process(child, None, "test.gguf".to_string(), "secret".to_string());
        let manager = Arc::new(Mutex::new(sidecar));

        assert_eq!(
            wait_for_assigned_port(Arc::clone(&manager))
                .await
                .expect("port is discovered"),
            49_152
        );
        assert_eq!(manager.lock().expect("manager lock").port(), Some(49_152));
        manager
            .lock()
            .expect("manager lock")
            .stop()
            .expect("cleanup child");
    }

    #[test]
    fn dropping_the_manager_reaps_its_child_on_supported_desktop_platforms() {
        let child = long_lived_child();
        let pid = child.id();
        let mut sidecar = SidecarState::new();
        sidecar.attach_process(
            child,
            Some(11_435),
            "test.gguf".to_string(),
            "secret".to_string(),
        );
        sidecar.mark_healthy();
        drop(sidecar);

        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        loop {
            let mut system = sysinfo::System::new_all();
            system.refresh_all();
            if system.process(sysinfo::Pid::from_u32(pid)).is_none() {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "managed child {pid} survived manager drop"
            );
            thread::sleep(Duration::from_millis(25));
        }
    }

    #[test]
    fn launch_uses_loopback_os_assigned_port_auth_and_disables_upstream_ui() {
        let arguments = llama_server_arguments("model.gguf", "launch-secret");
        assert_eq!(
            arguments,
            vec![
                "-m",
                "model.gguf",
                "--host",
                "127.0.0.1",
                "--port",
                "0",
                "--no-ui",
                "-c",
                "4096",
                "--api-key",
                "launch-secret",
            ]
        );
    }

    #[test]
    fn assigned_port_parser_accepts_only_nonzero_ipv4_loopback_listener_lines() {
        assert_eq!(
            parse_assigned_loopback_port("srv  listening on http://127.0.0.1:49152"),
            Some(49_152)
        );
        assert_eq!(
            parse_assigned_loopback_port("listening on http://127.0.0.1:65535 (threads)"),
            Some(65_535)
        );
        assert_eq!(
            parse_assigned_loopback_port("listening on http://0.0.0.0:49152"),
            None
        );
        assert_eq!(
            parse_assigned_loopback_port("listening on http://127.0.0.1:0"),
            None
        );
        assert_eq!(
            parse_assigned_loopback_port("listening on http://127.0.0.1:70000"),
            None
        );
    }

    #[test]
    fn launch_secrets_are_distinct_random_version_four_uuids() {
        let first = generate_runtime_api_key();
        let second = generate_runtime_api_key();
        assert_ne!(first, second);
        for secret in [first, second] {
            let parsed = uuid::Uuid::parse_str(&secret).expect("valid UUID");
            assert_eq!(parsed.get_version(), Some(uuid::Version::Random));
        }
    }

    #[cfg(windows)]
    fn short_lived_child() -> Child {
        Command::new("cmd")
            .args(["/C", "echo token=secret & exit /B 7"])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn short-lived process")
    }

    #[cfg(unix)]
    fn short_lived_child() -> Child {
        Command::new("sh")
            .args(["-c", "echo token=secret; exit 7"])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn short-lived process")
    }

    #[cfg(windows)]
    fn long_lived_child() -> Child {
        Command::new("ping")
            .args(["-n", "30", "127.0.0.1"])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn long-lived process")
    }

    #[cfg(unix)]
    fn long_lived_child() -> Child {
        Command::new("sleep")
            .arg("30")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn long-lived process")
    }

    #[cfg(windows)]
    fn port_reporting_child(port: u16) -> Child {
        Command::new("cmd")
            .args([
                "/C",
                &format!("echo listening on http://127.0.0.1:{port} & ping -n 30 127.0.0.1"),
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn port-reporting process")
    }

    #[cfg(unix)]
    fn port_reporting_child(port: u16) -> Child {
        Command::new("sh")
            .args([
                "-c",
                &format!("echo 'listening on http://127.0.0.1:{port}'; sleep 30"),
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn port-reporting process")
    }

    fn manager_with_live_child(port: u16) -> Arc<Mutex<SidecarState>> {
        let child = long_lived_child();
        let mut sidecar = SidecarState::new();
        sidecar.attach_process(
            child,
            Some(port),
            "test.gguf".to_string(),
            "secret".to_string(),
        );
        Arc::new(Mutex::new(sidecar))
    }
}
