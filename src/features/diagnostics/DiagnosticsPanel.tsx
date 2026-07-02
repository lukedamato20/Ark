import { Activity, Cpu, HardDrive, Loader2, MemoryStick, Zap } from "lucide-react";
import * as React from "react";
import { getErrorMessage, runDiagnostics } from "../../lib/api";
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
  const [result, setResult] = React.useState<DiagnosticsResult | null>(null);
  const [running, setRunning] = React.useState(false);

  async function run() {
    if (!provider) {
      return;
    }

    setRunning(true);
    try {
      setResult(await runDiagnostics(provider.id, selectedModel));
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

          <div className="grid gap-3 md:grid-cols-2">
            <Metric icon={<Cpu className="h-4 w-4" />} label="CPU" value={`${result.cpu} (${result.cpuCores} cores)`} />
            <Metric icon={<MemoryStick className="h-4 w-4" />} label="Memory" value={`${formatBytes(result.availableMemoryBytes)} available / ${formatBytes(result.totalMemoryBytes)}`} />
            <Metric icon={<HardDrive className="h-4 w-4" />} label="Disk" value={`${formatBytes(result.availableDiskBytes)} available / ${formatBytes(result.totalDiskBytes)}`} />
            <Metric icon={<Zap className="h-4 w-4" />} label="Accelerator" value={result.gpu} />
          </div>

          {result.benchmark && (
            <div className="grid gap-3 md:grid-cols-3">
              <Metric
                label="First token"
                value={
                  result.benchmark.timeToFirstTokenMs == null
                    ? "Unknown"
                    : `${result.benchmark.timeToFirstTokenMs} ms`
                }
              />
              <Metric label="Total time" value={`${result.benchmark.totalTimeMs} ms`} />
              <Metric
                label="Approx speed"
                value={
                  result.benchmark.approximateTokensPerSecond == null
                    ? "Unknown"
                    : `${result.benchmark.approximateTokensPerSecond.toFixed(1)} tok/s`
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
