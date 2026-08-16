import { Check, Copy } from "lucide-react";
import * as React from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { checkExternalLink } from "../../lib/externalLinks";
import { highlightCode } from "../../lib/highlightCode";
import { useArkClient } from "../../lib/useArkClient";
import { Button } from "../../ui/button";

/** PERF-005: memoized so a parent rerender that doesn't actually change `content`/`isStreaming`
 * (e.g. an unrelated store update reaching the owning `MessageBubble`) never forces `ReactMarkdown`
 * to reparse — the expensive step this component exists to gate. */
export const MarkdownMessage = React.memo(function MarkdownMessage({
  content,
  isStreaming,
}: {
  content: string;
  /** PERF-005: while true, code blocks render as plain preformatted text instead of running
   * `highlightCode` — an actively streaming message's fences are still forming, so the highlighted
   * result would likely be thrown away on the very next delta anyway. Every code block in a
   * streaming message is treated this way (not just the last/open one): distinguishing an
   * already-closed fence from a still-growing one earlier in the same message would need raw-
   * source position tracking `ReactMarkdown`'s parsed AST doesn't expose, so this trades a
   * modest, temporary loss of highlighting on earlier fences for a simple, robust rule. A full
   * highlighted pass always runs once the message reaches a terminal status. */
  isStreaming?: boolean;
}) {
  return (
    <ReactMarkdown
      className="markdown text-sm"
      remarkPlugins={[remarkGfm]}
      components={{
        code({ className, children, ...props }) {
          const match = /language-(\w+)/.exec(className ?? "");
          const code = String(children).replace(/\n$/, "");

          if (!match) {
            return (
              <code className={className} {...props}>
                {children}
              </code>
            );
          }

          return <CodeBlock code={code} language={match[1]} isStreaming={Boolean(isStreaming)} />;
        },
        a({ href, children, ...props }) {
          return (
            <MarkdownLink href={href} {...props}>
              {children}
            </MarkdownLink>
          );
        },
      }}
    >
      {content}
    </ReactMarkdown>
  );
});

/**
 * SEC-008: every Markdown link renders through this component rather than react-markdown's
 * default `<a>` — model output and imported content are untrusted, so a link's destination is
 * independently re-validated here regardless of what react-markdown/remark already permitted.
 * A link with an unsupported scheme (javascript:, data:, file:, a relative path, ...) renders as
 * inert text, never a clickable or navigable element. A supported link never navigates the app's
 * own window: the click is always intercepted and handed to the OS's default browser through
 * `ArkClient.openExternalUrl`, and the real destination is shown via `title` so link text cannot
 * silently point somewhere other than what it displays.
 */
function MarkdownLink({ href, children, ...props }: React.AnchorHTMLAttributes<HTMLAnchorElement>) {
  const client = useArkClient();
  const check = href ? checkExternalLink(href) : { safe: false as const, url: "", reason: "not a supported link" };

  if (!check.safe) {
    return <span title={check.reason}>{children}</span>;
  }

  return (
    <a
      {...props}
      href={check.url}
      title={check.url}
      rel="noopener noreferrer"
      onClick={(event) => {
        event.preventDefault();
        void client.openExternalUrl(check.url);
      }}
    >
      {children}
    </a>
  );
}

function CodeBlock({ code, language, isStreaming }: { code: string; language: string; isStreaming: boolean }) {
  // UX-011: previously `copy()` had no error handling at all — a rejected `writeText` (denied
  // permission, an unfocused document, no Clipboard API) left the button showing "Copy" forever
  // with an uncaught promise rejection and zero indication to the user that anything went wrong.
  // Confirmed live: this genuinely happens (`NotAllowedError: Document is not focused`) rather
  // than being a theoretical case.
  const [copyState, setCopyState] = React.useState<"idle" | "copied" | "failed">("idle");
  // PERF-005: `null` while streaming means "render plain" — skips `highlightCode` entirely
  // rather than computing and discarding a highlighted result that a growing fence would
  // invalidate on the very next delta. Recomputed once, for real, the moment streaming ends.
  const html = React.useMemo(() => (isStreaming ? null : highlightCode(code, language)), [code, language, isStreaming]);

  async function copy() {
    try {
      await navigator.clipboard.writeText(code);
      setCopyState("copied");
    } catch {
      setCopyState("failed");
    }
    window.setTimeout(() => setCopyState("idle"), 1200);
  }

  return (
    <div className="my-3 overflow-hidden rounded-lg border border-border bg-[#0b0f17]">
      <div className="flex h-9 items-center justify-between border-b border-white/10 px-3">
        <span className="text-xs text-slate-400">{language}</span>
        <Button
          size="sm"
          variant="ghost"
          onClick={copy}
          className="h-7 text-slate-300 hover:bg-white/10 hover:text-white"
        >
          {copyState === "copied" ? <Check className="h-3.5 w-3.5" /> : <Copy className="h-3.5 w-3.5" />}
          {copyState === "copied" ? "Copied" : copyState === "failed" ? "Copy failed" : "Copy"}
        </Button>
      </div>
      <pre className="m-0 overflow-auto border-0 bg-transparent p-4">
        {html === null ? <code>{code}</code> : <code dangerouslySetInnerHTML={{ __html: html }} />}
      </pre>
      {/* UX-011: the Copy/Copied swap above is visual only — a screen-reader user watching
       * focus, not the icon, previously got no confirmation the copy happened, succeeded, or
       * failed at all. */}
      <div role="status" aria-live="polite" className="sr-only">
        {copyState === "copied" && "Copied to clipboard."}
        {copyState === "failed" && "Copy failed. Your browser or OS blocked clipboard access."}
      </div>
    </div>
  );
}
