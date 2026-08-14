/**
 * SEC-008: a pure, dependency-free scheme allowlist for links rendered from untrusted content
 * (model output, imported Markdown). Deliberately conservative — only the schemes a human could
 * meaningfully and safely act on from a chat message. Notably absent: `javascript:`, `data:`,
 * `file:`, `vbscript:`, and any custom/app-launching scheme.
 *
 * This mirrors (and is independently enforced ahead of) the Tauri opener plugin's own
 * `opener:allow-default-urls` capability scope (`src-tauri/capabilities/default.json`) — two
 * independent checks, in two different layers (this one in the UI before ever calling the
 * plugin; the plugin's own native-side scope as a backstop), rather than trusting either alone.
 */
const ALLOWED_EXTERNAL_LINK_SCHEMES = new Set(["http:", "https:", "mailto:", "tel:"]);

export interface ExternalLinkCheck {
  safe: boolean;
  /** The URL as `URL` parsed and re-serialized it — only meaningful when `safe` is true. */
  url: string;
  /** Present only when `safe` is false; a short, non-technical reason suitable for a title/aria-label. */
  reason?: string;
}

/**
 * Validates a link `href` extracted from rendered Markdown/model content before it is ever
 * shown as clickable or passed to `ArkClient.openExternalUrl`. Rejects anything that isn't an
 * absolute URL with an allowed scheme — including relative paths, which have no meaning for a
 * "open externally" action and would otherwise resolve against the app's own origin.
 */
export function checkExternalLink(href: string): ExternalLinkCheck {
  let parsed: URL;
  try {
    parsed = new URL(href);
  } catch {
    return { safe: false, url: href, reason: "not a supported link" };
  }
  if (!ALLOWED_EXTERNAL_LINK_SCHEMES.has(parsed.protocol)) {
    return { safe: false, url: href, reason: "not a supported link" };
  }
  return { safe: true, url: parsed.toString() };
}
