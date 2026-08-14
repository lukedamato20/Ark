//! OPS-001: a shared redaction pass used by every local log sink — the sidecar's runtime log
//! buffer (`sidecar.rs`, the original home of this logic) and the structured diagnostics log
//! (`observability.rs`). Extracted rather than duplicated so a marker/pattern added for one
//! consumer automatically protects the other; both need the identical guarantee ("no credential,
//! token, header value, query-string value, or absolute path ever reaches a stored log line")
//! and drift between two copies of this logic would be a real security regression, not a style
//! issue.
//!
//! What this module deliberately does *not* try to do: detect and redact arbitrary user prompt
//! or model-output content, or attachment text. That content has no reliable syntactic marker to
//! pattern-match on — the actual control for it is architectural, not textual: call sites in
//! `observability.rs` never pass prompt/response/attachment text into a log message in the first
//! place (see that module's own doc comment and its `log_event` call sites). This module's job is
//! the content that *does* have a reliable shape: credentials, bearer/API tokens, session/sync
//! tokens, cookie headers, query-string values, and absolute filesystem paths.

/// Case-insensitive markers after which the following token (up to the next whitespace/quote/
/// separator) is replaced with `[REDACTED]`. Covers credentials and bearer-style auth headers,
/// API keys, generic tokens, session/sync tokens (OPS-001's "sync tokens" acceptance criterion),
/// and cookie headers (OPS-001's "headers" acceptance criterion — `Authorization` is already
/// covered by the bearer markers below).
const VALUE_AFTER_MARKERS: &[&str] = &[
    "authorization: bearer ",
    "bearer ",
    "--api-key ",
    "api-key=",
    "api_key=",
    "apikey=",
    "token=",
    "access_token=",
    "refresh_token=",
    "sync_token=",
    "sync-token=",
    "session_token=",
    "cookie: ",
    "set-cookie: ",
    "password=",
    "secret=",
];

/// Redacts `message` for safe storage in any local log: replaces every occurrence of each known
/// sensitive value verbatim, then applies marker-based redaction for common credential/token/
/// cookie shapes, then strips query strings and absolute filesystem paths.
pub fn redact(message: &str, sensitive_values: &[String]) -> String {
    let mut redacted = message.to_string();
    for value in sensitive_values {
        if !value.is_empty() {
            redacted = redacted.replace(value, "[REDACTED]");
        }
    }
    for marker in VALUE_AFTER_MARKERS {
        redacted = redact_value_after_marker(redacted, marker);
    }
    redacted = redact_query_strings(&redacted);
    redact_absolute_path_tokens(&redacted)
}

/// A URL's query string can carry the same secrets a header would (API keys, session/sync
/// tokens) via `?key=...&token=...`. Rather than enumerate every possible parameter name, the
/// whole query string is replaced once a `?` is found within what looks like a URL/path token —
/// conservative, but a stray `?` redacting slightly more context is a far safer failure mode
/// than a missed secret.
fn redact_query_strings(text: &str) -> String {
    text.split_inclusive(char::is_whitespace)
        .map(|segment| {
            let token = segment.trim_end_matches(char::is_whitespace);
            let whitespace = &segment[token.len()..];
            match token.split_once('?') {
                Some((before, query)) if !query.is_empty() => {
                    format!("{before}?[REDACTED_QUERY]{whitespace}")
                }
                _ => segment.to_string(),
            }
        })
        .collect()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_known_paths_secrets_and_common_auth_shapes() {
        let sensitive = vec![
            "C:\\Users\\person\\Models\\private.gguf".to_string(),
            "launch-secret".to_string(),
        ];
        let source = "model=C:\\Users\\person\\Models\\private.gguf Authorization: Bearer launch-secret token=other /Users/person/cache";
        let redacted = redact(source, &sensitive);
        assert!(!redacted.contains("person"));
        assert!(!redacted.contains("launch-secret"));
        assert!(!redacted.contains("other"));
        assert!(!redacted.contains("/Users/person"));
        assert!(redacted.matches("[REDACTED]").count() >= 3);
    }

    #[test]
    fn redacts_credentials_in_various_marker_shapes() {
        for source in [
            "api_key=sk-abc123 done",
            "apikey=sk-abc123 done",
            "--api-key sk-abc123 done",
            "password=sk-abc123 done",
            "secret=sk-abc123 done",
        ] {
            let redacted = redact(source, &[]);
            assert!(
                !redacted.contains("sk-abc123"),
                "expected credential redacted from: {source} -> {redacted}"
            );
        }
    }

    #[test]
    fn redacts_sync_and_session_tokens() {
        for source in [
            "sync_token=abcdef123 next",
            "sync-token=abcdef123 next",
            "refresh_token=abcdef123 next",
            "session_token=abcdef123 next",
            "access_token=abcdef123 next",
        ] {
            let redacted = redact(source, &[]);
            assert!(
                !redacted.contains("abcdef123"),
                "expected sync/session token redacted from: {source} -> {redacted}"
            );
        }
    }

    #[test]
    fn redacts_cookie_headers() {
        let source = "Cookie: session=abc123; other=xyz\nSet-Cookie: session=abc123; Path=/";
        let redacted = redact(source, &[]);
        assert!(!redacted.contains("abc123"));
    }

    #[test]
    fn redacts_query_strings_in_urls() {
        let source = "GET http://127.0.0.1:8080/models?api_key=sk-live-secret&user=me HTTP/1.1";
        let redacted = redact(source, &[]);
        assert!(!redacted.contains("sk-live-secret"));
        assert!(!redacted.contains("user=me"));
        assert!(redacted.contains("/models?[REDACTED_QUERY]"));
    }

    #[test]
    fn redacts_absolute_unix_and_windows_paths() {
        let source = "reading /home/person/secret.txt and D:\\Users\\person\\notes.txt";
        let redacted = redact(source, &[]);
        assert!(!redacted.contains("person"));
        assert!(redacted.matches("[REDACTED_PATH]").count() >= 2);
    }

    #[test]
    fn leaves_ordinary_non_sensitive_text_unchanged() {
        let source = "runtime became healthy after 3 attempts";
        assert_eq!(redact(source, &[]), source);
    }
}
