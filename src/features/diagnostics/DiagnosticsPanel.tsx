import { Activity, Cpu, HardDrive, Loader2, MemoryStick, Zap } from "lucide-react";
import * as React from "react";
import { getErrorMessage } from "../../lib/arkErrors";
import { useArkClient } from "../../lib/useArkClient";
import { formatBytes } from "../../lib/format";
import type { DiagnosticsResult, ProviderConfig } from "../../types/ark";
import { Badge } from "../../ui/badge";
import { Button } from "../../ui/button";
import { Panel } from "../../ui/panel";

interface DiagnosticsPanelProps {
  provider?: ProviderConfig;
  selectedModel?: string | null;
  onError: (message: string) => void;
}

export function DiagnosticsPanel({ provider, selectedModel, onError }: DiagnosticsPanelProps) {
  const client = useArkClient();
  const [result, setResult] = React.useState<DiagnosticsResult | null>(null);
  const [running, setRunning] = React.useState(false);
  const [includeRuntimeLogs, setIncludeRuntimeLogs] = React.useState(false);

  async function run() {
    if (!provider) {
      return;
    }

    setRunning(true);
    try {
      setResult(await client.runDiagnostics(provider.id, selectedModel, includeRuntimeLogs));
    } catch (error) {
      onError(getErrorMessage(error));
    } finally {
      setRunning(false);
    }
  }

  return (
    <Panel className="p-4">
      <div className="flex items-center justify-between gap-3">
        <div>
          <h3 className="text-sm font-semibold">Diagnostics</h3>
          <p className="mt-1 text-sm text-muted-foreground">
            Check local runtime readiness and benchmark the selected model.
          </p>
        </div>
        <Button onClick={run} disabled={!provider || running}>
          {running ? <Loader2 className="h-4 w-4 animate-spin" /> : <Activity className="h-4 w-4" />}
          Run test
        </Button>
      </div>

      <label className="mt-3 flex items-start gap-2 text-xs text-muted-foreground">
        <input
          type="checkbox"
          checked={includeRuntimeLogs}
          onChange={(event) => setIncludeRuntimeLogs(event.target.checked)}
          className="mt-0.5 h-4 w-4 accent-primary"
        />
        <span>
          Include up to 50 recent managed-runtime log lines. Ark redacts known paths and secrets; leave this off unless
          you consent to include runtime output in this diagnostic result.
        </span>
      </label>

      {result && (
        <div className="mt-4 grid gap-3">
          <div className="rounded-md border border-border bg-background p-3">
            <div className="flex items-center justify-between">
              <div className="font-medium">{result.guidance}</div>
              <Badge tone={result.providerHealth.isReachable && result.modelAvailable ? "success" : "warning"}>
                {result.providerHealth.status}
              </Badge>
            </div>
            <p className="mt-1 text-sm text-muted-foreground">{result.providerHealth.message}</p>
          </div>

          <div className="rounded-md border border-border bg-background p-3">
            <div className="flex items-center justify-between gap-2">
              <span className="text-xs font-medium uppercase tracking-wide text-muted-foreground">Managed runtime</span>
              <Badge
                tone={
                  result.runtime.state === "healthy"
                    ? "success"
                    : result.runtime.state === "stopped"
                      ? "muted"
                      : "warning"
                }
              >
                {result.runtime.state.replaceAll("_", " ")}
              </Badge>
            </div>
            {result.runtime.failure && (
              <p className="mt-2 text-sm text-muted-foreground">
                {result.runtime.failure.category.replaceAll("_", " ")}: {result.runtime.failure.message}
              </p>
            )}
            {result.runtime.recentLogs.length > 0 && (
              <pre className="mt-3 max-h-48 overflow-auto whitespace-pre-wrap rounded bg-muted p-2 text-xs">
                {result.runtime.recentLogs.map((entry) => `[${entry.stream}] ${entry.message}`).join("\n")}
              </pre>
            )}
          </div>

          <div className="grid gap-3 md:grid-cols-2">
            <Metric icon={<Cpu className="h-4 w-4" />} label="CPU" value={`${result.cpu} (${result.cpuCores} cores)`} />
            <Metric
              icon={<MemoryStick className="h-4 w-4" />}
              label="Memory"
              value={`${formatBytes(result.availableMemoryBytes)} available / ${formatBytes(result.totalMemoryBytes)}`}
            />
            <Metric
              icon={<HardDrive className="h-4 w-4" />}
              label="Disk (workspace volume)"
              value={`${formatBytes(result.availableDiskBytes)} available / ${formatBytes(result.totalDiskBytes)}`}
            />
            <Metric icon={<Zap className="h-4 w-4" />} label="Accelerator" value={result.gpu} />
          </div>

          {result.benchmarkFailure && (
            <div role="alert" className="rounded-md border border-destructive/30 bg-destructive/10 p-3 text-sm">
              <div className="font-medium text-destructive">Benchmark failed</div>
              <p className="mt-1 text-muted-foreground">
                {result.benchmarkFailure.code}: {result.benchmarkFailure.message}
              </p>
            </div>
          )}

          {result.benchmark && (
            <div className="grid gap-3 md:grid-cols-4">
              <Metric
                label="First token"
                value={
                  result.benchmark.timeToFirstTokenMs == null ? "Unknown" : `${result.benchmark.timeToFirstTokenMs} ms`
                }
              />
              <Metric
                label="Generation time"
                value={
                  result.benchmark.generationTimeMs == null ? "Unknown" : `${result.benchmark.generationTimeMs} ms`
                }
              />
              <Metric label="Total time" value={`${result.benchmark.totalTimeMs} ms`} />
              <Metric
                label="Approx speed"
                value={
                  result.benchmark.approximateTokensPerSecond == null
                    ? "Unknown"
                    : `${result.benchmark.approximateTokensPerSecond.toFixed(1)} tok/s (generation-only)`
                }
              />
            </div>
          )}
        </div>
      )}
    </Panel>
  );
}

function Metric({ icon, label, value }: { icon?: React.ReactNode; label: string; value: string }) {
  return (
    <div className="rounded-md border border-border bg-background p-3">
      <div className="mb-1 flex items-center gap-2 text-xs font-medium uppercase tracking-wide text-muted-foreground">
        {icon}
        {label}
      </div>
      <div className="break-words text-sm">{value}</div>
    </div>
  );
}
