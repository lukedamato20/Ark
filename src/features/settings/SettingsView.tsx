import {
  AlertCircle,
  ArrowLeft,
  CheckCircle2,
  Circle,
  Database,
  Download,
  FileText,
  HardDrive,
  Info,
  Loader2,
  Moon,
  RefreshCw,
  Save,
  Shield,
  SlidersHorizontal,
  Sun,
  Trash2,
} from "lucide-react";
import * as React from "react";
import { providerIsVisible, releaseCapabilities } from "../../config/releaseCapabilities";
import { getErrorMessage } from "../../lib/arkErrors";
import { validateNumberInput } from "../../lib/numberField";
import { useArkClient } from "../../lib/useArkClient";
import type {
  AppErrorShape,
  BackupResult,
  BuiltInRuntimeStatus,
  DiagnosticsBundle,
  ModelInfo,
  OllamaPullProgress,
  ProviderConfig,
  ProviderHealth,
  RestorePreview,
  SecretMetadata,
  SecretStoreStatus,
  ThemeMode,
  WorkspaceInfo,
  WorkspaceProtectionStatus,
} from "../../types/ark";
import { Badge } from "../../ui/badge";
import { Button } from "../../ui/button";
import { Input } from "../../ui/input";
import { NumberField } from "../../ui/numberField";
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
  /** ARC-006: device-scoped — see docs/settings-catalog.md. */
  builtInModelPath: string | null;
  onBuiltInModelPathChange: (path: string) => void;
  /** OPS-001: opt-in, off by default — see `observability.rs`'s module doc. */
  crashCaptureEnabled: boolean;
  onCrashCaptureEnabledChange: (enabled: boolean) => void;
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
  builtInModelPath,
  onBuiltInModelPathChange,
  crashCaptureEnabled,
  onCrashCaptureEnabledChange,
  onThemeChange,
  onWorkspaceChange,
  onProviderSaved,
  onModelsRefresh,
  onBack,
  onError,
}: SettingsViewProps) {
  const client = useArkClient();
  const visibleProviders = React.useMemo(
    () => providers.filter((candidate) => providerIsVisible(candidate.providerType)),
    [providers],
  );
  const [selectedProviderId, setSelectedProviderId] = React.useState(visibleProviders[0]?.id ?? "");
  const [workspaceDraft, setWorkspaceDraft] = React.useState(workspace?.rootPath ?? "");
  const [workspaceSaving, setWorkspaceSaving] = React.useState(false);
  const [copyWorkspaceData, setCopyWorkspaceData] = React.useState(false);
  const [secretStoreStatus, setSecretStoreStatus] = React.useState<SecretStoreStatus | null>(null);
  const [secretStoreChecking, setSecretStoreChecking] = React.useState(false);
  const [protectionStatus, setProtectionStatus] = React.useState<WorkspaceProtectionStatus | null>(null);
  const [protectionBusy, setProtectionBusy] = React.useState(false);
  const [protectionError, setProtectionError] = React.useState<string | null>(null);
  const [protectionConfirmation, setProtectionConfirmation] = React.useState<"enable" | "rotate" | "disable" | null>(
    null,
  );
  const [recoveryKey, setRecoveryKey] = React.useState<string | null>(null);
  const [recoveryDraft, setRecoveryDraft] = React.useState("");

  const checkSecretStore = React.useCallback(async () => {
    setSecretStoreChecking(true);
    try {
      setSecretStoreStatus(await client.getSecretStoreStatus());
    } catch (error) {
      setSecretStoreStatus({
        available: false,
        code: "secret_store_failed",
        message: getErrorMessage(error),
      });
    } finally {
      setSecretStoreChecking(false);
    }
  }, [client]);

  React.useEffect(() => {
    void checkSecretStore();
  }, [checkSecretStore]);

  const checkWorkspaceProtection = React.useCallback(async () => {
    try {
      setProtectionStatus(await client.getWorkspaceProtectionStatus());
      setProtectionError(null);
    } catch (error) {
      setProtectionError(getErrorMessage(error));
    }
  }, [client]);

  React.useEffect(() => {
    void checkWorkspaceProtection();
  }, [checkWorkspaceProtection]);

  const provider = visibleProviders.find((candidate) => candidate.id === selectedProviderId) ?? visibleProviders[0];
  const health = providerHealth[provider?.id ?? ""] ?? null;
  const providerModels = models.filter((m) => m.providerId === (provider?.id ?? ""));

  React.useEffect(() => {
    if (visibleProviders.length > 0 && !visibleProviders.some((candidate) => candidate.id === selectedProviderId)) {
      setSelectedProviderId(visibleProviders[0].id);
    }
  }, [selectedProviderId, visibleProviders]);

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
      const result = await client.setWorkspace(nextPath, copyWorkspaceData);
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
      const result = await client.resetWorkspace();
      onWorkspaceChange(result);
    } catch (error) {
      onError(getErrorMessage(error));
    } finally {
      setWorkspaceSaving(false);
    }
  }

  async function changeProtection(action: "enable" | "rotate" | "disable") {
    setProtectionBusy(true);
    setProtectionError(null);
    setProtectionConfirmation(null);
    try {
      if (action === "disable") {
        setProtectionStatus(await client.disableWorkspaceEncryption());
        setRecoveryKey(null);
      } else {
        const change =
          action === "enable" ? await client.enableWorkspaceEncryption() : await client.rotateWorkspaceEncryption();
        setProtectionStatus(change.status);
        setRecoveryKey(change.recoveryKey ?? null);
      }
    } catch (error) {
      setProtectionError(getErrorMessage(error));
    } finally {
      setProtectionBusy(false);
    }
  }

  async function restoreRecoveryKey() {
    const key = recoveryDraft.trim();
    if (!key) return;
    // Key material leaves component state before the asynchronous native operation starts.
    setRecoveryDraft("");
    setProtectionBusy(true);
    setProtectionError(null);
    try {
      setProtectionStatus(await client.restoreWorkspaceRecoveryKey(key));
    } catch (error) {
      setProtectionError(getErrorMessage(error));
    } finally {
      setProtectionBusy(false);
    }
  }

  return (
    <main aria-label="Settings" className="flex min-w-0 flex-1 flex-col">
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
              <Button
                variant={theme === "dark" ? "primary" : "secondary"}
                aria-pressed={theme === "dark"}
                onClick={() => onThemeChange("dark")}
              >
                <Moon className="h-4 w-4" />
                Dark
              </Button>
              <Button
                variant={theme === "light" ? "primary" : "secondary"}
                aria-pressed={theme === "light"}
                onClick={() => onThemeChange("light")}
              >
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
              {visibleProviders.length > 1 && (
                <div
                  role="tablist"
                  aria-label="Providers"
                  className="flex items-center gap-1 rounded-md border border-border bg-muted/40 p-0.5"
                >
                  {visibleProviders.map((p) => (
                    <button
                      key={p.id}
                      type="button"
                      role="tab"
                      id={`provider-tab-${p.id}`}
                      aria-selected={p.id === selectedProviderId}
                      aria-controls={`provider-tabpanel-${p.id}`}
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
            <div
              role="tabpanel"
              id={provider ? `provider-tabpanel-${provider.id}` : undefined}
              aria-labelledby={provider ? `provider-tab-${provider.id}` : undefined}
            >
              {provider?.providerType === "built_in" ? (
                <BuiltInRuntimeForm
                  key={provider.id}
                  status={builtInStatus}
                  onStatusChange={onBuiltInStatusChange}
                  modelPath={builtInModelPath}
                  onModelPathChange={onBuiltInModelPathChange}
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
                  secretStoreStatus={secretStoreStatus}
                  onSecretStoreRetry={checkSecretStore}
                />
              ) : (
                <p className="text-sm text-muted-foreground">No providers configured.</p>
              )}
            </div>
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
                <p className="text-sm text-muted-foreground">Close and reopen Ark to use the selected workspace.</p>
              )}
              <label className="flex items-start gap-2 text-xs text-muted-foreground">
                <input
                  type="checkbox"
                  checked={copyWorkspaceData}
                  onChange={(event) => setCopyWorkspaceData(event.target.checked)}
                  className="mt-0.5 h-4 w-4 accent-primary"
                />
                <span>
                  Copy current conversation data to the new location. The original is left untouched — nothing is ever
                  deleted automatically. Leave unchecked to start the new location empty.
                </span>
              </label>
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
              <div className="grid gap-3 rounded-md border border-border bg-muted/30 p-3">
                <div className="flex flex-wrap items-start justify-between gap-2">
                  <div>
                    <div className="text-sm font-medium">Workspace encryption</div>
                    <p className="mt-1 text-xs text-muted-foreground">
                      {protectionStatus?.message ?? "Checking workspace protection…"}
                    </p>
                  </div>
                  <Badge
                    tone={
                      protectionStatus?.locked
                        ? "warning"
                        : protectionStatus?.mode === "encrypted"
                          ? "success"
                          : "muted"
                    }
                  >
                    {protectionStatus?.locked
                      ? "locked"
                      : protectionStatus?.mode === "encrypted"
                        ? "encrypted"
                        : "plaintext"}
                  </Badge>
                </div>

                {protectionStatus?.transitionInProgress && (
                  <p className="text-xs text-warning">
                    An interrupted protection change was detected. Reopen Ark to reconcile its verified database copy.
                  </p>
                )}

                {protectionStatus?.locked && (
                  <div className="grid gap-2">
                    <label className="grid gap-1.5 text-sm" htmlFor="workspace-recovery-key">
                      Recovery key
                      <Input
                        id="workspace-recovery-key"
                        type="password"
                        autoComplete="off"
                        value={recoveryDraft}
                        onChange={(event) => setRecoveryDraft(event.target.value)}
                        placeholder="ark-recovery-v1:…"
                      />
                    </label>
                    <Button
                      variant="secondary"
                      onClick={() => void restoreRecoveryKey()}
                      disabled={protectionBusy || !recoveryDraft.trim()}
                    >
                      Restore and unlock
                    </Button>
                    <p className="text-xs text-muted-foreground">
                      A forgotten key cannot be reset. Ark never erases or replaces a locked workspace; restore a
                      matching backup and recovery key.
                    </p>
                  </div>
                )}

                {recoveryKey && (
                  <div className="grid gap-2 rounded-md border border-warning/50 bg-warning/10 p-3" role="alert">
                    <div className="text-sm font-medium">Save this recovery key now</div>
                    <p className="text-xs text-muted-foreground">
                      Ark shows it once and does not store a second recoverable copy. Keep it separate from workspace
                      backups and cloud-synced folders.
                    </p>
                    <code className="select-all break-all rounded bg-background p-2 text-xs">{recoveryKey}</code>
                    <Button variant="secondary" onClick={() => setRecoveryKey(null)}>
                      I saved this key
                    </Button>
                  </div>
                )}

                {protectionConfirmation && (
                  <div className="grid gap-2 rounded-md border border-warning/50 p-3" role="alert">
                    <p className="text-sm">
                      {protectionConfirmation === "enable" &&
                        "Ark will create and verify an encrypted copy, then replace the plaintext database. Losing both the OS key and recovery key makes the data unrecoverable."}
                      {protectionConfirmation === "rotate" &&
                        "The old recovery key stops working after rotation. Save the new key before dismissing it."}
                      {protectionConfirmation === "disable" &&
                        "Ark will create a verified plaintext copy. Anyone who can read the workspace files can then read chat data."}
                    </p>
                    <div className="flex flex-wrap gap-2">
                      <Button onClick={() => void changeProtection(protectionConfirmation)} disabled={protectionBusy}>
                        Confirm
                      </Button>
                      <Button variant="secondary" onClick={() => setProtectionConfirmation(null)}>
                        Cancel
                      </Button>
                    </div>
                  </div>
                )}

                {!protectionStatus?.locked && !recoveryKey && !protectionConfirmation && (
                  <div className="flex flex-wrap gap-2">
                    {protectionStatus?.mode === "encrypted" ? (
                      <>
                        <Button
                          variant="secondary"
                          onClick={() => setProtectionConfirmation("rotate")}
                          disabled={protectionBusy}
                        >
                          Rotate key
                        </Button>
                        <Button
                          variant="secondary"
                          onClick={() => setProtectionConfirmation("disable")}
                          disabled={protectionBusy}
                        >
                          Decrypt workspace
                        </Button>
                      </>
                    ) : (
                      <Button onClick={() => setProtectionConfirmation("enable")} disabled={protectionBusy}>
                        Encrypt workspace
                      </Button>
                    )}
                    <Button variant="ghost" onClick={() => void checkWorkspaceProtection()} disabled={protectionBusy}>
                      Refresh status
                    </Button>
                  </div>
                )}
                {protectionError && <p className="text-sm text-destructive">{protectionError}</p>}
              </div>
            </div>
          </Panel>

          <BackupRestorePanel onError={onError} />

          <DiagnosticsBundlePanel
            onError={onError}
            crashCaptureEnabled={crashCaptureEnabled}
            onCrashCaptureEnabledChange={onCrashCaptureEnabledChange}
          />

          <Panel className="p-4">
            <div className="mb-2 flex items-center gap-2">
              <Shield className="h-4 w-4" />
              <h2 className="text-sm font-semibold">Privacy</h2>
            </div>
            <p className="text-sm text-muted-foreground">
              Chats stay local and use the configured provider endpoints. Cloud providers, telemetry, tools, memory, and
              document chat are not enabled in this milestone.
            </p>
            <div
              className="mt-3 flex flex-wrap items-center justify-between gap-2 rounded-md border border-border bg-muted/40 px-3 py-2"
              role="status"
            >
              <div className="min-w-0">
                <div className="text-xs font-medium text-foreground">Operating-system credential storage</div>
                <div className="text-xs text-muted-foreground">
                  {secretStoreStatus?.message ?? "Checking credential storage…"}
                </div>
              </div>
              <div className="flex items-center gap-2">
                <Badge tone={secretStoreStatus?.available ? "success" : "warning"}>
                  {secretStoreStatus?.available ? "available" : secretStoreChecking ? "checking" : "needs attention"}
                </Badge>
                {!secretStoreStatus?.available && (
                  <Button
                    size="sm"
                    variant="secondary"
                    onClick={() => void checkSecretStore()}
                    disabled={secretStoreChecking}
                  >
                    <RefreshCw className={secretStoreChecking ? "h-3.5 w-3.5 animate-spin" : "h-3.5 w-3.5"} />
                    Retry
                  </Button>
                )}
              </div>
            </div>
          </Panel>

          <DiagnosticsPanel provider={provider} selectedModel={provider?.defaultModelId ?? ""} onError={onError} />
        </div>
      </div>
    </main>
  );
}

function ProviderForm({
  provider,
  models,
  onProviderSaved,
  onModelsRefresh,
  onError,
  secretStoreStatus,
  onSecretStoreRetry,
}: {
  provider: ProviderConfig;
  models: ModelInfo[];
  onProviderSaved: (provider: ProviderConfig) => void;
  onModelsRefresh: (result: { health: ProviderHealth; models: ModelInfo[]; provider: ProviderConfig }) => void;
  onError: (message: string) => void;
  secretStoreStatus: SecretStoreStatus | null;
  onSecretStoreRetry: () => Promise<void>;
}) {
  const client = useArkClient();
  const [baseUrl, setBaseUrl] = React.useState(provider.baseUrl ?? "");
  const [defaultModelId, setDefaultModelId] = React.useState(provider.defaultModelId ?? "");
  const [temperature, setTemperature] = React.useState(String(provider.defaultTemperature ?? 0.7));
  const [maxTokens, setMaxTokens] = React.useState(String(provider.defaultMaxTokens ?? 2048));
  const [saving, setSaving] = React.useState(false);
  const [refreshing, setRefreshing] = React.useState(false);
  // SEC-001: when the backend rejects a save because the URL classifies as a public/remote
  // destination, we surface its exact risk message and require an explicit acknowledgment
  // before retrying the save with acknowledgeRemoteRisk: true.
  const [remoteRiskMessage, setRemoteRiskMessage] = React.useState<string | null>(null);
  const [riskAcknowledged, setRiskAcknowledged] = React.useState(false);
  const [convertToRemoteProvider, setConvertToRemoteProvider] = React.useState(!provider.isLocal);
  const [allowInsecureRemote, setAllowInsecureRemote] = React.useState(provider.allowInsecureRemote);
  const [secretDraft, setSecretDraft] = React.useState("");
  const [secretMetadata, setSecretMetadata] = React.useState<SecretMetadata | null>(null);
  const [secretBusy, setSecretBusy] = React.useState(false);
  const [secretError, setSecretError] = React.useState<string | null>(null);
  const [secretReload, setSecretReload] = React.useState(0);
  const insecureHttpDestination = baseUrl.trim().toLowerCase().startsWith("http://");
  const supportsCredential = provider.capabilities.requiresAuth || Boolean(provider.apiKeyRef);

  React.useEffect(() => {
    setRemoteRiskMessage(null);
    setRiskAcknowledged(false);
    setConvertToRemoteProvider(!provider.isLocal);
    setAllowInsecureRemote(provider.allowInsecureRemote);
  }, [baseUrl, provider.allowInsecureRemote, provider.isLocal]);

  React.useEffect(() => {
    if (!supportsCredential) return;
    let active = true;
    setSecretBusy(true);
    setSecretError(null);
    void client
      .getProviderSecretMetadata(provider.id)
      .then((metadata) => {
        if (active) setSecretMetadata(metadata);
      })
      .catch((error) => {
        if (active) setSecretError(getErrorMessage(error));
      })
      .finally(() => {
        if (active) setSecretBusy(false);
      });
    return () => {
      active = false;
    };
  }, [client, provider.id, secretReload, supportsCredential]);

  async function saveSecret() {
    if (!secretDraft || !secretStoreStatus?.available) return;
    const secret = secretDraft;
    setSecretDraft("");
    setSecretBusy(true);
    setSecretError(null);
    try {
      const metadata = await client.upsertProviderSecret(provider.id, secret);
      setSecretMetadata(metadata);
      onProviderSaved({ ...provider, apiKeyRef: metadata.id });
    } catch (error) {
      setSecretError(getErrorMessage(error));
    } finally {
      setSecretBusy(false);
    }
  }

  async function deleteSecret() {
    setSecretBusy(true);
    setSecretError(null);
    try {
      await client.deleteProviderSecret(provider.id);
      setSecretMetadata(null);
      setSecretDraft("");
      onProviderSaved({ ...provider, apiKeyRef: null });
    } catch (error) {
      setSecretError(getErrorMessage(error));
    } finally {
      setSecretBusy(false);
    }
  }

  const temperatureValidation = validateNumberInput(temperature, 0, 2, "Temperature");
  const maxTokensValidation = validateNumberInput(maxTokens, 1, 1_000_000, "Max tokens");
  const numericFieldsValid = temperatureValidation.valid && maxTokensValidation.valid;

  async function saveProvider(acknowledgeRemoteRisk = false) {
    if (!temperatureValidation.valid || !maxTokensValidation.valid) return;
    setSaving(true);
    try {
      const saved = await client.updateProvider({
        providerId: provider.id,
        baseUrl,
        defaultModelId: defaultModelId || null,
        temperature: temperatureValidation.parsed,
        maxTokens: maxTokensValidation.parsed,
        acknowledgeRemoteRisk,
        convertToRemoteProvider,
        allowInsecureRemote,
      });
      onProviderSaved(saved);
      setRemoteRiskMessage(null);
      setRiskAcknowledged(false);
    } catch (error) {
      const code = error && typeof error === "object" && "code" in error ? (error as AppErrorShape).code : undefined;
      if (
        code === "destination_requires_remote_provider_class" ||
        code === "destination_requires_confirmation" ||
        code === "insecure_remote_requires_development_mode"
      ) {
        setRemoteRiskMessage((error as AppErrorShape).message ?? "This destination requires confirmation.");
      } else {
        onError(getErrorMessage(error));
      }
    } finally {
      setSaving(false);
    }
  }

  async function handleRefresh() {
    setRefreshing(true);
    try {
      const result = await client.refreshModels(provider.id);
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
              Ark connects to an <span className="font-medium">OpenAI-compatible HTTP server</span> you run locally — it
              doesn't manage or bundle an inference engine. Start one of the following, then click{" "}
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
              Models are discovered automatically via{" "}
              <code className="rounded bg-background px-1 font-mono text-[11px]">/v1/models</code> once the server is
              reachable at the Base URL above.
            </p>
          </div>
        )}
        <div className="text-xs text-muted-foreground">
          Provider class:{" "}
          <span className="font-medium text-foreground">{provider.isLocal ? "Local-only" : "Remote"}</span>
        </div>
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
        <NumberField
          id={`${provider.id}-temperature`}
          label="Temperature"
          value={temperature}
          onChange={setTemperature}
          min={0}
          max={2}
          help="Controls response randomness — lower is more focused, higher is more varied."
        />
        <NumberField
          id={`${provider.id}-max-tokens`}
          label="Max tokens"
          value={maxTokens}
          onChange={setMaxTokens}
          min={1}
          max={1_000_000}
          help="Upper bound on response length."
        />
      </div>
      {supportsCredential && (
        <div className="grid gap-2 rounded-md border border-border bg-muted/20 p-3">
          <div>
            <div className="text-sm font-medium">API credential</div>
            <p className="text-xs text-muted-foreground">
              Stored by the operating system. Ark keeps only an opaque reference in this workspace and never returns the
              credential to the UI after saving.
            </p>
          </div>
          {secretMetadata && (
            <div className="flex items-center gap-2 text-xs">
              <Badge tone={secretMetadata.available ? "success" : "warning"}>
                {secretMetadata.available ? "connected" : "reconnection required"}
              </Badge>
              <span aria-label="Saved credential">{secretMetadata.masked}</span>
            </div>
          )}
          <label className="grid gap-1.5 text-sm">
            {secretMetadata ? "Replace credential" : "Credential"}
            <Input
              type="password"
              value={secretDraft}
              onChange={(event) => setSecretDraft(event.target.value)}
              autoComplete="new-password"
              autoCapitalize="none"
              spellCheck={false}
              disabled={secretBusy || !secretStoreStatus?.available}
            />
          </label>
          {!secretStoreStatus?.available && (
            <div role="alert" className="text-xs text-amber-700 dark:text-amber-300">
              {secretStoreStatus?.message ?? "Credential storage is not available."}{" "}
              <button
                className="underline"
                onClick={() => void onSecretStoreRetry()}
                disabled={secretStoreStatus === null}
              >
                Retry
              </button>
            </div>
          )}
          {secretError && (
            <div role="alert" className="flex flex-wrap items-center gap-2 text-xs text-destructive">
              <span>{secretError}</span>
              <Button size="sm" variant="secondary" onClick={() => setSecretReload((value) => value + 1)}>
                Retry status
              </Button>
            </div>
          )}
          <div className="flex flex-wrap gap-2">
            <Button
              onClick={() => void saveSecret()}
              disabled={!secretDraft || secretBusy || !secretStoreStatus?.available}
            >
              <Save className="h-4 w-4" />
              Save credential
            </Button>
            {secretMetadata && (
              <Button variant="secondary" onClick={() => void deleteSecret()} disabled={secretBusy}>
                <Trash2 className="h-4 w-4" />
                Remove credential
              </Button>
            )}
          </div>
        </div>
      )}
      {remoteRiskMessage && (
        <div role="alert" className="rounded-md border border-amber-500/30 bg-amber-500/10 p-3 text-xs">
          <div className="mb-2 flex items-start gap-1.5 font-medium text-amber-700 dark:text-amber-300">
            <AlertCircle className="mt-0.5 h-3.5 w-3.5 shrink-0" aria-hidden="true" />
            {remoteRiskMessage}
          </div>
          <label className="flex items-start gap-2 text-foreground">
            <input
              type="checkbox"
              checked={convertToRemoteProvider}
              onChange={(event) => setConvertToRemoteProvider(event.target.checked)}
              className="mt-0.5"
            />
            Convert this provider from Local-only to the Remote provider class.
          </label>
          <label className="mt-2 flex items-start gap-2 text-foreground">
            <input
              type="checkbox"
              checked={riskAcknowledged}
              onChange={(event) => setRiskAcknowledged(event.target.checked)}
              className="mt-0.5"
            />
            I understand that prompts, conversation history, and the configured system prompt leave this device.
          </label>
          {insecureHttpDestination && (
            <label className="mt-2 flex items-start gap-2 text-foreground">
              <input
                type="checkbox"
                checked={allowInsecureRemote}
                onChange={(event) => setAllowInsecureRemote(event.target.checked)}
                className="mt-0.5"
              />
              Development mode: allow this unencrypted HTTP destination. Network observers may read or alter requests.
            </label>
          )}
          <div className="mt-2 flex gap-2">
            <Button
              size="sm"
              variant="secondary"
              onClick={() => void saveProvider(true)}
              disabled={
                !convertToRemoteProvider ||
                !riskAcknowledged ||
                (insecureHttpDestination && !allowInsecureRemote) ||
                saving ||
                !numericFieldsValid
              }
            >
              Save anyway
            </Button>
            <Button size="sm" variant="ghost" onClick={() => setRemoteRiskMessage(null)}>
              Cancel
            </Button>
          </div>
        </div>
      )}
      <div className="flex flex-wrap gap-2">
        <Button onClick={() => void saveProvider()} disabled={saving || !numericFieldsValid}>
          <Save className="h-4 w-4" />
          Save provider
        </Button>
        <Button variant="secondary" onClick={handleRefresh} disabled={refreshing}>
          <RefreshCw className={refreshing ? "h-4 w-4 animate-spin" : "h-4 w-4"} />
          Refresh models
        </Button>
      </div>
      {/* ARC-003: gated on the capability (pull/delete support), not a hardcoded providerType
          check — a future provider type that also supports model pull/delete would show this
          panel without any change here. */}
      {provider.capabilities.modelPull && (
        <OllamaModelsPanel provider={provider} models={models} onModelsRefresh={onModelsRefresh} onError={onError} />
      )}
    </div>
  );
}

function OllamaModelsPanel({
  provider,
  models,
  onModelsRefresh,
  onError,
}: {
  provider: ProviderConfig;
  models: ModelInfo[];
  onModelsRefresh: (result: { health: ProviderHealth; models: ModelInfo[]; provider: ProviderConfig }) => void;
  onError: (message: string) => void;
}) {
  const client = useArkClient();
  const [pullName, setPullName] = React.useState("");
  const [pulling, setPulling] = React.useState(false);
  const [pullProgress, setPullProgress] = React.useState<OllamaPullProgress | null>(null);
  const [deletingModel, setDeletingModel] = React.useState<string | null>(null);

  React.useEffect(() => {
    if (!pulling) return;

    let unlisten: (() => void) | undefined;
    void client
      .onOllamaPullProgress((event) => {
        if (event.providerId === provider.id) {
          setPullProgress(event);
        }
      })
      .then((u) => {
        unlisten = u;
      });

    return () => {
      unlisten?.();
    };
  }, [pulling, provider.id, client]);

  async function handlePull() {
    const name = pullName.trim();
    if (!name) return;

    setPulling(true);
    setPullProgress(null);
    try {
      await client.pullOllamaModel(provider.id, name);
      const result = await client.refreshModels(provider.id);
      onModelsRefresh(result);
      setPullName("");
    } catch (error) {
      onError(getErrorMessage(error));
    } finally {
      setPulling(false);
      setPullProgress(null);
    }
  }

  async function handleDelete(modelName: string) {
    const confirmed = window.confirm(`Delete model "${modelName}" from Ollama? This cannot be undone.`);
    if (!confirmed) return;

    setDeletingModel(modelName);
    try {
      await client.deleteOllamaModel(provider.id, modelName);
      const result = await client.refreshModels(provider.id);
      onModelsRefresh(result);
    } catch (error) {
      onError(getErrorMessage(error));
    } finally {
      setDeletingModel(null);
    }
  }

  const pullPercent =
    pullProgress?.total && pullProgress.completed
      ? Math.round((pullProgress.completed / pullProgress.total) * 100)
      : null;

  const availableModels = models.filter((m) => m.isAvailable);

  return (
    <div className="mt-4 border-t border-border pt-4">
      <div className="mb-3 flex items-center gap-2">
        <HardDrive className="h-4 w-4" />
        <h3 className="text-sm font-semibold">Installed models</h3>
        <span className="ml-auto text-xs text-muted-foreground">
          {availableModels.length} model{availableModels.length !== 1 ? "s" : ""}
        </span>
      </div>

      {availableModels.length === 0 ? (
        <p className="text-sm text-muted-foreground">No models installed. Pull one below to get started.</p>
      ) : (
        <div className="mb-4 divide-y divide-border rounded-md border border-border">
          {availableModels.map((model) => {
            const meta = model.metadataJson
              ? (() => {
                  try {
                    return JSON.parse(model.metadataJson!);
                  } catch {
                    return null;
                  }
                })()
              : null;
            const sizeBytes: number | null = meta?.size ?? null;
            const sizeLabel = sizeBytes ? formatBytes(sizeBytes) : null;
            return (
              <div key={model.id} className="flex items-center gap-3 px-3 py-2.5">
                <span className="min-w-0 flex-1 text-sm font-medium">{model.displayName ?? model.name}</span>
                {sizeLabel && <span className="text-xs text-muted-foreground">{sizeLabel}</span>}
                <button
                  type="button"
                  onClick={() => void handleDelete(model.name)}
                  disabled={deletingModel === model.name}
                  aria-label={`Delete ${model.name}`}
                  className="rounded p-1 text-muted-foreground hover:text-destructive focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring disabled:opacity-40"
                >
                  {deletingModel === model.name ? (
                    <Loader2 className="h-3.5 w-3.5 animate-spin" />
                  ) : (
                    <Trash2 className="h-3.5 w-3.5" />
                  )}
                </button>
              </div>
            );
          })}
        </div>
      )}

      <div className="grid gap-2">
        <div className="flex gap-2">
          <input
            type="text"
            value={pullName}
            onChange={(e) => setPullName(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter" && !pulling && pullName.trim()) {
                e.preventDefault();
                void handlePull();
              }
            }}
            placeholder="llama3.2:3b"
            disabled={pulling}
            className="h-9 flex-1 rounded-md border border-input bg-background px-3 text-sm outline-none focus-visible:ring-2 focus-visible:ring-ring disabled:opacity-50"
            aria-label="Model name to pull"
          />
          <Button onClick={handlePull} disabled={pulling || !pullName.trim()}>
            {pulling ? <Loader2 className="h-4 w-4 animate-spin" /> : <Download className="h-4 w-4" />}
            {pulling ? "Pulling…" : "Pull"}
          </Button>
        </div>

        {pulling && pullProgress && (
          <div className="space-y-1">
            <div className="flex items-center justify-between text-xs text-muted-foreground">
              <span className="truncate">{pullProgress.status}</span>
              {pullPercent !== null && <span>{pullPercent}%</span>}
            </div>
            {pullPercent !== null && (
              <div className="h-1.5 overflow-hidden rounded-full bg-muted">
                <div
                  className="h-full rounded-full bg-primary transition-all duration-standard"
                  style={{ width: `${pullPercent}%` }}
                  role="progressbar"
                  aria-valuenow={pullPercent}
                  aria-valuemin={0}
                  aria-valuemax={100}
                />
              </div>
            )}
          </div>
        )}
      </div>
    </div>
  );
}

/**
 * FTR-001: create/preview/restore all reuse typed paths, not native pickers — the same
 * UX-005-established deferral (native file/folder dialogs need a new `@tauri-apps/plugin-dialog`
 * dependency, judged out of scope for that task and unchanged here).
 */
function BackupRestorePanel({ onError }: { onError: (message: string) => void }) {
  const client = useArkClient();
  const [backupDestination, setBackupDestination] = React.useState("");
  const [creatingBackup, setCreatingBackup] = React.useState(false);
  const [backupResult, setBackupResult] = React.useState<BackupResult | null>(null);

  const [restorePath, setRestorePath] = React.useState("");
  const [previewing, setPreviewing] = React.useState(false);
  const [preview, setPreview] = React.useState<RestorePreview | null>(null);
  const [restoreTarget, setRestoreTarget] = React.useState("");
  const [restoring, setRestoring] = React.useState(false);
  const [restoreSuccess, setRestoreSuccess] = React.useState<string | null>(null);

  async function createBackup() {
    if (!backupDestination.trim()) {
      onError("Choose a backup destination folder.");
      return;
    }
    setCreatingBackup(true);
    setBackupResult(null);
    try {
      setBackupResult(await client.createWorkspaceBackup(backupDestination.trim()));
    } catch (error) {
      onError(getErrorMessage(error));
    } finally {
      setCreatingBackup(false);
    }
  }

  async function previewRestore() {
    if (!restorePath.trim()) {
      onError("Enter the path to a backup file.");
      return;
    }
    setPreviewing(true);
    setPreview(null);
    setRestoreSuccess(null);
    try {
      setPreview(await client.previewWorkspaceRestore(restorePath.trim()));
    } catch (error) {
      onError(getErrorMessage(error));
    } finally {
      setPreviewing(false);
    }
  }

  async function restore() {
    if (!preview || !restoreTarget.trim()) return;
    setRestoring(true);
    try {
      await client.restoreWorkspaceBackup(restorePath.trim(), restoreTarget.trim());
      setRestoreSuccess(restoreTarget.trim());
      setPreview(null);
    } catch (error) {
      onError(getErrorMessage(error));
    } finally {
      setRestoring(false);
    }
  }

  return (
    <Panel className="p-4">
      <div className="mb-2 flex items-center gap-2">
        <Database className="h-4 w-4" />
        <h2 className="text-sm font-semibold">Backup &amp; Restore</h2>
      </div>
      <div className="grid gap-4">
        <div className="grid gap-2">
          <div className="text-sm font-medium">Create a backup</div>
          <div className="flex flex-wrap gap-2">
            <Input
              value={backupDestination}
              onChange={(event) => setBackupDestination(event.target.value)}
              placeholder="C:\Backups\Ark"
              className="min-w-64 flex-1"
              aria-label="Backup destination folder"
            />
            <Button onClick={() => void createBackup()} disabled={creatingBackup}>
              {creatingBackup ? <Loader2 className="h-4 w-4 animate-spin" /> : <Download className="h-4 w-4" />}
              Create backup
            </Button>
          </div>
          {backupResult && (
            <div role="status" className="rounded-md border border-border bg-muted/30 p-2.5 text-xs">
              <div className="font-medium text-foreground">Backup created</div>
              <div className="mt-1 break-all text-muted-foreground">{backupResult.backupPath}</div>
              <div className="mt-1 text-muted-foreground">
                {formatBytes(backupResult.manifest.databaseSizeBytes)} · SHA-256{" "}
                {backupResult.manifest.databaseSha256.slice(0, 12)}…
              </div>
            </div>
          )}
        </div>

        <div className="grid gap-2 border-t border-border pt-3">
          <div className="text-sm font-medium">Restore from a backup</div>
          <label className="grid gap-1.5 text-sm">
            Backup file
            <Input
              value={restorePath}
              onChange={(event) => {
                setRestorePath(event.target.value);
                setPreview(null);
              }}
              placeholder="C:\Backups\Ark\ark.sqlite3"
            />
          </label>
          <Button variant="secondary" onClick={() => void previewRestore()} disabled={previewing} className="w-fit">
            {previewing ? <Loader2 className="h-4 w-4 animate-spin" /> : null}
            Preview
          </Button>
          {preview && (
            <div className="grid gap-2 rounded-md border border-border bg-muted/30 p-3 text-xs">
              <div className="flex flex-wrap items-center gap-2">
                <Badge tone={preview.schemaSupported ? "success" : "danger"}>
                  {preview.schemaSupported ? "compatible" : "unsupported schema"}
                </Badge>
                <span className="text-muted-foreground">
                  {preview.conversationCount} conversation{preview.conversationCount === 1 ? "" : "s"},{" "}
                  {preview.messageCount} messages
                </span>
              </div>
              {preview.manifest && (
                <div className="text-muted-foreground">
                  Created {preview.manifest.createdAt} · Ark {preview.manifest.appVersion}
                </div>
              )}
              {preview.schemaSupported ? (
                <>
                  <label className="grid gap-1.5 text-sm text-foreground">
                    Restore to (must be an empty or new folder)
                    <Input
                      value={restoreTarget}
                      onChange={(event) => setRestoreTarget(event.target.value)}
                      placeholder="C:\Ark-restored"
                    />
                  </label>
                  <p className="text-muted-foreground">
                    This never touches your current workspace. Once restored, use "Workspace folder" above to switch to
                    it.
                  </p>
                  <Button
                    onClick={() => void restore()}
                    disabled={restoring || !restoreTarget.trim()}
                    className="w-fit"
                  >
                    {restoring ? <Loader2 className="h-4 w-4 animate-spin" /> : null}
                    Restore to new workspace
                  </Button>
                </>
              ) : (
                <p className="text-destructive">
                  This backup was made by a newer version of Ark than this build supports. Update Ark before restoring
                  it.
                </p>
              )}
            </div>
          )}
          {restoreSuccess && (
            <p role="status" className="text-sm text-emerald-600 dark:text-emerald-300">
              Restored to {restoreSuccess}.
            </p>
          )}
        </div>
      </div>
    </Panel>
  );
}

function DiagnosticsBundlePanel({
  onError,
  crashCaptureEnabled,
  onCrashCaptureEnabledChange,
}: {
  onError: (message: string) => void;
  crashCaptureEnabled: boolean;
  onCrashCaptureEnabledChange: (enabled: boolean) => void;
}) {
  const client = useArkClient();
  const [bundle, setBundle] = React.useState<DiagnosticsBundle | null>(null);
  const [generating, setGenerating] = React.useState(false);
  const [savePath, setSavePath] = React.useState("");
  const [saving, setSaving] = React.useState(false);
  const [saved, setSaved] = React.useState<string | null>(null);

  async function generateBundle() {
    setGenerating(true);
    setSaved(null);
    try {
      setBundle(await client.exportDiagnosticsBundle());
    } catch (error) {
      onError(getErrorMessage(error));
    } finally {
      setGenerating(false);
    }
  }

  async function saveBundle() {
    if (!bundle || !savePath.trim()) return;
    setSaving(true);
    try {
      // OPS-001: saves exactly `bundle.previewText` — the same text already shown for review
      // below, byte for byte, so what gets saved can never differ from what was reviewed.
      await client.saveDiagnosticsBundle(savePath.trim(), bundle.previewText);
      setSaved(savePath.trim());
    } catch (error) {
      onError(getErrorMessage(error));
    } finally {
      setSaving(false);
    }
  }

  return (
    <Panel className="p-4">
      <div className="mb-2 flex items-center gap-2">
        <FileText className="h-4 w-4" />
        <h2 className="text-sm font-semibold">Diagnostics bundle</h2>
      </div>
      <p className="text-sm text-muted-foreground">
        Generates a local, redacted support bundle — hardware info, managed-runtime status, and recent app log lines.
        Never includes prompts, model output, or attachment content. Nothing leaves this device unless you save and send
        the file yourself.
      </p>

      <label className="mt-3 flex items-center gap-2 text-sm">
        <input
          type="checkbox"
          checked={crashCaptureEnabled}
          onChange={(event) => onCrashCaptureEnabledChange(event.target.checked)}
          className="h-4 w-4"
        />
        Capture crash details locally (off by default; never sent anywhere automatically)
      </label>

      <div className="mt-3 grid gap-2">
        <Button onClick={() => void generateBundle()} disabled={generating} className="w-fit">
          {generating ? <Loader2 className="h-4 w-4 animate-spin" /> : <FileText className="h-4 w-4" />}
          Generate diagnostics bundle
        </Button>

        {bundle && (
          <div className="grid gap-2">
            <div className="text-sm font-medium">Review before saving</div>
            <textarea
              readOnly
              value={bundle.previewText}
              aria-label="Diagnostics bundle contents"
              className="h-48 w-full resize-y rounded-md border border-border bg-muted/30 p-2.5 font-mono text-xs"
            />
            <div className="flex flex-wrap gap-2">
              <Input
                value={savePath}
                onChange={(event) => setSavePath(event.target.value)}
                placeholder="C:\Diagnostics\ark-bundle.txt"
                className="min-w-64 flex-1"
                aria-label="Diagnostics bundle save path"
              />
              <Button onClick={() => void saveBundle()} disabled={saving || !savePath.trim()}>
                {saving ? <Loader2 className="h-4 w-4 animate-spin" /> : <Save className="h-4 w-4" />}
                Save
              </Button>
            </div>
            {saved && (
              <p role="status" className="text-sm text-emerald-600 dark:text-emerald-300">
                Saved to {saved}.
              </p>
            )}
          </div>
        )}
      </div>
    </Panel>
  );
}

function formatBytes(bytes: number): string {
  if (bytes >= 1e9) return `${(bytes / 1e9).toFixed(1)} GB`;
  if (bytes >= 1e6) return `${(bytes / 1e6).toFixed(0)} MB`;
  return `${Math.round(bytes / 1e3)} KB`;
}

function BuiltInRuntimeForm({
  status,
  onStatusChange,
  modelPath,
  onModelPathChange,
  onModelsRefresh,
  onError,
}: {
  status: BuiltInRuntimeStatus;
  onStatusChange: (status: BuiltInRuntimeStatus) => void;
  /** ARC-006: the device-scoped last-selected model path (see `App.tsx`'s `builtInModelPath`
   * state) — this component owns only the in-progress text-field draft, not the persisted
   * value itself, so a keystroke doesn't trigger a backend write on every character. */
  modelPath: string | null;
  onModelPathChange: (path: string) => void;
  onModelsRefresh: (result: { health: ProviderHealth; models: ModelInfo[]; provider: ProviderConfig }) => void;
  onError: (message: string) => void;
}) {
  const client = useArkClient();
  const [modelPathDraft, setModelPathDraft] = React.useState(modelPath ?? "");
  const [modelSource, setModelSource] = React.useState(status.modelProvenance?.source ?? "");
  const [modelLicense, setModelLicense] = React.useState(status.modelProvenance?.license ?? "");
  const [starting, setStarting] = React.useState(false);
  const [stopping, setStopping] = React.useState(false);
  const [refreshing, setRefreshing] = React.useState(false);

  async function handleStart() {
    const path = modelPathDraft.trim();
    if (!path || !modelSource.trim() || !modelLicense.trim()) {
      onError("Enter the GGUF path, model source, and model license before starting.");
      return;
    }
    onModelPathChange(path);
    setStarting(true);
    try {
      const next = await client.startBuiltInRuntime(path, modelSource.trim(), modelLicense.trim());
      onStatusChange(next);
      const result = await client.refreshModels("built_in");
      onModelsRefresh(result);
    } catch (err) {
      const startError = getErrorMessage(err);
      try {
        onStatusChange(await client.getBuiltInRuntimeStatus());
        onError(startError);
      } catch (statusError) {
        onError(`${startError} Status reconciliation also failed: ${getErrorMessage(statusError)}`);
      }
    } finally {
      setStarting(false);
    }
  }

  async function handleStop() {
    setStopping(true);
    try {
      await client.stopBuiltInRuntime();
      onStatusChange(await client.getBuiltInRuntimeStatus());
    } catch (err) {
      onError(getErrorMessage(err));
    } finally {
      setStopping(false);
    }
  }

  async function handleRefresh() {
    setRefreshing(true);
    try {
      const result = await client.refreshModels("built_in");
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
        {!status.binaryInstalled || !status.binaryVerified ? (
          <AlertCircle className="h-4 w-4 shrink-0 text-amber-500" />
        ) : status.running ? (
          <CheckCircle2 className="h-4 w-4 shrink-0 text-emerald-500" />
        ) : (
          <Circle className="h-4 w-4 shrink-0 text-muted-foreground" />
        )}
        <span className="font-medium">{status.state.replaceAll("_", " ")}</span>
        {status.running && status.port && <span className="text-muted-foreground">· port {status.port}</span>}
        {modelFileName && (
          <span className="ml-auto max-w-[220px] truncate text-xs text-muted-foreground">{modelFileName}</span>
        )}
      </div>

      {status.failure && (
        <div role="alert" className="rounded-md border border-destructive/30 bg-destructive/10 px-3 py-2.5 text-xs">
          <div className="font-medium">{status.failure.category.replaceAll("_", " ")}</div>
          <p className="mt-1 text-muted-foreground">{status.failure.message}</p>
        </div>
      )}

      {!status.binaryInstalled && (
        <div className="rounded-md border border-amber-500/30 bg-amber-500/10 px-3 py-2.5 text-xs text-foreground">
          <div className="mb-1.5 flex items-center gap-1.5 font-medium">
            <AlertCircle className="h-3.5 w-3.5 shrink-0" aria-hidden="true" />
            Runtime binary not installed
          </div>
          <p>
            Ark does not bundle the{" "}
            <code className="rounded bg-background px-1 font-mono text-[11px]">llama-server</code> binary in this build.
            Run <code className="rounded bg-background px-1 font-mono text-[11px]">scripts/setup-llama.ps1</code> (or{" "}
            <code className="rounded bg-background px-1 font-mono text-[11px]">setup-llama.sh</code> on macOS/Linux)
            from the repo root to download it, then reopen Settings.
          </p>
        </div>
      )}

      {status.binaryInstalled && !status.binaryVerified && (
        <div role="alert" className="rounded-md border border-destructive/30 bg-destructive/10 px-3 py-2.5 text-xs">
          The runtime exists but its checked-in artifact provenance or installed-file hashes did not verify. Re-run the
          setup script; Ark will not execute it.
        </div>
      )}

      <label className="grid gap-1.5 text-sm">
        Model file
        <Input
          value={modelPathDraft}
          onChange={(e) => setModelPathDraft(e.target.value)}
          placeholder="C:\Models\llama-3-8b-q4_K_M.gguf"
          disabled={status.running || starting}
        />
        <span className="text-xs text-muted-foreground">
          GGUF format only. Find models on Hugging Face (search "GGUF") or convert with{" "}
          <code className="rounded bg-muted px-1 text-[11px]">llama.cpp convert</code>.
        </span>
      </label>

      <label className="grid gap-1.5 text-sm">
        Model source
        <Input
          value={modelSource}
          onChange={(event) => setModelSource(event.target.value)}
          placeholder="https://huggingface.co/publisher/model"
          maxLength={2048}
          disabled={status.running || starting}
        />
        <span className="text-xs text-muted-foreground">
          Publisher/repository URL or another precise origin record.
        </span>
      </label>

      <label className="grid gap-1.5 text-sm">
        Model license
        <Input
          value={modelLicense}
          onChange={(event) => setModelLicense(event.target.value)}
          placeholder="Apache-2.0"
          maxLength={256}
          disabled={status.running || starting}
        />
      </label>

      {(status.runtimeProvenance || status.modelProvenance) && (
        <div className="grid gap-2 rounded-md border border-border bg-muted/40 px-3 py-2.5 text-xs">
          {status.runtimeProvenance && (
            <div>
              <div className="font-medium text-foreground">
                Runtime verified · {status.runtimeProvenance.version} · {status.runtimeProvenance.license}
              </div>
              <div className="break-all text-muted-foreground">
                Source {status.runtimeProvenance.sourceRepository} · artifact SHA-256{" "}
                {status.runtimeProvenance.artifactSha256} · verified {status.runtimeProvenance.verifiedAt}
              </div>
            </div>
          )}
          {status.modelProvenance && (
            <div>
              <div className="font-medium text-foreground">
                Model verified · {formatBytes(status.modelProvenance.sizeBytes)} · {status.modelProvenance.license}
              </div>
              <div className="break-all text-muted-foreground">
                Source {status.modelProvenance.source} · SHA-256 {status.modelProvenance.sha256} · verified{" "}
                {status.modelProvenance.verifiedAt}
              </div>
            </div>
          )}
        </div>
      )}

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
          <Button
            onClick={handleStart}
            disabled={
              starting ||
              !modelPathDraft.trim() ||
              !modelSource.trim() ||
              !modelLicense.trim() ||
              !status.binaryVerified
            }
          >
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
          llama.cpp runtime — fully offline once installed
        </div>
        <p>
          Ark can run a local <code className="rounded bg-background px-1 font-mono text-[11px]">llama-server</code>{" "}
          process for you, but the binary is not bundled with the app — it's downloaded once via the setup script (see
          above if not yet installed). After that, point it at a GGUF model file and start; it works without internet.
          For GPU acceleration, use the Ollama or Local Inference Host provider instead.
        </p>
        <p className="mt-1">
          Artifact delivery:{" "}
          <span className="font-medium">{releaseCapabilities.providers.built_in.delivery.replaceAll("_", " ")}</span>.
        </p>
      </div>
    </div>
  );
}

function cn(...classes: Array<string | undefined | false | null>): string {
  return classes.filter(Boolean).join(" ");
}
