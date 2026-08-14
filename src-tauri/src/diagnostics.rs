//! ARC-001: the system diagnostics application workflow (hardware probing, provider health
//! check, optional live benchmark), extracted from `commands::mod`. `run_diagnostics` still
//! needs `&AppState` — it locks the database for provider/model lookups and reads the sidecar's
//! bearer token — so unlike `import_export`, this workflow is not reducible to a plain
//! `&Database` function without losing real behavior. Taking `&AppState` directly (the plain
//! data port) rather than Tauri's `State<AppState>` wrapper means this function, in principle,
//! can be constructed and driven in a test with no running Tauri app. `performance_guidance` has
//! no such dependency and is kept as a pure function, which is what makes it independently
//! unit-testable below without a Tauri runtime or a network connection.

use crate::chat::ChatMessage;
use crate::errors::AppError;
use crate::providers::{Provider, ProviderChatRequest, ProviderHealth, ProviderRegistry};
use crate::sidecar::RuntimeDiagnostics;
use crate::AppState;
use serde::Serialize;
use std::time::Instant;
use sysinfo::{Disks, System};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticsResult {
    pub os: String,
    pub cpu: String,
    pub cpu_cores: usize,
    pub total_memory_bytes: u64,
    pub available_memory_bytes: u64,
    pub total_disk_bytes: u64,
    pub available_disk_bytes: u64,
    pub gpu: String,
    pub provider_health: ProviderHealth,
    pub model_available: bool,
    pub benchmark: Option<BenchmarkResult>,
    /// UX-010: previously the benchmark's own error was discarded via `.ok()` — a failed
    /// benchmark and an unattempted one were indistinguishable to the user, both showing the
    /// same generic "performance is unknown" guidance. Populated only when a benchmark was
    /// actually attempted and failed, so its typed category/message can reach the UI directly.
    pub benchmark_failure: Option<AppError>,
    pub guidance: String,
    pub runtime: RuntimeDiagnostics,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BenchmarkResult {
    pub time_to_first_token_ms: Option<u128>,
    /// UX-010: wall-clock time from the first token to the last, i.e. `total_time_ms` minus
    /// `time_to_first_token_ms` — the portion `approximate_tokens_per_second` is actually
    /// computed against. Distinguishing this from `total_time_ms` is the fix for this task's own
    /// cited "whitespace token/s mixes load/generation" problem.
    pub generation_time_ms: Option<u128>,
    pub total_time_ms: u128,
    pub approximate_tokens_per_second: Option<f64>,
    pub output_preview: String,
}

pub async fn run_diagnostics(
    state: &AppState,
    provider_id: String,
    model: Option<String>,
    include_runtime_logs: bool,
) -> Result<DiagnosticsResult, AppError> {
    let provider_id =
        crate::validation::validate_entity_id(&provider_id, "Provider ID")?.to_string();
    let mut system = System::new_all();
    system.refresh_all();

    // UX-010: the workspace's own volume, not a meaningless sum across every disk on the
    // machine — a user with a large secondary drive would previously see plenty of "available
    // disk" even while the drive their workspace actually lives on was nearly full. Matched by
    // the longest mount-point prefix, so nested mounts (e.g. `/` vs `/home`) resolve correctly.
    let workspace_root = {
        let workspace_info = state
            .workspace
            .lock()
            .map_err(|_| AppError::new("state_error", "Could not access workspace state."))?
            .clone();
        std::path::PathBuf::from(workspace_info.root_path)
    };
    let disks = Disks::new_with_refreshed_list();
    let workspace_disk = disks
        .iter()
        .filter(|disk| workspace_root.starts_with(disk.mount_point()))
        .max_by_key(|disk| disk.mount_point().as_os_str().len());
    let total_disk_bytes = workspace_disk.map(|disk| disk.total_space()).unwrap_or(0);
    let available_disk_bytes = workspace_disk
        .map(|disk| disk.available_space())
        .unwrap_or(0);

    let provider = {
        let db = crate::commands::lock_db(state)?;
        db.get_provider(&provider_id)?
    };
    let bearer_token = crate::commands::built_in_bearer_token(state, &provider);
    let runtime = ProviderRegistry::create_with_bearer_token(provider.clone(), bearer_token)?;
    let provider_health = runtime.health().await;

    let selected_model = model.or(provider.default_model_id.clone());
    let local_models = {
        let db = crate::commands::lock_db(state)?;
        db.list_models(&provider_id)?
    };
    let model_available = selected_model
        .as_deref()
        .map(|name| {
            local_models
                .iter()
                .any(|model| model.name == name && model.is_available)
        })
        .unwrap_or(false);

    // UX-010: previously `.ok()` discarded the error, so a benchmark that failed (e.g. the
    // provider disconnected mid-stream) was indistinguishable from one that was never attempted
    // — both produced `benchmark: None` and the same generic "performance is unknown" guidance,
    // silently throwing away a typed, actionable error category/message.
    let (benchmark, benchmark_failure) = if provider_health.is_reachable {
        if let Some(model_name) = selected_model.clone() {
            match run_benchmark(runtime.as_ref(), model_name).await {
                Ok(result) => (Some(result), None),
                Err(error) => (None, Some(error)),
            }
        } else {
            (None, None)
        }
    } else {
        (None, None)
    };

    let guidance = performance_guidance(
        &provider.name,
        provider_health.is_reachable,
        model_available,
        benchmark.as_ref(),
        benchmark_failure.as_ref(),
    );

    let runtime = crate::commands::lock_sidecar(state)?.diagnostics(include_runtime_logs);

    Ok(DiagnosticsResult {
        os: format!(
            "{} {}",
            System::name().unwrap_or_else(|| "Unknown OS".to_string()),
            System::long_os_version().unwrap_or_default()
        )
        .trim()
        .to_string(),
        cpu: system
            .cpus()
            .first()
            .map(|cpu| cpu.brand().to_string())
            .unwrap_or_else(|| "Unknown CPU".to_string()),
        cpu_cores: system.cpus().len(),
        total_memory_bytes: system.total_memory(),
        available_memory_bytes: system.available_memory(),
        total_disk_bytes,
        available_disk_bytes,
        gpu: "GPU/accelerator detection is not available in the MVP diagnostics.".to_string(),
        provider_health,
        model_available,
        benchmark,
        benchmark_failure,
        guidance,
        runtime,
    })
}

async fn run_benchmark(runtime: &dyn Provider, model: String) -> Result<BenchmarkResult, AppError> {
    let start = Instant::now();
    let mut first_token_at = None;
    let mut output = String::new();

    runtime
        .stream_chat(
            ProviderChatRequest {
                model,
                messages: vec![ChatMessage {
                    role: "user".to_string(),
                    content: "Reply with one short sentence about local AI readiness.".to_string(),
                }],
                temperature: Some(0.2),
                max_tokens: Some(64),
                user_deadline: Some(std::time::Duration::from_secs(30)),
            },
            &mut |delta| {
                if first_token_at.is_none() {
                    first_token_at = Some(start.elapsed());
                }
                output.push_str(delta);
                Ok(())
            },
        )
        .await?;

    let total_time = start.elapsed();
    let token_estimate = output.split_whitespace().count().max(1) as f64;

    // UX-010: throughput is generation-only, not total wall-clock time. `total_time` also
    // includes however long the provider took to produce anything at all (model load, request
    // queueing, prompt processing) — mixing that into a "tokens per second" figure means slow
    // startup and slow generation are indistinguishable, and the number has nothing reliable to
    // say about either one specifically.
    let generation_time = first_token_at.map(|elapsed| total_time.saturating_sub(elapsed));
    let approximate_tokens_per_second = generation_time
        .filter(|duration| !duration.is_zero())
        .map(|duration| token_estimate / duration.as_secs_f64());

    Ok(BenchmarkResult {
        time_to_first_token_ms: first_token_at.map(|d| d.as_millis()),
        generation_time_ms: generation_time.map(|d| d.as_millis()),
        total_time_ms: total_time.as_millis(),
        approximate_tokens_per_second,
        output_preview: output.chars().take(160).collect(),
    })
}

fn performance_guidance(
    provider_name: &str,
    provider_reachable: bool,
    model_available: bool,
    benchmark: Option<&BenchmarkResult>,
    benchmark_failure: Option<&AppError>,
) -> String {
    if !provider_reachable {
        return format!("{provider_name} is not reachable. Start it to run local models.");
    }

    if !model_available {
        return format!("The selected model is not available. Install a model via {provider_name}, then refresh.");
    }

    // UX-010: a failed benchmark gets its own actual error message, not the same generic
    // "performance is unknown" text an unattempted benchmark would show.
    if let Some(failure) = benchmark_failure {
        return format!(
            "The benchmark failed ({}): {}",
            failure.code, failure.message
        );
    }

    let Some(benchmark) = benchmark else {
        return "Ark could not complete the benchmark. Chat may still work, but performance is unknown.".to_string();
    };

    let tokens_per_second = benchmark.approximate_tokens_per_second.unwrap_or(0.0);
    if tokens_per_second >= 25.0 {
        "Good for small and medium local models.".to_string()
    } else if tokens_per_second >= 8.0 {
        "Usable for small local models. Larger models may feel slow.".to_string()
    } else {
        "Expect slower responses. Prefer smaller quantized models on this device.".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::DEFAULT_PROVIDER_TYPE;
    use crate::providers::test_support::{start_mock_stream_server, MockChunk};
    use crate::providers::ProviderConfig;

    fn benchmark_result(tokens_per_second: f64) -> BenchmarkResult {
        BenchmarkResult {
            time_to_first_token_ms: Some(10),
            generation_time_ms: Some(90),
            total_time_ms: 100,
            approximate_tokens_per_second: Some(tokens_per_second),
            output_preview: "preview".to_string(),
        }
    }

    #[test]
    fn performance_guidance_reports_unreachable_provider_before_anything_else() {
        let guidance = performance_guidance("Ollama", false, false, None, None);
        assert!(guidance.contains("Ollama is not reachable"));
    }

    #[test]
    fn performance_guidance_reports_missing_model_when_provider_is_reachable() {
        let guidance = performance_guidance("Ollama", true, false, None, None);
        assert!(guidance.contains("not available"));
    }

    #[test]
    fn performance_guidance_reports_unknown_performance_when_benchmark_did_not_run() {
        let guidance = performance_guidance("Ollama", true, true, None, None);
        assert!(guidance.contains("performance is unknown"));
    }

    #[test]
    fn performance_guidance_reports_the_actual_failure_when_the_benchmark_errored() {
        let failure = AppError::new(
            "stream_incomplete",
            "The provider closed the connection early.",
        );
        let guidance = performance_guidance("Ollama", true, true, None, Some(&failure));
        assert!(guidance.contains("stream_incomplete"));
        assert!(guidance.contains("closed the connection early"));
    }

    #[test]
    fn performance_guidance_ranks_fast_throughput_as_good() {
        let benchmark = benchmark_result(30.0);
        assert_eq!(
            performance_guidance("Ollama", true, true, Some(&benchmark), None),
            "Good for small and medium local models."
        );
    }

    #[test]
    fn performance_guidance_ranks_moderate_throughput_as_usable() {
        let benchmark = benchmark_result(12.0);
        assert_eq!(
            performance_guidance("Ollama", true, true, Some(&benchmark), None),
            "Usable for small local models. Larger models may feel slow."
        );
    }

    #[test]
    fn performance_guidance_ranks_slow_throughput_as_expect_slower_responses() {
        let benchmark = benchmark_result(2.0);
        assert_eq!(
            performance_guidance("Ollama", true, true, Some(&benchmark), None),
            "Expect slower responses. Prefer smaller quantized models on this device."
        );
    }

    #[test]
    fn performance_guidance_threshold_boundaries_are_inclusive() {
        assert_eq!(
            performance_guidance("Ollama", true, true, Some(&benchmark_result(25.0)), None),
            "Good for small and medium local models."
        );
        assert_eq!(
            performance_guidance("Ollama", true, true, Some(&benchmark_result(8.0)), None),
            "Usable for small local models. Larger models may feel slow."
        );
    }

    fn ollama_config_for_port(port: u16) -> ProviderConfig {
        ProviderConfig {
            id: "provider".to_string(),
            name: "Ollama".to_string(),
            provider_type: DEFAULT_PROVIDER_TYPE.to_string(),
            base_url: Some(format!("http://127.0.0.1:{port}")),
            api_key_ref: None,
            default_model_id: None,
            default_temperature: Some(0.7),
            default_max_tokens: Some(2048),
            is_local: true,
            allow_insecure_remote: false,
            destination_class: "loopback".to_string(),
            capabilities: crate::providers::ProviderCapabilities::for_provider_type(
                DEFAULT_PROVIDER_TYPE,
            ),
            is_enabled: true,
            created_at: "2026-07-02T00:00:00Z".to_string(),
            updated_at: "2026-07-02T00:00:00Z".to_string(),
        }
    }

    /// ARC-001 acceptance evidence: `run_benchmark` is a private function reachable only through
    /// this crate; exercising it end-to-end against a real (mock) provider socket is possible
    /// specifically because `providers::test_support` was made `pub(crate)`.
    #[tokio::test]
    async fn run_benchmark_reports_time_to_first_token_and_a_content_preview() {
        let port = start_mock_stream_server(
            "HTTP/1.1 200 OK",
            vec![MockChunk::new(
                "{\"message\":{\"content\":\"Local AI\"},\"done\":false}\n\
                 {\"message\":{\"content\":\" is ready.\"},\"done\":false}\n\
                 {\"message\":{\"content\":\"\"},\"done\":true,\"prompt_eval_count\":3,\"eval_count\":4}\n"
                    .as_bytes()
                    .to_vec(),
            )],
        )
        .await;

        let runtime =
            ProviderRegistry::create(ollama_config_for_port(port)).expect("runtime constructs");
        let result = run_benchmark(runtime.as_ref(), "test-model".to_string())
            .await
            .expect("benchmark completes");

        assert!(result.time_to_first_token_ms.is_some());
        assert_eq!(result.output_preview, "Local AI is ready.");
        assert!(result.approximate_tokens_per_second.unwrap_or(0.0) > 0.0);
        // UX-010: generation_time_ms is the portion throughput is actually computed against —
        // it must exist whenever a first token arrived, and can never exceed total_time_ms
        // (total time includes the time-to-first-token phase generation_time_ms excludes).
        let generation_time_ms = result.generation_time_ms.expect("first token arrived");
        assert!(generation_time_ms <= result.total_time_ms);
    }

    #[tokio::test]
    async fn run_benchmark_propagates_a_protocol_error_from_a_truncated_stream() {
        let port = start_mock_stream_server(
            "HTTP/1.1 200 OK",
            vec![MockChunk::new(
                "{\"message\":{\"content\":\"Partial\"},\"done\":false}\n"
                    .as_bytes()
                    .to_vec(),
            )],
        )
        .await;

        let runtime =
            ProviderRegistry::create(ollama_config_for_port(port)).expect("runtime constructs");
        let error = run_benchmark(runtime.as_ref(), "test-model".to_string())
            .await
            .expect_err("truncated stream must fail");
        assert_eq!(error.code, "stream_incomplete");
    }
}
