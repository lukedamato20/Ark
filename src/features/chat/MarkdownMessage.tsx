import hljs from "highlight.js/lib/core";
import bash from "highlight.js/lib/languages/bash";
import c from "highlight.js/lib/languages/c";
import cpp from "highlight.js/lib/languages/cpp";
import css from "highlight.js/lib/languages/css";
import go from "highlight.js/lib/languages/go";
import javascript from "highlight.js/lib/languages/javascript";
import json from "highlight.js/lib/languages/json";
import python from "highlight.js/lib/languages/python";
import rust from "highlight.js/lib/languages/rust";
import sql from "highlight.js/lib/languages/sql";
import typescript from "highlight.js/lib/languages/typescript";
import xml from "highlight.js/lib/languages/xml";
import yaml from "highlight.js/lib/languages/yaml";
import { Check, Copy } from "lucide-react";
import * as React from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { Button } from "../../ui/button";

hljs.registerLanguage("bash", bash);
hljs.registerLanguage("sh", bash);
hljs.registerLanguage("shell", bash);
hljs.registerLanguage("c", c);
hljs.registerLanguage("cpp", cpp);
hljs.registerLanguage("css", css);
hljs.registerLanguage("go", go);
hljs.registerLanguage("js", javascript);
hljs.registerLanguage("javascript", javascript);
hljs.registerLanguage("json", json);
hljs.registerLanguage("py", python);
hljs.registerLanguage("python", python);
hljs.registerLanguage("rust", rust);
hljs.registerLanguage("sql", sql);
hljs.registerLanguage("ts", typescript);
hljs.registerLanguage("typescript", typescript);
hljs.registerLanguage("html", xml);
hljs.registerLanguage("xml", xml);
hljs.registerLanguage("yaml", yaml);
hljs.registerLanguage("yml", yaml);

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
      }}
    >
      {content}
    </ReactMarkdown>
  );
}

function CodeBlock({ code, language }: { code: string; language: string }) {
  const [copied, setCopied] = React.useState(false);
  const html = React.useMemo(() => {
    const known = hljs.getLanguage(language);
    if (!known) {
      return escapeHtml(code);
    }
    return hljs.highlight(code, { language }).value;
  }, [code, language]);

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

function escapeHtml(value: string) {
  return value
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&#039;");
}
