//! Deterministic provider protocol fixtures. The raw loopback server exposes exact byte/chunk
//! control without a network dependency or an HTTP-mocking abstraction that hides framing.
#![cfg(test)]

use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::mpsc;

pub struct MockChunk {
    pub delay: Duration,
    pub bytes: Vec<u8>,
}

impl MockChunk {
    pub fn new(bytes: impl Into<Vec<u8>>) -> Self {
        Self {
            delay: Duration::ZERO,
            bytes: bytes.into(),
        }
    }

    pub fn delayed(delay: Duration, bytes: impl Into<Vec<u8>>) -> Self {
        Self {
            delay,
            bytes: bytes.into(),
        }
    }

    /// Splits a protocol fixture at every possible byte boundary. This deliberately fragments
    /// multi-byte UTF-8 and JSON tokens so adapter buffering is exercised by the real socket
    /// path rather than only by an in-memory parser helper.
    pub fn fragment_every_byte(bytes: impl AsRef<[u8]>) -> Vec<Self> {
        bytes
            .as_ref()
            .iter()
            .map(|byte| Self::new(vec![*byte]))
            .collect()
    }
}

pub struct MockResponsePlan {
    pub status_line: String,
    pub header_delay: Duration,
    pub chunks: Vec<MockChunk>,
}

impl MockResponsePlan {
    pub fn new(status_line: impl Into<String>, chunks: Vec<MockChunk>) -> Self {
        Self {
            status_line: status_line.into(),
            header_delay: Duration::ZERO,
            chunks,
        }
    }

    pub fn with_header_delay(mut self, delay: Duration) -> Self {
        self.header_delay = delay;
        self
    }
}

#[derive(Debug)]
pub struct CapturedRequest {
    pub method: String,
    pub path: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl CapturedRequest {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(header, _)| header.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }
}

pub async fn start_mock_stream_server(status_line: &str, chunks: Vec<MockChunk>) -> u16 {
    start_scripted_stream_server(vec![MockResponsePlan::new(status_line, chunks)])
        .await
        .0
}

/// Consumes one response plan per connection and exposes each parsed request through a bounded
/// channel. Header, delta, and terminal-frame delays are independent. Tests assert protocol
/// results rather than elapsed time, so scheduler variance does not decide pass/fail.
pub async fn start_scripted_stream_server(
    plans: Vec<MockResponsePlan>,
) -> (u16, mpsc::Receiver<CapturedRequest>) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind mock server");
    let port = listener.local_addr().expect("local addr").port();
    let (request_tx, request_rx) = mpsc::channel(plans.len().max(1));

    tokio::spawn(async move {
        for plan in plans {
            let Ok((mut socket, _)) = listener.accept().await else {
                return;
            };
            if let Some(request) = read_request(&mut socket).await {
                let _ = request_tx.send(request).await;
            }
            if !plan.header_delay.is_zero() {
                tokio::time::sleep(plan.header_delay).await;
            }
            let headers = format!(
                "{}\r\nContent-Type: application/octet-stream\r\nConnection: close\r\n\r\n",
                plan.status_line
            );
            if socket.write_all(headers.as_bytes()).await.is_err() {
                return;
            }
            for chunk in plan.chunks {
                if !chunk.delay.is_zero() {
                    tokio::time::sleep(chunk.delay).await;
                }
                if socket.write_all(&chunk.bytes).await.is_err() {
                    break;
                }
                let _ = socket.flush().await;
            }
        }
    });

    (port, request_rx)
}

async fn read_request(socket: &mut tokio::net::TcpStream) -> Option<CapturedRequest> {
    const MAX_REQUEST_BYTES: usize = 1024 * 1024;
    let mut bytes = Vec::new();
    let mut buffer = [0u8; 4096];
    let header_end = loop {
        let read = socket.read(&mut buffer).await.ok()?;
        if read == 0 {
            return None;
        }
        bytes.extend_from_slice(&buffer[..read]);
        if bytes.len() > MAX_REQUEST_BYTES {
            return None;
        }
        if let Some(position) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
            break position + 4;
        }
    };

    let header_text = std::str::from_utf8(&bytes[..header_end]).ok()?;
    let mut lines = header_text.split("\r\n");
    let mut request_line = lines.next()?.split_whitespace();
    let method = request_line.next()?.to_string();
    let path = request_line.next()?.to_string();
    let headers = lines
        .filter_map(|line| {
            let (name, value) = line.split_once(':')?;
            Some((name.trim().to_string(), value.trim().to_string()))
        })
        .collect::<Vec<_>>();
    let content_length = headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("content-length"))
        .and_then(|(_, value)| value.parse::<usize>().ok())
        .unwrap_or(0);
    if header_end.saturating_add(content_length) > MAX_REQUEST_BYTES {
        return None;
    }
    while bytes.len() < header_end + content_length {
        let read = socket.read(&mut buffer).await.ok()?;
        if read == 0 {
            return None;
        }
        bytes.extend_from_slice(&buffer[..read]);
    }
    Some(CapturedRequest {
        method,
        path,
        headers,
        body: bytes[header_end..header_end + content_length].to_vec(),
    })
}
