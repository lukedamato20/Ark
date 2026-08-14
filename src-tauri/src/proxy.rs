//! SEC-002: a minimal loopback-only authenticating reverse proxy placed in front of the
//! managed llama-server child process.
//!
//! llama.cpp b9859's own HTTP server (`server-http.cpp`) exempts `/health`, `/v1/health`,
//! `/models`, `/v1/models`, `/`, and embedded UI assets from its `--api-key` check, and
//! reflects any request `Origin` header into `Access-Control-Allow-Origin` with no restrictive
//! CORS/trusted-host control available. Waiting on an upstream fix has no committed timeline
//! (see `implementation-plan.md`'s SEC-002 entry for the full threat-model reasoning), so this
//! proxy closes both gaps for Ark's own managed runtime without touching llama.cpp itself:
//!
//! - Every request, on every path with no exemption, must carry `Authorization: Bearer
//!   <per-launch secret>` or it never reaches llama-server at all.
//! - No response this proxy sends ever carries an `Access-Control-Allow-*` header — not even
//!   llama-server's own reflected one, which is stripped before forwarding back — so a
//!   cross-origin browser page can never read a response even in the hypothetical case it
//!   guessed the token, and a real CORS preflight (which never carries the caller's intended
//!   `Authorization` header) always fails the auth check first.
//!
//! Ark's own traffic to the built-in provider goes through `reqwest` from Rust, which is never
//! subject to browser CORS restrictions, so none of the above affects Ark's own use of the
//! runtime it manages.

use bytes::Bytes;
use futures_util::StreamExt;
use http_body_util::combinators::BoxBody;
use http_body_util::{BodyExt, Empty, Full, StreamBody};
use hyper::body::{Frame, Incoming};
use hyper::header::HeaderName;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::task::JoinHandle;

/// Chat prompts are small JSON payloads; this is a defensive ceiling against an oversized or
/// malicious request body, not a tuned limit for any real workload.
const MAX_REQUEST_BODY_BYTES: usize = 16 * 1024 * 1024;

type ProxyBody = BoxBody<Bytes, ProxyBodyError>;

#[derive(Debug)]
struct ProxyBodyError(String);

impl std::fmt::Display for ProxyBodyError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

impl std::error::Error for ProxyBodyError {}

struct ProxyContext {
    target_base: String,
    api_key: String,
    client: reqwest::Client,
}

/// Binds a loopback-only listener on an OS-assigned port and starts accepting connections in
/// the background. Returns the assigned port and a handle the caller must abort to stop the
/// listener — `SidecarState` owns the handle for the lifetime of the managed runtime it fronts.
pub async fn spawn_auth_proxy(
    target_port: u16,
    api_key: String,
) -> std::io::Result<(u16, JoinHandle<()>)> {
    let listener = TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0))).await?;
    let port = listener.local_addr()?.port();

    let context = Arc::new(ProxyContext {
        target_base: format!("http://127.0.0.1:{target_port}"),
        api_key,
        client: reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .expect("proxy HTTP client configuration is static and valid"),
    });

    let handle = tokio::spawn(async move {
        loop {
            let (stream, _) = match listener.accept().await {
                Ok(accepted) => accepted,
                Err(_) => {
                    tokio::time::sleep(Duration::from_millis(50)).await;
                    continue;
                }
            };
            let io = TokioIo::new(stream);
            let context = Arc::clone(&context);
            tokio::spawn(async move {
                let service =
                    service_fn(move |request| handle_request(Arc::clone(&context), request));
                let _ = http1::Builder::new().serve_connection(io, service).await;
            });
        }
    });

    Ok((port, handle))
}

async fn handle_request(
    context: Arc<ProxyContext>,
    request: Request<Incoming>,
) -> Result<Response<ProxyBody>, Infallible> {
    Ok(forward(context, request)
        .await
        .unwrap_or_else(|| simple_response(StatusCode::BAD_GATEWAY)))
}

async fn forward(
    context: Arc<ProxyContext>,
    request: Request<Incoming>,
) -> Option<Response<ProxyBody>> {
    if !is_authorized(&request, &context.api_key) {
        return Some(simple_response(StatusCode::UNAUTHORIZED));
    }

    let method = request.method().clone();
    let path_and_query = request
        .uri()
        .path_and_query()
        .map(|value| value.as_str())
        .unwrap_or("/")
        .to_string();
    let headers = request.headers().clone();

    let collected = BodyExt::collect(request.into_body()).await.ok()?;
    let body_bytes = collected.to_bytes();
    if body_bytes.len() > MAX_REQUEST_BODY_BYTES {
        return Some(simple_response(StatusCode::PAYLOAD_TOO_LARGE));
    }

    let reqwest_method =
        reqwest::Method::from_bytes(method.as_str().as_bytes()).unwrap_or(reqwest::Method::GET);
    let url = format!("{}{}", context.target_base, path_and_query);
    let mut outbound = context.client.request(reqwest_method, url);
    for (name, value) in headers.iter() {
        if is_forwardable_request_header(name) {
            if let Ok(value_str) = value.to_str() {
                outbound = outbound.header(name.as_str(), value_str);
            }
        }
    }
    let response = outbound.body(body_bytes).send().await.ok()?;

    let status =
        StatusCode::from_u16(response.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
    let mut builder = Response::builder().status(status);
    for (name, value) in response.headers().iter() {
        if is_forwardable_response_header(name) {
            builder = builder.header(name.as_str(), value.as_bytes());
        }
    }

    let stream = response.bytes_stream().map(|chunk| {
        chunk
            .map(Frame::data)
            .map_err(|error| ProxyBodyError(error.to_string()))
    });
    let body = BodyExt::boxed(StreamBody::new(stream));
    builder.body(body).ok()
}

fn is_authorized(request: &Request<Incoming>, api_key: &str) -> bool {
    let Some(header_value) = request.headers().get(hyper::header::AUTHORIZATION) else {
        return false;
    };
    let Ok(header_str) = header_value.to_str() else {
        return false;
    };
    header_str
        .strip_prefix("Bearer ")
        .is_some_and(|token| token == api_key)
}

/// Hop-by-hop headers (RFC 7230 §6.1) are connection-specific and must never be forwarded
/// through a proxy; `host` is excluded separately so `reqwest` sets its own for the target.
fn is_hop_by_hop(name: &HeaderName) -> bool {
    matches!(
        name.as_str(),
        "connection"
            | "keep-alive"
            | "proxy-authenticate"
            | "proxy-authorization"
            | "te"
            | "trailers"
            | "transfer-encoding"
            | "upgrade"
    )
}

fn is_forwardable_request_header(name: &HeaderName) -> bool {
    !is_hop_by_hop(name) && name != hyper::header::HOST
}

/// Excludes hop-by-hop headers, framing headers this proxy recomputes for its own streamed
/// response (`content-length`), and — the core CORS-sanitization behavior — any
/// `access-control-*` header llama-server may have set (including its reflected-`Origin`
/// `Access-Control-Allow-Origin`), so this proxy's own response never carries one.
fn is_forwardable_response_header(name: &HeaderName) -> bool {
    !is_hop_by_hop(name)
        && name != hyper::header::CONTENT_LENGTH
        && !name.as_str().starts_with("access-control-")
}

fn simple_response(status: StatusCode) -> Response<ProxyBody> {
    Response::builder()
        .status(status)
        .body(empty_body())
        .unwrap_or_else(|_| {
            let mut response = Response::new(empty_body());
            *response.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
            response
        })
}

fn empty_body() -> ProxyBody {
    BodyExt::boxed(Empty::<Bytes>::new().map_err(|never: Infallible| match never {}))
}

#[allow(dead_code)]
fn full_body(bytes: Bytes) -> ProxyBody {
    BodyExt::boxed(Full::new(bytes).map_err(|never: Infallible| match never {}))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::convert::Infallible;
    use tokio::net::TcpListener as TestListener;

    async fn spawn_stub_upstream() -> (u16, JoinHandle<()>) {
        let listener = TestListener::bind(SocketAddr::from(([127, 0, 0, 1], 0)))
            .await
            .expect("bind stub upstream");
        let port = listener.local_addr().expect("local addr").port();
        let handle = tokio::spawn(async move {
            loop {
                let Ok((stream, _)) = listener.accept().await else {
                    continue;
                };
                let io = TokioIo::new(stream);
                tokio::spawn(async move {
                    let service = service_fn(|request: Request<Incoming>| async move {
                        let path = request.uri().path().to_string();
                        let has_auth = request.headers().contains_key(hyper::header::AUTHORIZATION);
                        let origin_reflected =
                            request.headers().get(hyper::header::ORIGIN).cloned();

                        let mut builder = Response::builder().status(StatusCode::OK);
                        // Simulate llama.cpp's real behavior: reflect Origin on every route,
                        // including the exempt ones, regardless of auth.
                        if let Some(origin) = origin_reflected {
                            builder = builder.header("access-control-allow-origin", origin);
                        }
                        let payload = format!("path={path};auth={has_auth}");
                        Ok::<_, Infallible>(
                            builder
                                .body(full_body(Bytes::from(payload)))
                                .expect("build stub response"),
                        )
                    });
                    let _ = http1::Builder::new().serve_connection(io, service).await;
                });
            }
        });
        (port, handle)
    }

    #[tokio::test]
    async fn unauthenticated_requests_are_rejected_on_every_path_including_exempt_ones() {
        let (upstream_port, upstream_handle) = spawn_stub_upstream().await;
        let (proxy_port, proxy_handle) =
            spawn_auth_proxy(upstream_port, "correct-token".to_string())
                .await
                .expect("spawn proxy");
        let client = reqwest::Client::new();

        for path in [
            "/health",
            "/v1/health",
            "/models",
            "/v1/models",
            "/",
            "/completion",
        ] {
            let response = client
                .get(format!("http://127.0.0.1:{proxy_port}{path}"))
                .send()
                .await
                .unwrap_or_else(|error| panic!("request to {path} failed: {error}"));
            assert_eq!(
                response.status(),
                reqwest::StatusCode::UNAUTHORIZED,
                "path {path} should require authentication"
            );
            assert!(
                !response
                    .headers()
                    .contains_key("access-control-allow-origin"),
                "path {path} must never carry a CORS header on an unauthenticated response"
            );
        }

        proxy_handle.abort();
        upstream_handle.abort();
    }

    #[tokio::test]
    async fn a_wrong_bearer_token_is_rejected() {
        let (upstream_port, upstream_handle) = spawn_stub_upstream().await;
        let (proxy_port, proxy_handle) =
            spawn_auth_proxy(upstream_port, "correct-token".to_string())
                .await
                .expect("spawn proxy");
        let client = reqwest::Client::new();

        let response = client
            .get(format!("http://127.0.0.1:{proxy_port}/health"))
            .bearer_auth("wrong-token")
            .send()
            .await
            .expect("request completes");
        assert_eq!(response.status(), reqwest::StatusCode::UNAUTHORIZED);

        proxy_handle.abort();
        upstream_handle.abort();
    }

    #[tokio::test]
    async fn an_authenticated_request_is_forwarded_and_upstream_cors_is_stripped() {
        let (upstream_port, upstream_handle) = spawn_stub_upstream().await;
        let (proxy_port, proxy_handle) =
            spawn_auth_proxy(upstream_port, "correct-token".to_string())
                .await
                .expect("spawn proxy");
        let client = reqwest::Client::new();

        let response = client
            .get(format!("http://127.0.0.1:{proxy_port}/models"))
            .bearer_auth("correct-token")
            .header("origin", "https://evil.example")
            .send()
            .await
            .expect("request completes");
        assert_eq!(response.status(), reqwest::StatusCode::OK);
        assert!(
            !response
                .headers()
                .contains_key("access-control-allow-origin"),
            "the proxy must strip llama-server's reflected CORS header, not forward it"
        );
        let body = response.text().await.expect("response body");
        assert!(body.contains("path=/models"));
        assert!(body.contains("auth=true"));

        proxy_handle.abort();
        upstream_handle.abort();
    }

    #[tokio::test]
    async fn a_missing_authorization_header_on_a_simulated_preflight_is_rejected() {
        // Browsers never attach the caller's intended Authorization header to an OPTIONS
        // preflight — this proves the proxy needs no OPTIONS special-casing: a preflight fails
        // the same auth check as any other unauthenticated request, so the real cross-origin
        // request it was gating is never sent.
        let (upstream_port, upstream_handle) = spawn_stub_upstream().await;
        let (proxy_port, proxy_handle) =
            spawn_auth_proxy(upstream_port, "correct-token".to_string())
                .await
                .expect("spawn proxy");
        let client = reqwest::Client::new();

        let response = client
            .request(
                reqwest::Method::OPTIONS,
                format!("http://127.0.0.1:{proxy_port}/completion"),
            )
            .header("origin", "https://evil.example")
            .header("access-control-request-method", "POST")
            .send()
            .await
            .expect("request completes");
        assert_eq!(response.status(), reqwest::StatusCode::UNAUTHORIZED);
        assert!(!response
            .headers()
            .contains_key("access-control-allow-origin"));

        proxy_handle.abort();
        upstream_handle.abort();
    }
}
