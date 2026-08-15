//! CMP-004: the second built-in tool (see `tools.rs`), bringing genuinely untrusted third-party
//! content into Ark for the first time — every result returned here is treated by
//! `generation.rs`'s `build_search_disclosure` as ADR 0002 §1's channel-3 "retrieved/tool-result"
//! data: quoted, labeled, never merged into the system channel. This module owns only the Brave
//! Search HTTP call and result shaping; the capability-scope/grant/audit/preview mechanics are
//! entirely `tool_policy.rs`/`tools.rs`'s, reused as-is.
//!
//! Deliberately does **not** go through `security::enforce_destination_policy` or the
//! `Provider`/`ProviderConfig` abstraction. The former exists to classify *user-supplied* base
//! URLs (loopback vs. private-LAN vs. public) — Brave's endpoint is a fixed, always-HTTPS,
//! build-time constant with no such ambiguity to gate. The latter is purpose-built for LLM
//! inference backends (chat streaming, model listing/pull/delete); a search API doesn't fit its
//! `Provider` trait and forcing it in would blur a single-responsibility dispatch point
//! (`ProviderRegistry::create_with_bearer_token`) that this codebase deliberately keeps narrow.
//!
//! Errors get their own distinct codes rather than relying on `From<reqwest::Error> for
//! AppError`'s blanket conversion (`errors.rs`) — that conversion's wording is Ollama-specific
//! ("Check that Ollama is running"), which would misinform a web-search failure and would also
//! collide its `provider_error` code with real LLM-provider failures, breaking this task's own
//! "failures distinguish search, fetch, parsing, and model errors" acceptance criterion.

use crate::errors::AppError;
use crate::tool_policy::{IdempotencyPolicy, SideEffectPreview};
use crate::tools::{ToolInvocationAttempt, WEB_SEARCH_TOOL_ID};
use crate::AppState;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

const BRAVE_SEARCH_URL: &str = "https://api.search.brave.com/res/v1/web/search";
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);
/// Capped both for prompt-injection surface area (fewer untrusted blocks reaching the model) and
/// token budget — Brave itself can return far more per query.
const MAX_RESULTS: usize = 6;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchCitation {
    pub title: String,
    pub url: String,
    pub snippet: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebSearchResult {
    pub citations: Vec<SearchCitation>,
}

#[derive(Debug, Deserialize)]
struct BraveSearchResponse {
    web: Option<BraveWebResults>,
}

#[derive(Debug, Deserialize)]
struct BraveWebResults {
    #[serde(default)]
    results: Vec<BraveResult>,
}

#[derive(Debug, Deserialize)]
struct BraveResult {
    title: String,
    url: String,
    #[serde(default)]
    description: String,
}

/// Parses a Brave Search API response body into citations — a pure function, independently
/// testable without a network call or a running HTTP server.
fn parse_brave_response(body: &str) -> Result<Vec<SearchCitation>, AppError> {
    let parsed: BraveSearchResponse = serde_json::from_str(body).map_err(|error| {
        AppError::new(
            "web_search_failed",
            format!("Could not parse Brave Search's response: {error}"),
        )
    })?;
    Ok(parsed
        .web
        .map(|web| web.results)
        .unwrap_or_default()
        .into_iter()
        .take(MAX_RESULTS)
        .map(|result| SearchCitation {
            title: result.title,
            url: result.url,
            snippet: result.description,
        })
        .collect())
}

fn http_client() -> Result<Client, AppError> {
    Client::builder()
        .connect_timeout(CONNECT_TIMEOUT)
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|error| {
            AppError::new(
                "web_search_failed",
                format!("Could not construct the web search HTTP client: {error}"),
            )
        })
}

async fn brave_search(api_key: &str, query: &str) -> Result<Vec<SearchCitation>, AppError> {
    let client = http_client()?;
    match client
        .get(BRAVE_SEARCH_URL)
        .header("X-Subscription-Token", api_key)
        .header("Accept", "application/json")
        .query(&[("q", query)])
        .timeout(REQUEST_TIMEOUT)
        .send()
        .await
    {
        Ok(response) if response.status().is_success() => {
            let body = response.text().await.map_err(|error| {
                AppError::new(
                    "web_search_failed",
                    format!("Could not read Brave Search's response body: {error}"),
                )
            })?;
            parse_brave_response(&body)
        }
        Ok(response) if response.status().as_u16() == 401 || response.status().as_u16() == 403 => {
            Err(AppError::new(
                "web_search_unauthorized",
                "Brave Search rejected the configured API key. Check the key in Settings → Tools.",
            ))
        }
        Ok(response) if response.status().as_u16() == 429 => Err(AppError::new(
            "web_search_rate_limited",
            "Brave Search's rate limit was hit. Wait and try again.",
        )),
        Ok(response) => Err(AppError::new(
            "web_search_failed",
            format!("Brave Search returned HTTP {}.", response.status()),
        )),
        Err(error) if error.is_timeout() => Err(AppError::new(
            "web_search_timeout",
            "The search request to Brave Search timed out.",
        )),
        Err(error) if error.is_connect() => Err(AppError::new(
            "web_search_unreachable",
            "Could not reach Brave Search. Check the network connection.",
        )),
        Err(error) => Err(AppError::new(
            "web_search_failed",
            format!("Web search request failed: {error}"),
        )),
    }
}

/// Builds the human-readable preview shown before a search runs — names both the exact query and
/// the destination provider, satisfying this task's own "queries and destination provider are
/// previewed/disclosed" acceptance criterion literally. A live search always has a real cost and
/// can return different results each time, so — like every side-effecting tool in this codebase
/// — it defaults to `RequiresFreshApproval` rather than claiming a permissive idempotency it
/// cannot honestly promise.
pub fn preview_web_search(query: &str) -> SideEffectPreview {
    SideEffectPreview {
        tool_id: WEB_SEARCH_TOOL_ID.to_string(),
        summary: format!("Send this query to Brave Search: \"{query}\""),
        idempotency: IdempotencyPolicy::RequiresFreshApproval,
    }
}

/// What `search_web` decided: either the search actually ran (with its result), or it was
/// blocked pending approval — mirrors `tools::ToolInvocationAttempt`'s shape but carries the
/// result through the `Applied` case, since (unlike Notes' writes) this orchestration function
/// performs the whole action itself rather than leaving that to its caller.
pub enum WebSearchOutcome {
    Applied(WebSearchResult),
    ApprovalRequired,
}

/// The orchestration function `commands::search_web` calls: checks a credential is configured
/// (before even offering approval — approving a grant for a tool that can't function yet is a
/// dead end), authorizes the invocation, performs the search, and records a redacted audit event.
/// Five short, explicitly scoped blocks, mirroring `provider_management::pull_ollama_model`'s own
/// documented discipline — a `std::sync::MutexGuard` is `!Send`, so the compiler itself refuses
/// to let one cross an `.await` on this project's multi-threaded runtime; the block scoping here
/// makes that guarantee visible rather than relying on it merely compiling by accident.
pub async fn search_web(
    state: &AppState,
    query: String,
    approve: bool,
) -> Result<WebSearchOutcome, AppError> {
    let attempt = {
        let db = crate::commands::lock_db(state)?;
        if db.get_tool_secret_ref(WEB_SEARCH_TOOL_ID)?.is_none() {
            return Err(AppError::new(
                "tool_secret_not_configured",
                "Add a Brave Search API key in Settings → Tools before using web search.",
            ));
        }
        crate::tools::authorize_tool_invocation(&db, WEB_SEARCH_TOOL_ID, approve)?
    };
    if matches!(attempt, ToolInvocationAttempt::ApprovalRequired) {
        return Ok(WebSearchOutcome::ApprovalRequired);
    }

    let reference = {
        let db = crate::commands::lock_db(state)?;
        db.get_tool_secret_ref(WEB_SEARCH_TOOL_ID)?.ok_or_else(|| {
            AppError::new(
                "tool_secret_not_configured",
                "Add a Brave Search API key in Settings → Tools before using web search.",
            )
        })?
    };
    let api_key =
        tokio::task::spawn_blocking(move || crate::secret_store::read_tool_secret(&reference))
            .await
            .map_err(|_| {
                AppError::new(
                    "secret_store_failed",
                    "Credential-store worker did not complete. Retry.",
                )
            })??;

    let citations = brave_search(&api_key, &query).await?;

    {
        let db = crate::commands::lock_db(state)?;
        db.record_tool_invocation(
            WEB_SEARCH_TOOL_ID,
            &format!(
                "query: {} chars, {} results",
                query.chars().count(),
                citations.len()
            ),
        )?;
    }

    Ok(WebSearchOutcome::Applied(WebSearchResult { citations }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_web_search_discloses_the_exact_query_and_destination() {
        let preview = preview_web_search("latest rust release");
        assert_eq!(preview.tool_id, WEB_SEARCH_TOOL_ID);
        assert!(preview.summary.contains("latest rust release"));
        assert!(preview.summary.contains("Brave Search"));
        assert_eq!(
            preview.idempotency,
            IdempotencyPolicy::RequiresFreshApproval
        );
    }

    #[test]
    fn parse_brave_response_maps_results_and_caps_at_max_results() {
        let mut results = String::new();
        for i in 0..10 {
            if i > 0 {
                results.push(',');
            }
            results.push_str(&format!(
                r#"{{"title": "Result {i}", "url": "https://example.test/{i}", "description": "Snippet {i}"}}"#
            ));
        }
        let body = format!(r#"{{"web": {{"results": [{results}]}}}}"#);

        let citations = parse_brave_response(&body).expect("valid response parses");
        assert_eq!(citations.len(), MAX_RESULTS);
        assert_eq!(citations[0].title, "Result 0");
        assert_eq!(citations[0].url, "https://example.test/0");
        assert_eq!(citations[0].snippet, "Snippet 0");
    }

    #[test]
    fn parse_brave_response_handles_a_response_with_no_web_results() {
        let citations = parse_brave_response(r#"{"query": {"original": "x"}}"#)
            .expect("a response with no web key still parses");
        assert!(citations.is_empty());
    }

    #[test]
    fn parse_brave_response_rejects_malformed_json_with_a_distinct_code() {
        let error = parse_brave_response("not json").unwrap_err();
        assert_eq!(error.code, "web_search_failed");
    }
}
