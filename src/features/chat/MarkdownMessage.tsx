import { Check, Copy } from "lucide-react";
import * as React from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { checkExternalLink } from "../../lib/externalLinks";
import { highlightCode } from "../../lib/highlightCode";
import { useArkClient } from "../../lib/useArkClient";
import { Button } from "../../ui/button";

export function MarkdownMessage({ content }: { content: string }) {
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

          return <CodeBlock code={code} language={match[1]} />;
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
}

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

function CodeBlock({ code, language }: { code: string; language: string }) {
  const [copied, setCopied] = React.useState(false);
  const html = React.useMemo(() => highlightCode(code, language), [code, language]);

  async function copy() {
    await navigator.clipboard.writeText(code);
    setCopied(true);
    window.setTimeout(() => setCopied(false), 1200);
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
          {copied ? <Check className="h-3.5 w-3.5" /> : <Copy className="h-3.5 w-3.5" />}
          {copied ? "Copied" : "Copy"}
        </Button>
      </div>
      <pre className="m-0 overflow-auto border-0 bg-transparent p-4">
        <code dangerouslySetInnerHTML={{ __html: html }} />
      </pre>
    </div>
  );
}
