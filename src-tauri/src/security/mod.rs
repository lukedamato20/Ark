//! Native trust-boundary validation for provider destinations (SEC-001).
//!
//! Guiding principle 2.3: "Validate and classify provider destinations in Rust, where
//! requests are sent." All request-sending code paths (Ollama/OpenAI-compatible HTTP
//! clients) and all persistence paths (saving a provider's base URL) must go through
//! this module rather than trusting a client-supplied or previously-stored `is_local`
//! flag.

use crate::errors::AppError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DestinationClass {
    /// 127.0.0.0/8, ::1, or the literal hostname "localhost". Immune to DNS rebinding
    /// because no name resolution is required to reach it.
    Loopback,
    /// RFC 1918 / RFC 4193 private ranges expressed as an IP literal. Immune to DNS
    /// rebinding for the same reason as Loopback.
    PrivateLan,
    /// Everything else, including any bare hostname. A hostname is classified Public
    /// even if it currently resolves to a private address: Ark cannot guarantee it
    /// will still resolve there at request time (DNS rebinding), so hostnames other
    /// than "localhost" never receive local trust from static classification alone.
    Public,
}

impl DestinationClass {
    pub fn is_trusted_local(self) -> bool {
        matches!(
            self,
            DestinationClass::Loopback | DestinationClass::PrivateLan
        )
    }

    pub fn as_str(self) -> &'static str {
        match self {
            DestinationClass::Loopback => "loopback",
            DestinationClass::PrivateLan => "private_lan",
            DestinationClass::Public => "public",
        }
    }
}

/// Parses and classifies a provider base URL. Hard-rejects (no override possible via
/// [`enforce_destination_policy`]):
/// - non-http(s) schemes (e.g. `file://`, `javascript:`)
/// - embedded userinfo credentials (`http://user:pass@host`)
/// - missing host
pub fn classify_destination(raw_url: &str) -> Result<DestinationClass, AppError> {
    let trimmed = raw_url.trim();
    if trimmed.is_empty() {
        return Err(AppError::invalid_input(
            "Provider base URL cannot be empty.",
        ));
    }

    let parsed = reqwest::Url::parse(trimmed).map_err(|_| {
        AppError::invalid_input("Provider base URL must be a valid http:// or https:// URL.")
    })?;
    if parsed.scheme() != "http" && parsed.scheme() != "https" {
        return Err(AppError::invalid_input(format!(
            "Unsupported URL scheme '{}'. Only http and https are allowed.",
            parsed.scheme()
        )));
    }
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err(AppError::invalid_input(
            "Provider base URL must not contain embedded credentials.",
        ));
    }
    let host = parsed
        .host_str()
        .ok_or_else(|| AppError::invalid_input("Provider base URL is missing a host."))?;
    let host_lower = host
        .trim_start_matches('[')
        .trim_end_matches(']')
        .to_ascii_lowercase();

    if let Ok(ipv4) = host_lower.parse::<std::net::Ipv4Addr>() {
        return Ok(classify_ipv4(ipv4));
    }
    if let Ok(ipv6) = host_lower.parse::<std::net::Ipv6Addr>() {
        return Ok(classify_ipv6(ipv6));
    }
    if host_lower == "localhost" || host_lower.ends_with(".localhost") {
        return Ok(DestinationClass::Loopback);
    }

    Ok(DestinationClass::Public)
}

/// SEC-001 save-time policy: a Public destination — remote host, or any bare hostname
/// that cannot be statically trusted — requires the caller to explicitly acknowledge
/// the privacy risk before Ark persists it. Loopback/PrivateLan destinations always
/// pass without confirmation.
pub fn enforce_destination_policy(
    raw_url: &str,
    convert_to_remote_provider: bool,
    acknowledge_remote_risk: bool,
    allow_insecure_remote: bool,
) -> Result<DestinationClass, AppError> {
    let class = classify_destination(raw_url)?;

    if class == DestinationClass::Public && !convert_to_remote_provider {
        return Err(AppError::new(
            "destination_requires_remote_provider_class",
            "This provider is local-only, but the address is public. Explicitly convert it to the Remote provider class before Ark can use this destination.",
        ));
    }

    if class == DestinationClass::Public && !acknowledge_remote_risk {
        let is_https = raw_url.trim().to_ascii_lowercase().starts_with("https://");
        let message = if is_https {
            "This address is outside your local network. Ark blocks remote endpoints by default to keep \
             conversations private. Confirm you want to send your prompts and conversation history to this destination."
        } else {
            "This address is outside your local network and does not use HTTPS. Ark blocks insecure remote \
             endpoints by default. Confirm you want to send your prompts and conversation history, unencrypted, \
             to this destination."
        };
        return Err(AppError::new("destination_requires_confirmation", message));
    }

    let parsed = reqwest::Url::parse(raw_url.trim()).map_err(|error| {
        AppError::invalid_input(format!("Provider base URL is invalid: {error}"))
    })?;
    if class != DestinationClass::Loopback && parsed.scheme() != "https" && !allow_insecure_remote {
        return Err(AppError::new(
            "insecure_remote_requires_development_mode",
            "This non-loopback destination uses unencrypted HTTP. Enable the explicitly warned insecure-remote development mode, or use HTTPS.",
        ));
    }

    Ok(class)
}

/// Revalidates persisted provider policy when an HTTP adapter is constructed. Save-time checks
/// are not sufficient because workspace rows may have been imported or externally modified.
pub fn enforce_persisted_destination_policy(
    raw_url: &str,
    is_local_provider: bool,
    allow_insecure_remote: bool,
) -> Result<DestinationClass, AppError> {
    let class = classify_destination(raw_url)?;
    if class == DestinationClass::Public && is_local_provider {
        return Err(AppError::new(
            "destination_policy_violation",
            "A local-only provider cannot send requests to a public destination. Re-save it as a Remote provider in Settings.",
        ));
    }
    let parsed = reqwest::Url::parse(raw_url.trim()).map_err(|error| {
        AppError::invalid_input(format!("Provider base URL is invalid: {error}"))
    })?;
    if class != DestinationClass::Loopback && parsed.scheme() != "https" && !allow_insecure_remote {
        return Err(AppError::new(
            "destination_policy_violation",
            "A non-loopback HTTP provider is blocked unless its insecure-remote development-mode exception is explicitly enabled.",
        ));
    }
    Ok(class)
}

fn classify_ipv4(ip: std::net::Ipv4Addr) -> DestinationClass {
    let [a, b, _, _] = ip.octets();
    if a == 127 {
        DestinationClass::Loopback
    } else if a == 10
        || (a == 172 && (16..=31).contains(&b))
        || (a == 192 && b == 168)
        || (a == 169 && b == 254)
    {
        DestinationClass::PrivateLan
    } else {
        DestinationClass::Public
    }
}

fn classify_ipv6(ip: std::net::Ipv6Addr) -> DestinationClass {
    if ip.is_loopback() {
        DestinationClass::Loopback
    } else if (ip.segments()[0] & 0xfe00) == 0xfc00 {
        // Unique local addresses, fc00::/7 — the IPv6 analog of RFC 1918.
        DestinationClass::PrivateLan
    } else {
        DestinationClass::Public
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_loopback_hosts() {
        assert_eq!(
            classify_destination("http://localhost:11434").unwrap(),
            DestinationClass::Loopback
        );
        assert_eq!(
            classify_destination("http://127.0.0.1:8080").unwrap(),
            DestinationClass::Loopback
        );
        assert_eq!(
            classify_destination("http://127.5.5.5/api").unwrap(),
            DestinationClass::Loopback
        );
        assert_eq!(
            classify_destination("http://[::1]:11434").unwrap(),
            DestinationClass::Loopback
        );
    }

    #[test]
    fn classifies_private_lan_ip_literals() {
        assert_eq!(
            classify_destination("http://10.0.0.5:8080").unwrap(),
            DestinationClass::PrivateLan
        );
        assert_eq!(
            classify_destination("http://192.168.1.50:8080").unwrap(),
            DestinationClass::PrivateLan
        );
        assert_eq!(
            classify_destination("http://172.16.0.1:8080").unwrap(),
            DestinationClass::PrivateLan
        );
        assert_eq!(
            classify_destination("http://172.31.255.255:8080").unwrap(),
            DestinationClass::PrivateLan
        );
        assert_eq!(
            classify_destination("http://169.254.1.1:8080").unwrap(),
            DestinationClass::PrivateLan
        );
    }

    #[test]
    fn classifies_public_addresses_and_arbitrary_hostnames_as_public() {
        assert_eq!(
            classify_destination("https://api.openai.com/v1").unwrap(),
            DestinationClass::Public
        );
        assert_eq!(
            classify_destination("https://8.8.8.8/").unwrap(),
            DestinationClass::Public
        );
        assert_eq!(
            classify_destination("http://172.32.0.1:8080").unwrap(),
            DestinationClass::Public
        );
        // DNS-rebinding mitigation: a bare hostname is never trusted as local, even one
        // that looks like it might be an internal name, because it could later resolve
        // to any address at request time.
        assert_eq!(
            classify_destination("http://my-internal-server:8080").unwrap(),
            DestinationClass::Public
        );
        assert_eq!(
            classify_destination("http://ollama.local:11434").unwrap(),
            DestinationClass::Public
        );
    }

    #[test]
    fn rejects_non_http_schemes() {
        let error = classify_destination("file:///etc/passwd").unwrap_err();
        assert_eq!(error.code, "invalid_input");

        let error = classify_destination("javascript:alert(1)").unwrap_err();
        assert_eq!(error.code, "invalid_input");

        let error = classify_destination("ftp://example.com").unwrap_err();
        assert_eq!(error.code, "invalid_input");
    }

    #[test]
    fn rejects_embedded_credentials() {
        let error = classify_destination("http://user:secret@10.0.0.5:8080").unwrap_err();
        assert_eq!(error.code, "invalid_input");
    }

    #[test]
    fn rejects_empty_and_malformed_urls() {
        assert!(classify_destination("").is_err());
        assert!(classify_destination("   ").is_err());
        assert!(classify_destination("not-a-url").is_err());
        assert!(classify_destination("http://").is_err());
    }

    #[test]
    fn enforce_policy_allows_local_without_acknowledgment() {
        let class = enforce_destination_policy("http://localhost:11434", false, false, false)
            .expect("loopback always allowed");
        assert_eq!(class, DestinationClass::Loopback);

        let class = enforce_destination_policy("https://192.168.1.10:8080", false, false, false)
            .expect("private LAN over HTTPS is allowed");
        assert_eq!(class, DestinationClass::PrivateLan);
    }

    #[test]
    fn enforce_policy_requires_explicit_remote_class_then_acknowledgment() {
        let error =
            enforce_destination_policy("https://api.example.com", false, false, false).unwrap_err();
        assert_eq!(error.code, "destination_requires_remote_provider_class");

        let error =
            enforce_destination_policy("https://api.example.com", true, false, false).unwrap_err();
        assert_eq!(error.code, "destination_requires_confirmation");
    }

    #[test]
    fn enforce_policy_requires_explicit_development_mode_for_non_loopback_http() {
        let error = enforce_destination_policy("http://192.168.1.10:8080", false, false, false)
            .unwrap_err();
        assert_eq!(error.code, "insecure_remote_requires_development_mode");

        let error =
            enforce_destination_policy("http://api.example.com", true, true, false).unwrap_err();
        assert_eq!(error.code, "insecure_remote_requires_development_mode");

        let class = enforce_destination_policy("http://api.example.com", true, true, true)
            .expect("explicit development-mode exception");
        assert_eq!(class, DestinationClass::Public);
    }

    #[test]
    fn enforce_policy_allows_public_destination_with_explicit_acknowledgment() {
        let class = enforce_destination_policy("https://api.example.com", true, true, false)
            .expect("explicit override allowed");
        assert_eq!(class, DestinationClass::Public);
    }

    #[test]
    fn enforce_policy_still_hard_rejects_invalid_scheme_even_with_acknowledgment() {
        let error = enforce_destination_policy("file:///etc/passwd", true, true, true).unwrap_err();
        assert_eq!(error.code, "invalid_input");
    }

    #[test]
    fn persisted_policy_is_revalidated_before_any_adapter_request() {
        let error = enforce_persisted_destination_policy("https://api.example.com", true, false)
            .unwrap_err();
        assert_eq!(error.code, "destination_policy_violation");

        let error = enforce_persisted_destination_policy("http://192.168.1.10:8080", false, false)
            .unwrap_err();
        assert_eq!(error.code, "destination_policy_violation");
    }
}
