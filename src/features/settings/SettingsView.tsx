import { AlertCircle, ArrowLeft, CheckCircle2, Circle, Database, Info, Moon, RefreshCw, Save, Shield, SlidersHorizontal, Sun } from "lucide-react";
import * as React from "react";
import {
  getErrorMessage,
  refreshModels,
  resetWorkspace,
  setWorkspace,
  startBuiltInRuntime,
  stopBuiltInRuntime,
  updateProvider,
} from "../../lib/api";
import type { BuiltInRuntimeStatus, ModelInfo, ProviderConfig, ProviderHealth, ThemeMode, WorkspaceInfo } from "../../types/ark";
import { Badge } from "../../ui/badge";
import { Button } from "../../ui/button";
import { Input } from "../../ui/input";
import { Panel } from "../../ui/panel";
import { Select } from "../../ui/select";
import { DiagnosticsPanel } from "../diagnostics/DiagnosticsPanel";

interface SettingsViewProps {
  workspacePath: string;
  providers: ProviderConfig[];
  models: ModelInfo[];
  providerHealth: Record<string, ProviderHealth>;
  theme: ThemeMode;
  workspace?: WorkspaceInfo | null;
  builtInStatus: BuiltInRuntimeStatus;
  onBuiltInStatusChange: (status: BuiltInRuntimeStatus) => void;
  onThemeChange: (theme: ThemeMode) => void;
  onWorkspaceChange: (workspace: WorkspaceInfo) => void;
  onProviderSaved: (provider: ProviderConfig) => void;
  onModelsRefresh: (result: { health: ProviderHealth; models: ModelInfo[]; provider: ProviderConfig }) => void;
  onBack: () => void;
  onError: (message: string) => void;
}

export function SettingsView({
  workspacePath,
  providers,
  models,
  providerHealth,
  theme,
  workspace,
  builtInStatus,
  onBuiltInStatusChange,
  onThemeChange,
  onWorkspaceChange,
  onProviderSaved,
  onModelsRefresh,
  onBack,
  onError,
}: SettingsViewProps) {
  const [selectedProviderId, setSelectedProviderId] = React.useState(providers[0]?.id ?? "");
  const [workspaceDraft, setWorkspaceDraft] = React.useState(workspace?.rootPath ?? "");
  const [workspaceSaving, setWorkspaceSaving] = React.useState(false);

  const provider = providers.find((p) => p.id === selectedProviderId) ?? providers[0];
  const health = providerHealth[provider?.id ?? ""] ?? null;
  const providerModels = models.filter((m) => m.providerId === (provider?.id ?? ""));

  React.useEffect(() => {
    if (providers.length > 0 && !providers.some((p) => p.id === selectedProviderId)) {
      setSelectedProviderId(providers[0].id);
    }
  }, [providers, selectedProviderId]);

  React.useEffect(() => {
    setWorkspaceDraft(workspace?.rootPath ?? "");
  }, [workspace?.rootPath]);

  async function saveWorkspace() {
    const nextPath = workspaceDraft.trim();
    if (!nextPath) {
      onError("Workspace path cannot be empty.");
      return;
    }

    setWorkspaceSaving(true);
    try {
      const result = await setWorkspace(nextPath);
      onWorkspaceChange(result);
    } catch (error) {
      onError(getErrorMessage(error));
    } finally {
      setWorkspaceSaving(false);
    }
  }

  async function restoreDefaultWorkspace() {
    setWorkspaceSaving(true);
    try {
      const result = await resetWorkspace();
      onWorkspaceChange(result);
    } catch (error) {
      onError(getErrorMessage(error));
    } finally {
      setWorkspaceSaving(false);
    }
  }

  return (
    <section className="flex min-w-0 flex-1 flex-col">
      <header className="flex h-14 items-center justify-between border-b border-border px-4">
        <div className="flex items-center gap-3">
          <Button size="icon" variant="ghost" onClick={onBack} aria-label="Back to chat">
            <ArrowLeft className="h-4 w-4" />
          </Button>
          <div>
            <h1 className="text-sm font-semibold">Settings</h1>
            <p className="text-xs text-muted-foreground">Control center for local runtime, storage, and privacy.</p>
          </div>
        </div>
        <Badge tone={health?.isReachable ? "success" : "warning"}>{health?.status ?? "unknown"}</Badge>
      </header>

      <div className="min-h-0 flex-1 overflow-y-auto p-5">
        <div className="mx-auto grid max-w-4xl gap-4">
          <Panel className="p-4">
            <div className="mb-4 flex items-center gap-2">
              <Sun className="h-4 w-4" />
              <h2 className="text-sm font-semibold">Appearance</h2>
            </div>
            <div className="flex gap-2">
              <Button variant={theme === "dark" ? "primary" : "secondary"} onClick={() => onThemeChange("dark")}>
                <Moon className="h-4 w-4" />
                Dark
              </Button>
              <Button variant={theme === "light" ? "primary" : "secondary"} onClick={() => onThemeChange("light")}>
                <Sun className="h-4 w-4" />
                Light
              </Button>
            </div>
          </Panel>

          <Panel className="p-4">
            <div className="mb-4 flex items-center justify-between gap-2">
              <div className="flex items-center gap-2">
                <SlidersHorizontal className="h-4 w-4" />
                <h2 className="text-sm font-semibold">Provider</h2>
              </div>
              {providers.length > 1 && (
                <div className="flex items-center gap-1 rounded-md border border-border bg-muted/40 p-0.5">
                  {providers.map((p) => (
                    <button
                      key={p.id}
                      onClick={() => setSelectedProviderId(p.id)}
                      className={cn(
                        "rounded px-2.5 py-1 text-xs font-medium transition-colors",
                        p.id === selectedProviderId
                          ? "bg-background text-foreground shadow-sm"
                          : "text-muted-foreground hover:text-foreground",
                      )}
                    >
                      {p.name}
                    </button>
                  ))}
                </div>
              )}
            </div>
            {provider?.providerType === "built_in" ? (
              <BuiltInRuntimeForm
                key={provider.id}
                status={builtInStatus}
                onStatusChange={onBuiltInStatusChange}
                onModelsRefresh={onModelsRefresh}
                onError={onError}
              />
            ) : provider ? (
              <ProviderForm
                key={provider.id}
                provider={provider}
                models={providerModels}
                onProviderSaved={onProviderSaved}
                onModelsRefresh={onModelsRefresh}
                onError={onError}
              />
            ) : (
              <p className="text-sm text-muted-foreground">No providers configured.</p>
            )}
          </Panel>

          <Panel className="p-4">
            <div className="mb-2 flex items-center gap-2">
              <Database className="h-4 w-4" />
              <h2 className="text-sm font-semibold">Storage</h2>
            </div>
            <div className="grid gap-3">
              <div className="grid gap-1.5 text-sm">
                <span>Workspace folder</span>
                <Input
                  value={workspaceDraft}
                  onChange={(event) => setWorkspaceDraft(event.target.value)}
                  placeholder={workspace?.defaultRootPath ?? workspacePath}
                />
              </div>
              <div className="grid gap-1 text-xs text-muted-foreground">
                <span className="break-all">Database: {workspace?.databasePath ?? workspacePath}</span>
                {workspace?.configPath && <span className="break-all">Workspace config: {workspace.configPath}</span>}
              </div>
              <div className="flex flex-wrap items-center gap-2">
                <Badge tone={workspace?.isPortable ? "success" : "muted"}>
                  {workspace?.isPortable ? "portable workspace" : "default workspace"}
                </Badge>
                {workspace?.requiresRestart && <Badge tone="warning">restart required</Badge>}
              </div>
              {workspace?.requiresRestart && (
                <p className="text-sm text-muted-foreground">
                  Close and reopen Ark to use the selected workspace. Existing data is not moved automatically.
                </p>
              )}
              <div className="flex flex-wrap gap-2">
                <Button
                  onClick={saveWorkspace}
                  disabled={workspaceSaving || workspaceDraft.trim() === workspace?.rootPath}
                >
                  <Save className="h-4 w-4" />
                  Save workspace
                </Button>
                <Button variant="secondary" onClick={restoreDefaultWorkspace} disabled={workspaceSaving}>
                  Use default
                </Button>
              </div>
            </div>
          </Panel>

          <Panel className="p-4">
            <div className="mb-2 flex items-center gap-2">
              <Shield className="h-4 w-4" />
              <h2 className="text-sm font-semibold">Privacy</h2>
            </div>
            <p className="text-sm text-muted-foreground">
              Chats stay local and use the configured provider endpoints. Cloud providers, telemetry, tools, memory, and
              document chat are not enabled in this milestone.
            </p>
          </Panel>

          <DiagnosticsPanel provider={provider} selectedModel={provider?.defaultModelId ?? ""} onError={onError} />
        </div>
      </div>
    </section>
  );
}

function ProviderForm({
  provider,
  models,
  onProviderSaved,
  onModelsRefresh,
  onError,
}: {
  provider: ProviderConfig;
  models: ModelInfo[];
  onProviderSaved: (provider: ProviderConfig) => void;
  onModelsRefresh: (result: { health: ProviderHealth; models: ModelInfo[]; provider: ProviderConfig }) => void;
  onError: (message: string) => void;
}) {
  const [baseUrl, setBaseUrl] = React.useState(provider.baseUrl ?? "");
  const [defaultModelId, setDefaultModelId] = React.useState(provider.defaultModelId ?? "");
  const [temperature, setTemperature] = React.useState(String(provider.defaultTemperature ?? 0.7));
  const [maxTokens, setMaxTokens] = React.useState(String(provider.defaultMaxTokens ?? 2048));
  const [saving, setSaving] = React.useState(false);
  const [refreshing, setRefreshing] = React.useState(false);

  async function saveProvider() {
    setSaving(true);
    try {
      const saved = await updateProvider({
        providerId: provider.id,
        baseUrl,
        defaultModelId: defaultModelId || null,
        temperature: Number.parseFloat(temperature),
        maxTokens: Number.parseInt(maxTokens, 10),
        streamingEnabled: true,
      });
      onProviderSaved(saved);
    } catch (error) {
      onError(getErrorMessage(error));
    } finally {
      setSaving(false);
    }
  }

  async function handleRefresh() {
    setRefreshing(true);
    try {
      const result = await refreshModels(provider.id);
      onModelsRefresh(result);
      if (result.provider.defaultModelId) {
        setDefaultModelId(result.provider.defaultModelId);
      }
    } catch (error) {
      onError(getErrorMessage(error));
    } finally {
      setRefreshing(false);
    }
  }

  return (
    <div className="grid gap-4">
      <div className="grid gap-1.5">
        <label className="text-sm">
          Base URL
          <Input value={baseUrl} onChange={(event) => setBaseUrl(event.target.value)} className="mt-1.5" />
        </label>
        {provider.providerType === "local_inference_host" && (
          <div className="rounded-md border border-border bg-muted/40 px-3 py-2.5 text-xs text-muted-foreground">
            <div className="mb-1.5 flex items-center gap-1.5 font-medium text-foreground">
              <Info className="h-3.5 w-3.5 shrink-0" />
              About local inference host
            </div>
            <p className="mb-2">
              Ark connects to an <span className="font-medium">OpenAI-compatible HTTP server</span> you run locally —
              it doesn't manage or bundle an inference engine. Start one of the following, then click{" "}
              <span className="font-medium">Refresh models</span>:
            </p>
            <ul className="ml-3 grid gap-1">
              <li>
                <span className="font-medium text-foreground">llama.cpp</span> —{" "}
                <code className="rounded bg-background px-1 font-mono text-[11px]">
                  llama-server --model model.gguf --port 8080
                </code>
              </li>
              <li>
                <span className="font-medium text-foreground">LM Studio</span> — enable the Local Server from the
                Developer tab
              </li>
              <li>
                <span className="font-medium text-foreground">Jan</span> — start the local API server in Jan settings
              </li>
            </ul>
            <p className="mt-2">
              Models are discovered automatically via <code className="rounded bg-background px-1 font-mono text-[11px]">/v1/models</code> once
              the server is reachable at the Base URL above.
            </p>
          </div>
        )}
      </div>
      <label className="grid gap-1.5 text-sm">
        Default model
        <Select value={defaultModelId} onChange={(event) => setDefaultModelId(event.target.value)}>
          <option value="">No model selected</option>
          {models.map((model) => (
            <option key={model.id} value={model.name}>
              {model.displayName ?? model.name}
            </option>
          ))}
        </Select>
      </label>
      <div className="grid gap-3 sm:grid-cols-2">
        <label className="grid gap-1.5 text-sm">
          Temperature
          <Input value={temperature} onChange={(event) => setTemperature(event.target.value)} inputMode="decimal" />
        </label>
        <label className="grid gap-1.5 text-sm">
          Max tokens
          <Input value={maxTokens} onChange={(event) => setMaxTokens(event.target.value)} inputMode="numeric" />
        </label>
      </div>
      <div className="flex flex-wrap gap-2">
        <Button onClick={saveProvider} disabled={saving}>
          <Save className="h-4 w-4" />
          Save provider
        </Button>
        <Button variant="secondary" onClick={handleRefresh} disabled={refreshing}>
          <RefreshCw className={refreshing ? "h-4 w-4 animate-spin" : "h-4 w-4"} />
          Refresh models
        </Button>
      </div>
    </div>
  );
}

function BuiltInRuntimeForm({
  status,
  onStatusChange,
  onModelsRefresh,
  onError,
}: {
  status: BuiltInRuntimeStatus;
  onStatusChange: (status: BuiltInRuntimeStatus) => void;
  onModelsRefresh: (result: { health: ProviderHealth; models: ModelInfo[]; provider: ProviderConfig }) => void;
  onError: (message: string) => void;
}) {
  const [modelPath, setModelPath] = React.useState(
    () => localStorage.getItem("ark.builtIn.modelPath") ?? "",
  );
  const [starting, setStarting] = React.useState(false);
  const [stopping, setStopping] = React.useState(false);
  const [refreshing, setRefreshing] = React.useState(false);

  async function handleStart() {
    const path = modelPath.trim();
    if (!path) {
      onError("Enter the path to a GGUF model file before starting.");
      return;
    }
    localStorage.setItem("ark.builtIn.modelPath", path);
    setStarting(true);
    try {
      const next = await startBuiltInRuntime(path);
      onStatusChange(next);
      const result = await refreshModels("built_in");
      onModelsRefresh(result);
    } catch (err) {
      onError(getErrorMessage(err));
    } finally {
      setStarting(false);
    }
  }

  async function handleStop() {
    setStopping(true);
    try {
      await stopBuiltInRuntime();
      onStatusChange({ running: false });
    } catch (err) {
      onError(getErrorMessage(err));
    } finally {
      setStopping(false);
    }
  }

  async function handleRefresh() {
    setRefreshing(true);
    try {
      const result = await refreshModels("built_in");
      onModelsRefresh(result);
    } catch (err) {
      onError(getErrorMessage(err));
    } finally {
      setRefreshing(false);
    }
  }

  const modelFileName = status.modelPath?.split(/[/\\]/).pop() ?? null;

  return (
    <div className="grid gap-4">
      <div className="flex items-center gap-2 rounded-md border border-border px-3 py-2 text-sm">
        {status.running ? (
          <CheckCircle2 className="h-4 w-4 shrink-0 text-emerald-500" />
        ) : (
          <Circle className="h-4 w-4 shrink-0 text-muted-foreground" />
        )}
        <span className="font-medium">{status.running ? "Running" : "Stopped"}</span>
        {status.running && status.port && (
          <span className="text-muted-foreground">· port {status.port}</span>
        )}
        {modelFileName && (
          <span className="ml-auto max-w-[220px] truncate text-xs text-muted-foreground">
            {modelFileName}
          </span>
        )}
      </div>

      <label className="grid gap-1.5 text-sm">
        Model file
        <Input
          value={modelPath}
          onChange={(e) => setModelPath(e.target.value)}
          placeholder="C:\Models\llama-3-8b-q4_K_M.gguf"
          disabled={status.running || starting}
        />
        <span className="text-xs text-muted-foreground">
          GGUF format only. Find models on Hugging Face (search "GGUF") or convert with{" "}
          <code className="rounded bg-muted px-1 text-[11px]">llama.cpp convert</code>.
        </span>
      </label>

      {starting && (
        <div className="flex items-start gap-2 rounded-md border border-border bg-muted/40 px-3 py-2 text-xs text-muted-foreground">
          <AlertCircle className="mt-0.5 h-3.5 w-3.5 shrink-0" />
          Loading model into memory — this can take 15–60 seconds depending on size.
        </div>
      )}

      <div className="flex flex-wrap gap-2">
        {status.running ? (
          <Button variant="secondary" onClick={handleStop} disabled={stopping}>
            Stop runtime
          </Button>
        ) : (
          <Button onClick={handleStart} disabled={starting || !modelPath.trim()}>
            {starting ? "Starting…" : "Start runtime"}
          </Button>
        )}
        <Button variant="secondary" onClick={handleRefresh} disabled={!status.running || refreshing}>
          <RefreshCw className={refreshing ? "h-4 w-4 animate-spin" : "h-4 w-4"} />
          Refresh models
        </Button>
      </div>

      <div className="rounded-md border border-border bg-muted/40 px-3 py-2.5 text-xs text-muted-foreground">
        <div className="mb-1 flex items-center gap-1.5 font-medium text-foreground">
          <Info className="h-3.5 w-3.5 shrink-0" />
          Bundled llama.cpp — fully offline
        </div>
        <p>
          Ark ships a built-in inference engine. No external software needed — just point it at a
          GGUF model file and start. Works without internet. For GPU acceleration, use the Ollama
          or Local Inference Host provider instead.
        </p>
      </div>
    </div>
  );
}

function cn(...classes: Array<string | undefined | false | null>): string {
  return classes.filter(Boolean).join(" ");
}
