import { AlertCircle, CheckCircle2, Info } from "lucide-react";
import type { ModelInfo, ProviderConfig, ProviderHealth } from "../../types/ark";
import { Badge } from "../../ui/badge";
import { Button } from "../../ui/button";

interface SetupBannerProps {
  health?: ProviderHealth | null;
  provider?: ProviderConfig | null;
  models: ModelInfo[];
  selectedModel?: string | null;
  onRefresh: () => void;
}

export function SetupBanner({ health, provider, models, selectedModel, onRefresh }: SetupBannerProps) {
  const hasModel = Boolean(selectedModel && models.some((m) => m.name === selectedModel && m.isAvailable));
  const reachable = health?.isReachable ?? false;

  if (reachable && hasModel) {
    return (
      <div className="flex items-center gap-2 rounded-md border border-emerald-500/20 bg-emerald-500/10 px-3 py-2 text-sm">
        <CheckCircle2 className="h-4 w-4 text-emerald-500" />
        <span>Local runtime ready.</span>
        <Badge tone="success">offline-capable</Badge>
      </div>
    );
  }

  if (!reachable && provider?.providerType === "local_inference_host") {
    return <LocalInferenceHostBanner provider={provider} onRefresh={onRefresh} />;
  }

  return (
    <div className="flex flex-wrap items-center justify-between gap-3 rounded-md border border-amber-500/25 bg-amber-500/10 px-3 py-2 text-sm">
      <div className="flex min-w-0 items-start gap-2">
        <AlertCircle className="mt-0.5 h-4 w-4 shrink-0 text-amber-500" />
        <div>
          <div className="font-medium">Local setup needs attention</div>
          <div className="text-muted-foreground">
            {!reachable
              ? provider?.providerType === "ollama"
                ? "Start Ollama, then refresh models. Ark stays usable while local inference is unavailable."
                : `Start your inference runtime (${provider?.name ?? "unknown"}), then refresh models.`
              : "Install or select a local model before chatting."}
          </div>
        </div>
      </div>
      <Button size="sm" onClick={onRefresh}>
        Refresh
      </Button>
    </div>
  );
}

function LocalInferenceHostBanner({ provider, onRefresh }: { provider: ProviderConfig; onRefresh: () => void }) {
  const url = provider.baseUrl ?? "http://localhost:8080";

  return (
    <div className="rounded-md border border-amber-500/25 bg-amber-500/10 text-sm">
      <div className="flex flex-wrap items-start justify-between gap-3 px-3 py-2.5">
        <div className="flex min-w-0 items-start gap-2">
          <AlertCircle className="mt-0.5 h-4 w-4 shrink-0 text-amber-500" />
          <div>
            <div className="font-medium">Local inference server not running</div>
            <div className="text-muted-foreground">
              Ark connects to a server you run — it doesn't start one automatically. Launch your server at{" "}
              <code className="rounded bg-black/20 px-1 font-mono text-xs">{url}</code>, then click Refresh.
            </div>
          </div>
        </div>
        <Button size="sm" onClick={onRefresh} className="shrink-0">
          Refresh
        </Button>
      </div>

      <div className="border-t border-amber-500/20 px-3 py-2.5">
        <div className="mb-1.5 flex items-center gap-1.5 text-xs font-medium text-muted-foreground">
          <Info className="h-3.5 w-3.5" />
          Compatible software — start one of these, then refresh:
        </div>
        <div className="grid gap-1">
          {COMPATIBLE_SERVERS.map((s) => (
            <div key={s.name} className="flex flex-wrap items-baseline gap-x-2 gap-y-0.5">
              <span className="text-xs font-medium">{s.name}</span>
              <code className="rounded bg-black/20 px-1.5 py-0.5 font-mono text-[11px] text-muted-foreground">
                {s.example}
              </code>
            </div>
          ))}
        </div>
        <p className="mt-2 text-xs text-muted-foreground">
          Need a different port? Update the Base URL in{" "}
          <span className="font-medium">Settings → Provider → Local inference host</span>.
        </p>
      </div>
    </div>
  );
}

const COMPATIBLE_SERVERS = [
  {
    name: "llama.cpp",
    example: "llama-server --model model.gguf --port 8080",
  },
  {
    name: "LM Studio",
    example: "Enable local server in LM Studio → Developer tab",
  },
  {
    name: "Jan",
    example: "Start the local API server from Jan settings",
  },
  {
    name: "Ollama (OpenAI compat)",
    example: "Use http://localhost:11434 and switch to the Ollama provider",
  },
];
