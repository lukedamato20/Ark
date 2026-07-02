use crate::errors::AppError;
use reqwest::Client;
use std::net::TcpListener;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

pub struct SidecarState {
    pub process: Option<Child>,
    pub port: Option<u16>,
    pub model_path: Option<String>,
}

impl SidecarState {
    pub fn new() -> Self {
        Self { process: None, port: None, model_path: None }
    }

    pub fn is_running(&mut self) -> bool {
        let Some(child) = &mut self.process else { return false };
        match child.try_wait() {
            Ok(Some(_)) => {
                self.process = None;
                self.port = None;
                self.model_path = None;
                false
            }
            Ok(None) => true,
            Err(_) => false,
        }
    }

    pub fn stop(&mut self) {
        if let Some(mut child) = self.process.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        self.port = None;
        self.model_path = None;
    }
}

pub fn find_free_port(start: u16) -> Option<u16> {
    (start..start.saturating_add(100)).find(|&port| TcpListener::bind(("127.0.0.1", port)).is_ok())
}

fn server_exe_name() -> &'static str {
    if cfg!(windows) { "llama-server.exe" } else { "llama-server" }
}

/// Returns the path to the llama-server binary.
///
/// In dev builds, looks in `src-tauri/binaries/llama/` (populated by scripts/setup-llama.ps1).
/// In release builds, looks in `{resources_dir}/llama/` (bundled by Tauri from the same source).
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

pub fn spawn_llama_server(binary: &PathBuf, model_path: &str, port: u16) -> Result<Child, AppError> {
    if !binary.exists() {
        return Err(AppError::provider(format!(
            "Built-in runtime not installed. Run scripts/setup-llama.ps1 (or setup-llama.sh on Mac/Linux) from the repo root to download it."
        )));
    }

    Command::new(binary)
        .args(["-m", model_path, "--host", "127.0.0.1", "--port", &port.to_string(), "-c", "4096"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| AppError::provider(format!("Failed to start built-in runtime: {e}")))
}

pub async fn wait_for_ready(port: u16) -> bool {
    let client = Client::new();
    let url = format!("http://127.0.0.1:{port}/health");
    for _ in 0..60 {
        tokio::time::sleep(Duration::from_millis(500)).await;
        if let Ok(resp) = client.get(&url).timeout(Duration::from_secs(2)).send().await {
            if resp.status().is_success() {
                return true;
            }
        }
    }
    false
}
