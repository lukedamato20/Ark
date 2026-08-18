import {
  AlertCircle,
  ArrowLeft,
  Bell,
  CheckCircle2,
  Circle,
  Database,
  Download,
  FileText,
  Folder,
  Gauge,
  HardDrive,
  Info,
  Loader2,
  Moon,
  Network,
  Plus,
  RefreshCw,
  Save,
  SlidersHorizontal,
  Square,
  Sun,
  Trash2,
  Wrench,
} from "lucide-react";
import * as React from "react";
import { providerIsVisible, releaseCapabilities } from "../../config/releaseCapabilities";
import { getErrorMessage } from "../../lib/arkErrors";
import { downloadText, safeFilename } from "../../lib/download";
import { RESPONSE_STYLE_OPTIONS, TONE_OPTIONS } from "../../lib/generationPresets";
import { SUGGESTED_OLLAMA_MODELS, type SuggestedOllamaModel } from "../../lib/ollamaSuggestedModels";
import { assessHardwareFit, presentModel } from "../../lib/modelPresentation";
import { detectIsMacPlatform, formatShortcutKeys } from "../../lib/platform";
import { useBreakpoint } from "../../lib/useBreakpoint";
import { validateNumberInput } from "../../lib/numberField";
import { formatRelativeTime, isProviderHealthStale } from "../../lib/relativeTime";
import { SETTINGS_SECTIONS, type SettingsSectionId } from "../../lib/settingsSections";
import { SHORTCUTS } from "../../lib/shortcuts";
import { useArkClient } from "../../lib/useArkClient";
import type {
  AccentPalette,
  AppErrorShape,
  AuditEvent,
  BackupResult,
  BuiltInRuntimeStatus,
  CodeCommandDefinition,
  CompanionApiStatus,
  DiagnosticsBundle,
  ManagedModelDownloadProgress,
  HardwareFitEvidence,
  ManagedModelOperation,
  ManagedModelPreflight,
  ManagedModelStatus,
  ModelInfo,
  OllamaPullProgress,
  Persona,
  PersonaDeletionPreview,
  PersonaVersionSummary,
  Project,
  ProjectDeletionPreview,
  ProviderConfig,
  ProviderHealth,
  ResponseStyle,
  RestorePreview,
  SecretMetadata,
  SecretStoreStatus,
  ThemeMode,
  ToolStatus,
  Tone,
  WorkspaceImportPreview,
  WorkspaceImportResult,
  WorkspaceInfo,
  WorkspaceProtectionStatus,
} from "../../types/ark";
import { Badge } from "../../ui/badge";
import { Button } from "../../ui/button";
import { Card } from "../../ui/card";
import { Input } from "../../ui/input";
import { NumberField } from "../../ui/numberField";
import { Panel } from "../../ui/panel";
import { Select } from "../../ui/select";
import { Textarea } from "../../ui/textarea";
import { DiagnosticsPanel } from "../diagnostics/DiagnosticsPanel";

interface SettingsViewProps {
  settingsSection: SettingsSectionId;
  onSettingsSectionChange: (section: SettingsSectionId) => void;
  workspacePath: string;
  providers: ProviderConfig[];
  models: ModelInfo[];
  providerHealth: Record<string, ProviderHealth>;
  projects: Project[];
  personas: Persona[];
  applicationInstructions: string | null;
  onApplicationInstructionsChange: (instructions: string | null) => Promise<void>;
  theme: ThemeMode;
  accentPalette: AccentPalette;
  workspace?: WorkspaceInfo | null;
  builtInStatus: BuiltInRuntimeStatus;
  onBuiltInStatusChange: (status: BuiltInRuntimeStatus) => void;
  /** ARC-006: device-scoped — see docs/settings-catalog.md. */
  builtInModelPath: string | null;
  onBuiltInModelPathChange: (path: string) => void;
  managedModelDirectory: string | null;
  onManagedModelDirectoryChange: (path: string | null) => Promise<void>;
  /** OPS-001: opt-in, off by default — see `observability.rs`'s module doc. */
  crashCaptureEnabled: boolean;
  onCrashCaptureEnabledChange: (enabled: boolean) => void;
  /** CMP-006: opt-in, off by default — see `generation.rs`'s `should_notify`. */
  completionNotificationsEnabled: boolean;
  onCompletionNotificationsEnabledChange: (enabled: boolean) => void;
  /** PERF-001: opt-in, off by default — see `perf_metrics.rs`'s module doc. */
  perfMetricsEnabled: boolean;
  onPerfMetricsEnabledChange: (enabled: boolean) => void;
  onThemeChange: (theme: ThemeMode) => void;
  onAccentPaletteChange: (palette: AccentPalette) => void;
  onWorkspaceChange: (workspace: WorkspaceInfo) => void;
  onProviderSaved: (provider: ProviderConfig) => void;
  onProviderDeleted: (id: string) => void;
  onProjectSaved: (project: Project) => void;
  onProjectDeleted: (id: string) => void;
  onPersonaSaved: (persona: Persona) => void;
  onPersonaDeleted: (id: string) => void;
  /** FTR-009: centralized in the controller (sequenced/deduplicated per provider) — see
   * `useArkController.ts`'s `refreshProviderModels` doc comment. */
  onRefreshProviderModels: (providerId: string) => Promise<void>;
  onCancelProviderRefresh: (providerId: string) => Promise<void>;
  onBack: () => void;
  onError: (message: string) => void;
}

export function SettingsView({
  settingsSection,
  onSettingsSectionChange,
  workspacePath,
  providers,
  models,
  providerHealth,
  projects,
  personas,
  applicationInstructions,
  onApplicationInstructionsChange,
  theme,
  accentPalette,
  workspace,
  builtInStatus,
  onBuiltInStatusChange,
  builtInModelPath,
  onBuiltInModelPathChange,
  managedModelDirectory,
  onManagedModelDirectoryChange,
  crashCaptureEnabled,
  onCrashCaptureEnabledChange,
  completionNotificationsEnabled,
  onCompletionNotificationsEnabledChange,
  perfMetricsEnabled,
  onPerfMetricsEnabledChange,
  onThemeChange,
  onAccentPaletteChange,
  onWorkspaceChange,
  onProviderSaved,
  onProviderDeleted,
  onProjectSaved,
  onProjectDeleted,
  onPersonaSaved,
  onPersonaDeleted,
  onRefreshProviderModels,
  onCancelProviderRefresh,
  onBack,
  onError,
}: SettingsViewProps) {
  const client = useArkClient();
  const breakpoint = useBreakpoint();
  const isDesktopNav = breakpoint === "desktop";
  const visibleProviders = React.useMemo(
    () => providers.filter((candidate) => providerIsVisible(candidate.providerType)),
    [providers],
  );
  const [selectedProviderId, setSelectedProviderId] = React.useState(visibleProviders[0]?.id ?? "");
  const [providerCreateOpen, setProviderCreateOpen] = React.useState(false);
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
  // ARC-003: capability-gated (pull support), not a hardcoded providerType check — see
  // `OllamaModelsPanel`'s own doc comment on the same convention.
  const modelPullProviders = visibleProviders.filter((candidate) => candidate.capabilities.modelPull);

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

  const activeSectionMeta = SETTINGS_SECTIONS.find((section) => section.id === settingsSection) ?? SETTINGS_SECTIONS[0];

  let sectionContent: React.ReactNode;
  switch (settingsSection) {
    case "ai-behavior":
      sectionContent = (
        <>
          <ApplicationInstructionsPanel value={applicationInstructions} onChange={onApplicationInstructionsChange} />
          <ProjectsPanel
            projects={projects}
            providers={providers}
            models={models}
            onProjectSaved={onProjectSaved}
            onProjectDeleted={onProjectDeleted}
            onError={onError}
          />
          <PersonasPanel
            personas={personas}
            onPersonaSaved={onPersonaSaved}
            onPersonaDeleted={onPersonaDeleted}
            onError={onError}
          />
        </>
      );
      break;
    case "tools":
      sectionContent = (
        <>
          <ToolsPanel onError={onError} />
          <CodeCommandAllowlistPanel onError={onError} />
        </>
      );
      break;
    case "providers":
      sectionContent = (
        <Panel className="p-4">
          <div className="mb-4 flex flex-wrap items-center justify-between gap-2">
            <Button size="sm" onClick={() => setProviderCreateOpen((open) => !open)}>
              <Plus className="h-4 w-4" />
              {providerCreateOpen ? "Cancel" : "Add remote provider"}
            </Button>
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
          {providerCreateOpen && (
            <RemoteProviderCreateForm
              onCreated={(created) => {
                onProviderSaved(created);
                setSelectedProviderId(created.id);
                setProviderCreateOpen(false);
              }}
              onError={onError}
            />
          )}
          <div
            role="tabpanel"
            id={provider ? `provider-tabpanel-${provider.id}` : undefined}
            aria-labelledby={provider ? `provider-tab-${provider.id}` : undefined}
          >
            {provider?.providerType === "built_in" ? (
              <div className="grid gap-3 text-sm">
                <p className="text-muted-foreground">
                  The built-in runtime is configured through its reviewed model catalog in Settings → Models.
                </p>
                <Button variant="secondary" className="w-fit" onClick={() => onSettingsSectionChange("models")}>
                  Manage local runtime
                </Button>
              </div>
            ) : provider ? (
              <ProviderForm
                key={provider.id}
                provider={provider}
                models={providerModels}
                onProviderSaved={onProviderSaved}
                onProviderDeleted={onProviderDeleted}
                onRefreshProviderModels={onRefreshProviderModels}
                onCancelProviderRefresh={onCancelProviderRefresh}
                onError={onError}
                secretStoreStatus={secretStoreStatus}
                onSecretStoreRetry={checkSecretStore}
              />
            ) : (
              <p className="text-sm text-muted-foreground">No providers configured.</p>
            )}
          </div>
        </Panel>
      );
      break;
    case "models":
      sectionContent = (
        <>
          {visibleProviders.some((candidate) => candidate.providerType === "built_in") && (
            <Panel className="p-4">
              <div className="mb-4">
                <h3 className="font-medium">Managed local runtime</h3>
                <p className="mt-1 text-xs text-muted-foreground">
                  Download, verify, load, and remove Ark-reviewed local models without leaving the model library.
                </p>
              </div>
              <BuiltInRuntimeForm
                status={builtInStatus}
                onStatusChange={onBuiltInStatusChange}
                modelPath={builtInModelPath}
                onModelPathChange={onBuiltInModelPathChange}
                managedModelDirectory={managedModelDirectory}
                onManagedModelDirectoryChange={onManagedModelDirectoryChange}
                onRefreshProviderModels={onRefreshProviderModels}
                onError={onError}
              />
            </Panel>
          )}
          <ModelInventoryPanel
            providers={visibleProviders.filter(
              (candidate) => !candidate.capabilities.modelPull && candidate.providerType !== "built_in",
            )}
            models={models}
          />
          {modelPullProviders.length === 0 ? (
            <Panel className="p-6 text-center">
              <p className="text-sm text-muted-foreground">
                Connect Ollama to browse and pull its curated local library.
              </p>
              <Button variant="secondary" className="mt-3" onClick={() => onSettingsSectionChange("providers")}>
                Go to Providers
              </Button>
            </Panel>
          ) : (
            modelPullProviders.map((p) => (
              <OllamaModelsPanel
                key={p.id}
                provider={p}
                models={models.filter((model) => model.providerId === p.id)}
                health={providerHealth[p.id] ?? null}
                onRefreshProviderModels={onRefreshProviderModels}
                onError={onError}
              />
            ))
          )}
        </>
      );
      break;
    case "appearance":
      sectionContent = (
        <Panel className="space-y-5 p-4">
          <div>
            <h3 className="mb-2 text-sm font-medium">Theme</h3>
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
          </div>
          {releaseCapabilities.features.accentPreview && (
            <fieldset>
              <legend className="mb-2 text-sm font-medium">Accent preview</legend>
              <div className="flex flex-wrap gap-2">
                {(["blue", "violet", "teal", "amber", "graphite"] as const).map((palette) => (
                  <Button
                    key={palette}
                    variant={accentPalette === palette ? "primary" : "secondary"}
                    aria-pressed={accentPalette === palette}
                    onClick={() => onAccentPaletteChange(palette)}
                    className="capitalize"
                  >
                    <span className="h-3 w-3 rounded-full bg-primary" aria-hidden="true" />
                    {palette}
                  </Button>
                ))}
              </div>
              <p className="mt-2 text-xs text-muted-foreground">Device-only preview using audited semantic palettes.</p>
            </fieldset>
          )}
        </Panel>
      );
      break;
    case "shortcuts":
      sectionContent = <ShortcutsPanel />;
      break;
    case "storage":
      sectionContent = (
        <>
          <Panel className="p-4">
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
          <DataPortabilityPanel projects={projects} onError={onError} />
        </>
      );
      break;
    case "privacy":
      sectionContent = (
        <>
          <Panel className="p-4">
            <p className="text-sm text-muted-foreground">
              Chats stay local and use the configured provider endpoints. Cloud providers and telemetry are not enabled
              in this milestone.
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
          <CompanionApiPanel onError={onError} />
        </>
      );
      break;
    case "advanced":
      sectionContent = (
        <>
          <NotificationsPanel
            completionNotificationsEnabled={completionNotificationsEnabled}
            onCompletionNotificationsEnabledChange={onCompletionNotificationsEnabledChange}
          />
          <PerfMetricsPanel
            perfMetricsEnabled={perfMetricsEnabled}
            onPerfMetricsEnabledChange={onPerfMetricsEnabledChange}
          />
          <DiagnosticsBundlePanel
            onError={onError}
            crashCaptureEnabled={crashCaptureEnabled}
            onCrashCaptureEnabledChange={onCrashCaptureEnabledChange}
          />
          <DiagnosticsPanel provider={provider} selectedModel={provider?.defaultModelId ?? ""} onError={onError} />
        </>
      );
      break;
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

      <div className="flex min-h-0 flex-1 overflow-hidden">
        {isDesktopNav && (
          <SettingsNav active={settingsSection} onSelect={onSettingsSectionChange} orientation="vertical" />
        )}
        <div className="flex min-h-0 flex-1 flex-col overflow-hidden">
          {!isDesktopNav && (
            <SettingsNav active={settingsSection} onSelect={onSettingsSectionChange} orientation="horizontal" />
          )}
          <div
            id="settings-panel"
            role="tabpanel"
            aria-labelledby={`settings-tab-${settingsSection}`}
            className="min-h-0 flex-1 overflow-y-auto p-5"
          >
            <div className="mx-auto grid max-w-3xl gap-4">
              <div>
                <h2 className="text-base font-semibold">{activeSectionMeta.label}</h2>
                <p className="text-sm text-muted-foreground">{activeSectionMeta.description}</p>
              </div>
              {sectionContent}
            </div>
          </div>
        </div>
      </div>
    </main>
  );
}

function CodeCommandAllowlistPanel({ onError }: { onError: (message: string) => void }) {
  const client = useArkClient();
  const [definitions, setDefinitions] = React.useState<CodeCommandDefinition[]>([]);
  const [editingId, setEditingId] = React.useState<string | null>(null);
  const [label, setLabel] = React.useState("");
  const [program, setProgram] = React.useState("");
  const [argumentsText, setArgumentsText] = React.useState("");
  const [timeoutSeconds, setTimeoutSeconds] = React.useState("300");
  const [enabled, setEnabled] = React.useState(true);
  const [busy, setBusy] = React.useState(false);

  const load = React.useCallback(async () => {
    try {
      setDefinitions(await client.listCodeCommandDefinitions());
    } catch (error) {
      onError(getErrorMessage(error));
    }
  }, [client, onError]);

  React.useEffect(() => void load(), [load]);

  function edit(definition: CodeCommandDefinition) {
    setEditingId(definition.id);
    setLabel(definition.label);
    setProgram(definition.program);
    setArgumentsText(definition.arguments.join("\n"));
    setTimeoutSeconds(String(definition.timeoutSeconds));
    setEnabled(definition.enabled);
  }

  function clear() {
    setEditingId(null);
    setLabel("");
    setProgram("");
    setArgumentsText("");
    setTimeoutSeconds("300");
    setEnabled(true);
  }

  async function save() {
    const timeout = Number(timeoutSeconds);
    if (!label.trim() || !program.trim() || !Number.isInteger(timeout)) return;
    setBusy(true);
    try {
      await client.saveCodeCommandDefinition({
        id: editingId,
        label: label.trim(),
        program: program.trim(),
        arguments: argumentsText.split("\n").filter((argument) => argument.length > 0),
        timeoutSeconds: timeout,
        enabled,
      });
      clear();
      await load();
    } catch (error) {
      onError(getErrorMessage(error));
    } finally {
      setBusy(false);
    }
  }

  async function remove(id: string) {
    setBusy(true);
    try {
      await client.deleteCodeCommandDefinition(id);
      if (editingId === id) clear();
      await load();
    } catch (error) {
      onError(getErrorMessage(error));
    } finally {
      setBusy(false);
    }
  }

  return (
    <Panel className="space-y-4 p-4">
      <div>
        <h2 className="text-sm font-semibold">Ark Code verification commands</h2>
        <p className="mt-1 text-xs text-muted-foreground">
          Define exact test, build, or lint commands. Models can select only these fixed templates; every run still
          requires inline approval. Shell programs and scripts are rejected.
        </p>
      </div>
      <div className="grid gap-3 md:grid-cols-2">
        <label className="grid gap-1.5 text-sm">
          Label
          <Input
            value={label}
            maxLength={80}
            onChange={(event) => setLabel(event.target.value)}
            placeholder="Rust tests"
          />
        </label>
        <label className="grid gap-1.5 text-sm">
          Executable name
          <Input
            value={program}
            maxLength={128}
            spellCheck={false}
            onChange={(event) => setProgram(event.target.value)}
            placeholder="cargo"
          />
        </label>
      </div>
      <label className="grid gap-1.5 text-sm">
        Fixed arguments (one per line)
        <Textarea
          value={argumentsText}
          rows={3}
          spellCheck={false}
          onChange={(event) => setArgumentsText(event.target.value)}
          placeholder={"test\n--all-targets"}
        />
      </label>
      <div className="flex flex-wrap items-end gap-3">
        <label className="grid gap-1.5 text-sm">
          Timeout (seconds)
          <Input
            type="number"
            min={1}
            max={1800}
            value={timeoutSeconds}
            onChange={(event) => setTimeoutSeconds(event.target.value)}
          />
        </label>
        <label className="flex items-center gap-2 pb-2 text-sm">
          <input type="checkbox" checked={enabled} onChange={(event) => setEnabled(event.target.checked)} /> Enabled
        </label>
        <Button disabled={busy || !label.trim() || !program.trim()} onClick={() => void save()}>
          <Save className="h-4 w-4" /> {editingId ? "Update command" : "Add command"}
        </Button>
        {editingId && (
          <Button variant="ghost" onClick={clear}>
            Cancel
          </Button>
        )}
      </div>
      <div className="space-y-2">
        {definitions.map((definition) => (
          <div
            key={definition.id}
            className="flex flex-wrap items-center gap-2 rounded-md border border-border p-3 text-sm"
          >
            <button type="button" className="min-w-0 flex-1 text-left" onClick={() => edit(definition)}>
              <span className="block font-medium">{definition.label}</span>
              <code className="block truncate text-xs text-muted-foreground">
                {[definition.program, ...definition.arguments].join(" ")}
              </code>
            </button>
            <Badge tone={definition.enabled ? "success" : "muted"}>{definition.enabled ? "enabled" : "disabled"}</Badge>
            <Button
              variant="ghost"
              size="icon"
              aria-label={`Delete ${definition.label}`}
              disabled={busy}
              onClick={() => void remove(definition.id)}
            >
              <Trash2 className="h-4 w-4" />
            </Button>
          </div>
        ))}
        {definitions.length === 0 && (
          <p className="text-xs text-muted-foreground">No verification commands configured.</p>
        )}
      </div>
    </Panel>
  );
}

const MIN_PROJECT_TEMPERATURE = 0;
const MAX_PROJECT_TEMPERATURE = 2;
const MIN_PROJECT_MAX_TOKENS = 1;
const MAX_PROJECT_MAX_TOKENS = 1_000_000;
const MAX_APPLICATION_INSTRUCTIONS_CHARS = 32_000;

function ApplicationInstructionsPanel({
  value,
  onChange,
}: {
  value: string | null;
  onChange: (instructions: string | null) => Promise<void>;
}) {
  const [draft, setDraft] = React.useState(value ?? "");
  const [saving, setSaving] = React.useState(false);

  React.useEffect(() => setDraft(value ?? ""), [value]);

  const normalized = draft.trim() || null;
  const dirty = normalized !== value;

  async function save() {
    setSaving(true);
    try {
      await onChange(normalized);
    } finally {
      setSaving(false);
    }
  }

  return (
    <Panel className="grid gap-3 p-4">
      <div>
        <h3 className="text-sm font-semibold">Application instructions</h3>
        <p className="mt-1 text-xs text-muted-foreground">
          Portable fallback instructions for every conversation in this workspace. Leave empty for no workspace-wide
          instruction.
        </p>
      </div>
      <label className="grid gap-1.5 text-sm">
        Workspace-wide fallback
        <Textarea
          value={draft}
          onChange={(event) => setDraft(event.target.value)}
          rows={4}
          maxLength={MAX_APPLICATION_INSTRUCTIONS_CHARS}
          placeholder="No application instructions"
        />
      </label>
      <div className="flex flex-wrap items-center justify-between gap-2">
        <span className="text-xs text-muted-foreground">
          {draft.length.toLocaleString()} / {MAX_APPLICATION_INSTRUCTIONS_CHARS.toLocaleString()} characters
        </span>
        <Button disabled={!dirty || saving} onClick={() => void save()}>
          {saving ? <Loader2 className="h-4 w-4 animate-spin" /> : <Save className="h-4 w-4" />}
          Save instructions
        </Button>
      </div>
      <div className="rounded-md border border-border bg-muted/30 p-3 text-xs text-muted-foreground">
        <div className="font-medium text-foreground">Instruction precedence, lowest to highest</div>
        <div className="mt-1">Application → Project → Persona → Conversation → User request</div>
        <p className="mt-1">
          The most specific configured instruction source wins. The user request remains a separate intent channel;
          retrieved files and tool results remain separate untrusted context and can never approve tools.
        </p>
      </div>
    </Panel>
  );
}

/**
 * UX: the Settings navigation itself — the single place that renders `SETTINGS_SECTIONS`, so a
 * category can never appear in one layout (desktop/narrow) without the other. `orientation`
 * switches between a docked left column (desktop) and a horizontal scrollable strip (narrower
 * widths, via `useBreakpoint` in the parent) — the same `role="tablist"`/`role="tab"` semantics
 * either way, just a different `aria-orientation` and layout direction.
 */
export function SettingsNav({
  active,
  onSelect,
  orientation,
}: {
  active: SettingsSectionId;
  onSelect: (section: SettingsSectionId) => void;
  orientation: "vertical" | "horizontal";
}) {
  const handleKeyDown = (event: React.KeyboardEvent<HTMLElement>) => {
    const tabs = Array.from(event.currentTarget.querySelectorAll<HTMLButtonElement>('[role="tab"]'));
    const currentIndex = tabs.indexOf(event.target as HTMLButtonElement);
    if (currentIndex < 0) return;

    let nextIndex: number | null = null;
    if (event.key === "Home") nextIndex = 0;
    else if (event.key === "End") nextIndex = tabs.length - 1;
    else if (
      (orientation === "horizontal" && event.key === "ArrowRight") ||
      (orientation === "vertical" && event.key === "ArrowDown")
    ) {
      nextIndex = (currentIndex + 1) % tabs.length;
    } else if (
      (orientation === "horizontal" && event.key === "ArrowLeft") ||
      (orientation === "vertical" && event.key === "ArrowUp")
    ) {
      nextIndex = (currentIndex - 1 + tabs.length) % tabs.length;
    }

    if (nextIndex === null) return;
    event.preventDefault();
    const nextTab = tabs[nextIndex];
    nextTab.focus();
    nextTab.click();
  };

  return (
    <nav
      role="tablist"
      aria-label="Settings sections"
      aria-orientation={orientation}
      onKeyDown={handleKeyDown}
      className={cn(
        "shrink-0 gap-0.5",
        orientation === "vertical"
          ? "flex w-56 flex-col overflow-y-auto border-r border-border p-3"
          : "flex gap-1 overflow-x-auto border-b border-border px-3 py-2",
      )}
    >
      {SETTINGS_SECTIONS.map((section) => (
        <button
          key={section.id}
          type="button"
          role="tab"
          id={`settings-tab-${section.id}`}
          aria-selected={section.id === active}
          aria-controls="settings-panel"
          tabIndex={section.id === active ? 0 : -1}
          onClick={() => onSelect(section.id)}
          className={cn(
            "shrink-0 whitespace-nowrap rounded-md px-3 text-left text-sm font-medium transition-colors",
            orientation === "vertical" ? "py-2" : "py-1.5",
            section.id === active
              ? "bg-accent text-accent-foreground"
              : "text-muted-foreground hover:bg-accent/50 hover:text-foreground",
          )}
        >
          {section.label}
        </button>
      ))}
    </nav>
  );
}

/** UX: Settings' Keyboard Shortcuts section — renders `SHORTCUTS` (`src/lib/shortcuts.ts`), the
 * same registry `useArkController.ts`'s global keydown handler matches against and
 * `ShortcutsDialog` (`Shift+?`) also renders, so this list can never drift from what actually
 * fires. */
function ShortcutsPanel() {
  const isMac = React.useMemo(() => detectIsMacPlatform(navigator.userAgent), []);
  return (
    <Panel className="p-4">
      <dl className="grid gap-2">
        {SHORTCUTS.map((shortcut) => (
          <div key={shortcut.id} className="flex items-center justify-between gap-3 text-sm">
            <dt className="text-muted-foreground">{shortcut.description}</dt>
            <dd>
              <kbd className="rounded border border-border bg-muted/60 px-1.5 py-0.5 font-mono text-xs">
                {formatShortcutKeys(shortcut.keys, isMac)}
              </kbd>
            </dd>
          </div>
        ))}
      </dl>
    </Panel>
  );
}

/**
 * FTR-003: a project groups conversations under a shared name, instructions, and default
 * provider/model/temperature/max_tokens. This panel is the only UI surface for project CRUD —
 * projects themselves are picked/assigned per-conversation from `ChatView`'s
 * `ConversationSettingsButton`, which reads this same list from the bootstrap/controller store.
 */
function ProjectsPanel({
  projects,
  providers,
  models,
  onProjectSaved,
  onProjectDeleted,
  onError,
}: {
  projects: Project[];
  providers: ProviderConfig[];
  models: ModelInfo[];
  onProjectSaved: (project: Project) => void;
  onProjectDeleted: (id: string) => void;
  onError: (message: string) => void;
}) {
  const client = useArkClient();
  const [selectedId, setSelectedId] = React.useState<string | null>(null);
  const [newName, setNewName] = React.useState("");
  const [creating, setCreating] = React.useState(false);
  const [showArchived, setShowArchived] = React.useState(false);

  const visibleProjects = projects
    .filter((project) => showArchived || !project.archivedAt)
    .slice()
    .sort((a, b) => a.name.localeCompare(b.name));
  const selected = projects.find((project) => project.id === selectedId) ?? null;

  async function createProject() {
    const trimmed = newName.trim();
    if (!trimmed) return;
    setCreating(true);
    try {
      const project = await client.createProject(trimmed);
      onProjectSaved(project);
      setNewName("");
      setSelectedId(project.id);
    } catch (error) {
      onError(getErrorMessage(error));
    } finally {
      setCreating(false);
    }
  }

  return (
    <Panel className="p-4">
      <div className="mb-4 flex items-center justify-between gap-2">
        <div className="flex items-center gap-2">
          <Folder className="h-4 w-4" />
          <h2 className="text-sm font-semibold">Projects</h2>
        </div>
        <label className="flex items-center gap-1.5 text-xs text-muted-foreground">
          <input
            type="checkbox"
            checked={showArchived}
            onChange={(event) => setShowArchived(event.target.checked)}
            className="h-3.5 w-3.5"
          />
          Show archived
        </label>
      </div>

      <div className="mb-3 flex gap-2">
        <Input
          value={newName}
          onChange={(event) => setNewName(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === "Enter") void createProject();
          }}
          placeholder="New project name"
          maxLength={200}
        />
        <Button variant="secondary" disabled={creating || !newName.trim()} onClick={() => void createProject()}>
          {creating ? <Loader2 className="h-4 w-4 animate-spin" /> : <Plus className="h-4 w-4" />}
          Create
        </Button>
      </div>

      {visibleProjects.length === 0 ? (
        <p className="text-sm text-muted-foreground">No projects yet.</p>
      ) : (
        <div className="grid gap-1">
          {visibleProjects.map((project) => (
            <button
              key={project.id}
              type="button"
              onClick={() => setSelectedId(project.id === selectedId ? null : project.id)}
              aria-expanded={project.id === selectedId}
              className={cn(
                "flex items-center justify-between rounded-md border border-transparent px-2 py-1.5 text-left text-sm outline-none hover:bg-muted focus-visible:ring-2 focus-visible:ring-ring",
                project.id === selectedId && "border-border bg-muted",
              )}
            >
              <span className="truncate">{project.name}</span>
              {project.archivedAt && <Badge tone="warning">Archived</Badge>}
            </button>
          ))}
        </div>
      )}

      {selected && (
        <ProjectEditor
          key={selected.id}
          project={selected}
          providers={providers}
          models={models}
          onProjectSaved={onProjectSaved}
          onProjectDeleted={(id) => {
            onProjectDeleted(id);
            setSelectedId(null);
          }}
          onError={onError}
        />
      )}
    </Panel>
  );
}

function ProjectEditor({
  project,
  providers,
  models,
  onProjectSaved,
  onProjectDeleted,
  onError,
}: {
  project: Project;
  providers: ProviderConfig[];
  models: ModelInfo[];
  onProjectSaved: (project: Project) => void;
  onProjectDeleted: (id: string) => void;
  onError: (message: string) => void;
}) {
  const client = useArkClient();
  const [name, setName] = React.useState(project.name);
  const [repositoryPath, setRepositoryPath] = React.useState(project.repositoryPath ?? "");
  const [instructions, setInstructions] = React.useState(project.instructions ?? "");
  const [defaultProviderId, setDefaultProviderId] = React.useState(project.defaultProviderId ?? "");
  const [defaultModelId, setDefaultModelId] = React.useState(project.defaultModelId ?? "");
  const [temperature, setTemperature] = React.useState(
    project.defaultTemperature != null ? String(project.defaultTemperature) : "",
  );
  const [maxTokens, setMaxTokens] = React.useState(
    project.defaultMaxTokens != null ? String(project.defaultMaxTokens) : "",
  );
  const [responseStyle, setResponseStyle] = React.useState(project.responseStyle ?? "");
  const [tone, setTone] = React.useState(project.tone ?? "");
  const [saving, setSaving] = React.useState(false);
  const [savingRepository, setSavingRepository] = React.useState(false);
  const [archiving, setArchiving] = React.useState(false);
  const [deletePreview, setDeletePreview] = React.useState<ProjectDeletionPreview | null>(null);
  const [deleting, setDeleting] = React.useState(false);

  React.useEffect(() => {
    setName(project.name);
    setRepositoryPath(project.repositoryPath ?? "");
    setInstructions(project.instructions ?? "");
    setDefaultProviderId(project.defaultProviderId ?? "");
    setDefaultModelId(project.defaultModelId ?? "");
    setTemperature(project.defaultTemperature != null ? String(project.defaultTemperature) : "");
    setMaxTokens(project.defaultMaxTokens != null ? String(project.defaultMaxTokens) : "");
    setResponseStyle(project.responseStyle ?? "");
    setTone(project.tone ?? "");
    setDeletePreview(null);
  }, [project]);

  // FTR-003: empty is a valid, meaningful state here ("no project default, fall through to the
  // provider's") — unlike `ProviderForm`'s temperature/max-tokens, which are always-required
  // concrete values. `validateNumberInput`/`NumberField` are built for the latter case and treat
  // empty as an error, so this mirrors `ConversationSettingsButton`'s own inline validation
  // (the same "optional numeric override" shape) instead of reusing them.
  const temperatureTrimmed = temperature.trim();
  const maxTokensTrimmed = maxTokens.trim();
  const temperatureNumber = temperatureTrimmed === "" ? null : Number(temperatureTrimmed);
  const maxTokensNumber = maxTokensTrimmed === "" ? null : Number(maxTokensTrimmed);
  const temperatureValid =
    temperatureTrimmed === "" ||
    (temperatureNumber !== null &&
      Number.isFinite(temperatureNumber) &&
      temperatureNumber >= MIN_PROJECT_TEMPERATURE &&
      temperatureNumber <= MAX_PROJECT_TEMPERATURE);
  const maxTokensValid =
    maxTokensTrimmed === "" ||
    (maxTokensNumber !== null &&
      Number.isInteger(maxTokensNumber) &&
      maxTokensNumber >= MIN_PROJECT_MAX_TOKENS &&
      maxTokensNumber <= MAX_PROJECT_MAX_TOKENS);
  const projectModels = models.filter((model) => model.providerId === defaultProviderId);

  async function save() {
    if (!name.trim() || !temperatureValid || !maxTokensValid) return;
    setSaving(true);
    try {
      const saved = await client.updateProject({
        id: project.id,
        name,
        instructions: instructions.trim() || null,
        defaultProviderId: defaultProviderId || null,
        defaultModelId: defaultModelId || null,
        defaultTemperature: temperatureNumber,
        defaultMaxTokens: maxTokensNumber,
        responseStyle: (responseStyle || null) as ResponseStyle | null,
        tone: (tone || null) as Tone | null,
      });
      onProjectSaved(saved);
    } catch (error) {
      onError(getErrorMessage(error));
    } finally {
      setSaving(false);
    }
  }

  async function toggleArchived() {
    setArchiving(true);
    try {
      const saved = await client.setProjectArchived(project.id, !project.archivedAt);
      onProjectSaved(saved);
    } catch (error) {
      onError(getErrorMessage(error));
    } finally {
      setArchiving(false);
    }
  }

  async function saveRepository(nextPath: string | null) {
    setSavingRepository(true);
    try {
      const saved = await client.setProjectRepository(project.id, nextPath);
      setRepositoryPath(saved.repositoryPath ?? "");
      onProjectSaved(saved);
    } catch (error) {
      onError(getErrorMessage(error));
    } finally {
      setSavingRepository(false);
    }
  }

  async function loadDeletePreview() {
    try {
      setDeletePreview(await client.previewProjectDeletion(project.id));
    } catch (error) {
      onError(getErrorMessage(error));
    }
  }

  async function confirmDelete() {
    setDeleting(true);
    try {
      await client.deleteProject(project.id);
      onProjectDeleted(project.id);
    } catch (error) {
      onError(getErrorMessage(error));
      setDeleting(false);
    }
  }

  return (
    <div className="mt-3 grid gap-3 rounded-md border border-border p-3">
      <label className="grid gap-1.5 text-sm">
        Name
        <Input value={name} onChange={(event) => setName(event.target.value)} maxLength={200} />
      </label>
      <div className="grid gap-1.5 rounded-md border border-border bg-muted/30 p-3">
        <label className="grid gap-1.5 text-sm">
          Repository (Ark Code)
          <Input
            value={repositoryPath}
            onChange={(event) => setRepositoryPath(event.target.value)}
            placeholder="Absolute path to an existing code repository"
            spellCheck={false}
          />
        </label>
        <p className="text-xs text-muted-foreground">
          This is the codebase Ark Code may access. It is separate from Ark&apos;s storage Workspace, which contains app
          data. Binding, switching, or removing it takes effect immediately and never moves Workspace data.
        </p>
        <div className="flex flex-wrap gap-2">
          <Button
            type="button"
            variant="secondary"
            size="sm"
            disabled={
              savingRepository || !repositoryPath.trim() || repositoryPath.trim() === (project.repositoryPath ?? "")
            }
            onClick={() => void saveRepository(repositoryPath.trim())}
          >
            {savingRepository ? <Loader2 className="h-4 w-4 animate-spin" /> : null}
            {project.repositoryPath ? "Switch Repository" : "Bind Repository"}
          </Button>
          {project.repositoryPath ? (
            <Button
              type="button"
              variant="ghost"
              size="sm"
              disabled={savingRepository}
              onClick={() => void saveRepository(null)}
            >
              Remove Repository
            </Button>
          ) : null}
        </div>
      </div>
      <label className="grid gap-1.5 text-sm">
        Instructions
        <Textarea
          value={instructions}
          onChange={(event) => setInstructions(event.target.value)}
          rows={3}
          placeholder="No project instructions — every conversation in this project inherits its own default"
        />
      </label>
      <div className="grid grid-cols-2 gap-3">
        <label className="grid gap-1.5 text-sm">
          Default response style
          <Select value={responseStyle} onChange={(event) => setResponseStyle(event.target.value)}>
            <option value="">Provider default (none)</option>
            {RESPONSE_STYLE_OPTIONS.map((option) => (
              <option key={option.value} value={option.value}>
                {option.label}
              </option>
            ))}
          </Select>
        </label>
        <label className="grid gap-1.5 text-sm">
          Default tone
          <Select value={tone} onChange={(event) => setTone(event.target.value)}>
            <option value="">Provider default (none)</option>
            {TONE_OPTIONS.map((option) => (
              <option key={option.value} value={option.value}>
                {option.label}
              </option>
            ))}
          </Select>
        </label>
      </div>
      <label className="grid gap-1.5 text-sm">
        Default provider
        <Select
          value={defaultProviderId}
          onChange={(event) => {
            setDefaultProviderId(event.target.value);
            setDefaultModelId("");
          }}
        >
          <option value="">Provider default (none)</option>
          {providers.map((candidate) => (
            <option key={candidate.id} value={candidate.id}>
              {candidate.name}
            </option>
          ))}
        </Select>
      </label>
      {defaultProviderId && (
        <label className="grid gap-1.5 text-sm">
          Default model
          <Select value={defaultModelId} onChange={(event) => setDefaultModelId(event.target.value)}>
            <option value="">No default model</option>
            {projectModels.map((candidate) => (
              <option key={candidate.id} value={candidate.name}>
                {candidate.displayName ?? candidate.name}
              </option>
            ))}
          </Select>
        </label>
      )}
      <label className="grid gap-1.5 text-sm">
        Default temperature
        <Input
          value={temperature}
          onChange={(event) => setTemperature(event.target.value)}
          inputMode="decimal"
          placeholder="Provider default"
          aria-invalid={!temperatureValid}
          className={cn(!temperatureValid && "border-destructive focus-visible:ring-destructive")}
        />
        {!temperatureValid ? (
          <span role="alert" className="text-xs text-destructive">
            Must be between {MIN_PROJECT_TEMPERATURE} and {MAX_PROJECT_TEMPERATURE}, or empty to use the provider
            default.
          </span>
        ) : (
          <span className="text-xs text-muted-foreground">Falls through to the provider's default when empty.</span>
        )}
      </label>
      <label className="grid gap-1.5 text-sm">
        Default max tokens
        <Input
          value={maxTokens}
          onChange={(event) => setMaxTokens(event.target.value)}
          inputMode="numeric"
          placeholder="Provider default"
          aria-invalid={!maxTokensValid}
          className={cn(!maxTokensValid && "border-destructive focus-visible:ring-destructive")}
        />
        {!maxTokensValid ? (
          <span role="alert" className="text-xs text-destructive">
            Must be a whole number between {MIN_PROJECT_MAX_TOKENS} and {MAX_PROJECT_MAX_TOKENS}, or empty to use the
            provider default.
          </span>
        ) : (
          <span className="text-xs text-muted-foreground">Falls through to the provider's default when empty.</span>
        )}
      </label>
      <div className="flex flex-wrap items-center justify-between gap-2">
        <div className="flex flex-wrap items-center gap-2">
          <Button type="button" variant="secondary" onClick={() => void toggleArchived()} disabled={archiving}>
            {project.archivedAt ? "Unarchive" : "Archive"}
          </Button>
          {deletePreview ? (
            <div className="flex items-center gap-2 text-xs text-muted-foreground">
              <span>
                {deletePreview.conversationCount === 0
                  ? "No conversations are assigned."
                  : `${deletePreview.conversationCount} conversation(s) will be unassigned, not deleted.`}
                {deletePreview.attachmentCount > 0 &&
                  ` ${deletePreview.attachmentCount} attached file(s) will remain with those conversations.`}
              </span>
              <Button
                type="button"
                variant="destructive"
                size="sm"
                onClick={() => void confirmDelete()}
                disabled={deleting}
              >
                {deleting ? <Loader2 className="h-4 w-4 animate-spin" /> : null}
                Confirm delete
              </Button>
              <Button type="button" variant="ghost" size="sm" onClick={() => setDeletePreview(null)}>
                Cancel
              </Button>
            </div>
          ) : (
            <Button type="button" variant="ghost" onClick={() => void loadDeletePreview()}>
              <Trash2 className="h-4 w-4" />
              Delete
            </Button>
          )}
        </div>
        <Button
          type="button"
          onClick={() => void save()}
          disabled={saving || !name.trim() || !temperatureValid || !maxTokensValid}
        >
          {saving ? <Loader2 className="h-4 w-4 animate-spin" /> : <Save className="h-4 w-4" />}
          Save
        </Button>
      </div>
    </div>
  );
}

/**
 * FTR-003: a persona is a reusable, named instruction identity a conversation can be assigned
 * to — independent of `ProjectsPanel` above (a project groups conversations by subject, a
 * persona defines how the assistant behaves; a conversation can have both at once). Mirrors
 * `ProjectsPanel`'s structure exactly, with two real differences: instructions are required (a
 * persona's entire purpose is its prompt) rather than optional, and editing them is versioned —
 * see `PersonaEditor`'s version history.
 */
const MAX_PERSONA_IMPORT_FILE_BYTES = 5 * 1024 * 1024;

function PersonasPanel({
  personas,
  onPersonaSaved,
  onPersonaDeleted,
  onError,
}: {
  personas: Persona[];
  onPersonaSaved: (persona: Persona) => void;
  onPersonaDeleted: (id: string) => void;
  onError: (message: string) => void;
}) {
  const client = useArkClient();
  const fileInputRef = React.useRef<HTMLInputElement | null>(null);
  const [selectedId, setSelectedId] = React.useState<string | null>(null);
  const [newName, setNewName] = React.useState("");
  const [newInstructions, setNewInstructions] = React.useState("");
  const [creating, setCreating] = React.useState(false);
  const [importing, setImporting] = React.useState(false);
  const [showArchived, setShowArchived] = React.useState(false);

  const visiblePersonas = personas
    .filter((persona) => showArchived || !persona.archivedAt)
    .slice()
    .sort((a, b) => a.name.localeCompare(b.name));
  const selected = personas.find((persona) => persona.id === selectedId) ?? null;

  async function createPersona() {
    const trimmedName = newName.trim();
    const trimmedInstructions = newInstructions.trim();
    if (!trimmedName || !trimmedInstructions) return;
    setCreating(true);
    try {
      const persona = await client.createPersona({ name: trimmedName, instructions: trimmedInstructions });
      onPersonaSaved(persona);
      setNewName("");
      setNewInstructions("");
      setSelectedId(persona.id);
    } catch (error) {
      onError(getErrorMessage(error));
    } finally {
      setCreating(false);
    }
  }

  async function importPersona(event: React.ChangeEvent<HTMLInputElement>) {
    const file = event.target.files?.[0];
    event.target.value = "";
    if (!file) return;
    if (file.size > MAX_PERSONA_IMPORT_FILE_BYTES) {
      onError(`"${file.name}" exceeds the ${MAX_PERSONA_IMPORT_FILE_BYTES / (1024 * 1024)} MB persona import limit.`);
      return;
    }
    setImporting(true);
    try {
      const persona = await client.importPersonaJson(await file.text());
      onPersonaSaved(persona);
      setSelectedId(persona.id);
    } catch (error) {
      onError(getErrorMessage(error));
    } finally {
      setImporting(false);
    }
  }

  return (
    <Panel className="p-4">
      <div className="mb-4 flex items-center justify-between gap-2">
        <div className="flex items-center gap-2">
          <SlidersHorizontal className="h-4 w-4" />
          <h2 className="text-sm font-semibold">Personas</h2>
        </div>
        <div className="flex flex-wrap items-center justify-end gap-2">
          <Button
            type="button"
            variant="ghost"
            size="sm"
            onClick={() => fileInputRef.current?.click()}
            disabled={importing}
          >
            {importing ? <Loader2 className="h-3.5 w-3.5 animate-spin" /> : null}
            Import
          </Button>
          <input
            ref={fileInputRef}
            type="file"
            accept="application/json,.json"
            className="hidden"
            onChange={importPersona}
          />
          <label className="flex items-center gap-1.5 text-xs text-muted-foreground">
            <input
              type="checkbox"
              checked={showArchived}
              onChange={(event) => setShowArchived(event.target.checked)}
              className="h-3.5 w-3.5"
            />
            Show archived
          </label>
        </div>
      </div>
      <p className="mb-3 text-sm text-muted-foreground">
        A reusable instruction identity a conversation can be assigned to, independent of its project.
      </p>

      <div className="mb-3 grid gap-2 rounded-md border border-border p-3">
        <Input
          value={newName}
          onChange={(event) => setNewName(event.target.value)}
          placeholder="New persona name"
          maxLength={200}
        />
        <Textarea
          value={newInstructions}
          onChange={(event) => setNewInstructions(event.target.value)}
          placeholder="Instructions (required) — e.g. 'Be terse and cite line numbers.'"
          rows={2}
        />
        <Button
          variant="secondary"
          disabled={creating || !newName.trim() || !newInstructions.trim()}
          onClick={() => void createPersona()}
          className="w-fit"
        >
          {creating ? <Loader2 className="h-4 w-4 animate-spin" /> : <Plus className="h-4 w-4" />}
          Create
        </Button>
      </div>

      {visiblePersonas.length === 0 ? (
        <p className="text-sm text-muted-foreground">No personas yet.</p>
      ) : (
        <div className="grid gap-1">
          {visiblePersonas.map((persona) => (
            <button
              key={persona.id}
              type="button"
              onClick={() => setSelectedId(persona.id === selectedId ? null : persona.id)}
              aria-expanded={persona.id === selectedId}
              className={cn(
                "flex items-center justify-between rounded-md border border-transparent px-2 py-1.5 text-left text-sm outline-none hover:bg-muted focus-visible:ring-2 focus-visible:ring-ring",
                persona.id === selectedId && "border-border bg-muted",
              )}
            >
              <span className="truncate">{persona.name}</span>
              <span className="flex items-center gap-1.5 shrink-0">
                <span className="text-xs text-muted-foreground">v{persona.versionNumber}</span>
                {persona.archivedAt && <Badge tone="warning">Archived</Badge>}
              </span>
            </button>
          ))}
        </div>
      )}

      {selected && (
        <PersonaEditor
          key={selected.id}
          persona={selected}
          onPersonaSaved={onPersonaSaved}
          onPersonaDeleted={(id) => {
            onPersonaDeleted(id);
            setSelectedId(null);
          }}
          onError={onError}
        />
      )}
    </Panel>
  );
}

function PersonaEditor({
  persona,
  onPersonaSaved,
  onPersonaDeleted,
  onError,
}: {
  persona: Persona;
  onPersonaSaved: (persona: Persona) => void;
  onPersonaDeleted: (id: string) => void;
  onError: (message: string) => void;
}) {
  const client = useArkClient();
  const [name, setName] = React.useState(persona.name);
  const [instructions, setInstructions] = React.useState(persona.instructions);
  const [temperature, setTemperature] = React.useState(
    persona.defaultTemperature != null ? String(persona.defaultTemperature) : "",
  );
  const [maxTokens, setMaxTokens] = React.useState(
    persona.defaultMaxTokens != null ? String(persona.defaultMaxTokens) : "",
  );
  const [responseStyle, setResponseStyle] = React.useState(persona.responseStyle ?? "");
  const [tone, setTone] = React.useState(persona.tone ?? "");
  const [saving, setSaving] = React.useState(false);
  const [archiving, setArchiving] = React.useState(false);
  const [deletePreview, setDeletePreview] = React.useState<PersonaDeletionPreview | null>(null);
  const [deleting, setDeleting] = React.useState(false);
  const [versions, setVersions] = React.useState<PersonaVersionSummary[] | null>(null);
  const [versionsLoading, setVersionsLoading] = React.useState(false);
  const [exporting, setExporting] = React.useState(false);

  React.useEffect(() => {
    setName(persona.name);
    setInstructions(persona.instructions);
    setTemperature(persona.defaultTemperature != null ? String(persona.defaultTemperature) : "");
    setMaxTokens(persona.defaultMaxTokens != null ? String(persona.defaultMaxTokens) : "");
    setResponseStyle(persona.responseStyle ?? "");
    setTone(persona.tone ?? "");
    setDeletePreview(null);
    setVersions(null);
  }, [persona]);

  // FTR-003: mirrors `ProjectEditor`'s bespoke empty-is-valid inline validation for the same
  // "optional numeric override" shape — see that component's comment for why `NumberField` isn't
  // reused here.
  const temperatureTrimmed = temperature.trim();
  const maxTokensTrimmed = maxTokens.trim();
  const temperatureNumber = temperatureTrimmed === "" ? null : Number(temperatureTrimmed);
  const maxTokensNumber = maxTokensTrimmed === "" ? null : Number(maxTokensTrimmed);
  const temperatureValid =
    temperatureTrimmed === "" ||
    (temperatureNumber !== null &&
      Number.isFinite(temperatureNumber) &&
      temperatureNumber >= MIN_PROJECT_TEMPERATURE &&
      temperatureNumber <= MAX_PROJECT_TEMPERATURE);
  const maxTokensValid =
    maxTokensTrimmed === "" ||
    (maxTokensNumber !== null &&
      Number.isInteger(maxTokensNumber) &&
      maxTokensNumber >= MIN_PROJECT_MAX_TOKENS &&
      maxTokensNumber <= MAX_PROJECT_MAX_TOKENS);

  async function save() {
    if (!name.trim() || !instructions.trim() || !temperatureValid || !maxTokensValid) return;
    setSaving(true);
    try {
      const saved = await client.updatePersona({
        id: persona.id,
        name,
        instructions,
        defaultTemperature: temperatureNumber,
        defaultMaxTokens: maxTokensNumber,
        responseStyle: (responseStyle || null) as ResponseStyle | null,
        tone: (tone || null) as Tone | null,
      });
      onPersonaSaved(saved);
      setVersions(null);
    } catch (error) {
      onError(getErrorMessage(error));
    } finally {
      setSaving(false);
    }
  }

  async function toggleArchived() {
    setArchiving(true);
    try {
      const saved = await client.setPersonaArchived(persona.id, !persona.archivedAt);
      onPersonaSaved(saved);
    } catch (error) {
      onError(getErrorMessage(error));
    } finally {
      setArchiving(false);
    }
  }

  async function loadDeletePreview() {
    try {
      setDeletePreview(await client.previewPersonaDeletion(persona.id));
    } catch (error) {
      onError(getErrorMessage(error));
    }
  }

  async function confirmDelete() {
    setDeleting(true);
    try {
      await client.deletePersona(persona.id);
      onPersonaDeleted(persona.id);
    } catch (error) {
      onError(getErrorMessage(error));
      setDeleting(false);
    }
  }

  async function toggleVersionHistory() {
    if (versions) {
      setVersions(null);
      return;
    }
    setVersionsLoading(true);
    try {
      setVersions(await client.listPersonaVersions(persona.id));
    } catch (error) {
      onError(getErrorMessage(error));
    } finally {
      setVersionsLoading(false);
    }
  }

  async function exportPersona() {
    setExporting(true);
    try {
      const json = await client.exportPersonaJson(persona.id);
      downloadText(`ark-persona-${safeFilename(persona.name)}.json`, json, "application/json;charset=utf-8");
    } catch (error) {
      onError(getErrorMessage(error));
    } finally {
      setExporting(false);
    }
  }

  return (
    <div className="mt-3 grid gap-3 rounded-md border border-border p-3">
      <label className="grid gap-1.5 text-sm">
        Name
        <Input value={name} onChange={(event) => setName(event.target.value)} maxLength={200} />
      </label>
      <label className="grid gap-1.5 text-sm">
        Instructions
        <Textarea value={instructions} onChange={(event) => setInstructions(event.target.value)} rows={4} />
        {!instructions.trim() && (
          <span role="alert" className="text-xs text-destructive">
            Instructions cannot be empty.
          </span>
        )}
      </label>
      <div className="grid grid-cols-2 gap-3">
        <label className="grid gap-1.5 text-sm">
          Default response style
          <Select value={responseStyle} onChange={(event) => setResponseStyle(event.target.value)}>
            <option value="">Provider default (none)</option>
            {RESPONSE_STYLE_OPTIONS.map((option) => (
              <option key={option.value} value={option.value}>
                {option.label}
              </option>
            ))}
          </Select>
        </label>
        <label className="grid gap-1.5 text-sm">
          Default tone
          <Select value={tone} onChange={(event) => setTone(event.target.value)}>
            <option value="">Provider default (none)</option>
            {TONE_OPTIONS.map((option) => (
              <option key={option.value} value={option.value}>
                {option.label}
              </option>
            ))}
          </Select>
        </label>
      </div>
      <label className="grid gap-1.5 text-sm">
        Default temperature
        <Input
          value={temperature}
          onChange={(event) => setTemperature(event.target.value)}
          inputMode="decimal"
          placeholder="Provider default"
          aria-invalid={!temperatureValid}
          className={cn(!temperatureValid && "border-destructive focus-visible:ring-destructive")}
        />
        {!temperatureValid && (
          <span role="alert" className="text-xs text-destructive">
            Must be between {MIN_PROJECT_TEMPERATURE} and {MAX_PROJECT_TEMPERATURE}, or empty.
          </span>
        )}
      </label>
      <label className="grid gap-1.5 text-sm">
        Default max tokens
        <Input
          value={maxTokens}
          onChange={(event) => setMaxTokens(event.target.value)}
          inputMode="numeric"
          placeholder="Provider default"
          aria-invalid={!maxTokensValid}
          className={cn(!maxTokensValid && "border-destructive focus-visible:ring-destructive")}
        />
        {!maxTokensValid && (
          <span role="alert" className="text-xs text-destructive">
            Must be a whole number between {MIN_PROJECT_MAX_TOKENS} and {MAX_PROJECT_MAX_TOKENS}, or empty.
          </span>
        )}
      </label>

      <div>
        <div className="flex flex-wrap items-center gap-1">
          <Button type="button" variant="ghost" size="sm" onClick={() => void toggleVersionHistory()}>
            {versionsLoading ? <Loader2 className="h-3.5 w-3.5 animate-spin" /> : null}
            {versions ? "Hide version history" : "Show version history"}
          </Button>
          <Button type="button" variant="ghost" size="sm" onClick={() => void exportPersona()} disabled={exporting}>
            {exporting ? <Loader2 className="h-3.5 w-3.5 animate-spin" /> : <Download className="h-3.5 w-3.5" />}
            Export
          </Button>
        </div>
        {versions && (
          <ul className="mt-1 grid gap-1 rounded-md border border-border bg-muted/30 p-2 text-xs">
            {versions.map((version) => (
              <li key={version.id} className="grid gap-0.5">
                <span className="font-medium text-foreground">
                  v{version.versionNumber} · {new Date(version.createdAt).toLocaleString()}
                </span>
                <span className="text-muted-foreground">{version.instructions}</span>
              </li>
            ))}
          </ul>
        )}
      </div>

      <div className="flex flex-wrap items-center justify-between gap-2">
        <div className="flex flex-wrap items-center gap-2">
          <Button type="button" variant="secondary" onClick={() => void toggleArchived()} disabled={archiving}>
            {persona.archivedAt ? "Unarchive" : "Archive"}
          </Button>
          {deletePreview ? (
            <div className="flex items-center gap-2 text-xs text-muted-foreground">
              <span>
                {deletePreview.conversationCount === 0
                  ? "No conversations are assigned."
                  : `${deletePreview.conversationCount} conversation(s) will be unassigned, not deleted.`}
              </span>
              <Button
                type="button"
                variant="destructive"
                size="sm"
                onClick={() => void confirmDelete()}
                disabled={deleting}
              >
                {deleting ? <Loader2 className="h-4 w-4 animate-spin" /> : null}
                Confirm delete
              </Button>
              <Button type="button" variant="ghost" size="sm" onClick={() => setDeletePreview(null)}>
                Cancel
              </Button>
            </div>
          ) : (
            <Button type="button" variant="ghost" onClick={() => void loadDeletePreview()}>
              <Trash2 className="h-4 w-4" />
              Delete
            </Button>
          )}
        </div>
        <Button
          type="button"
          onClick={() => void save()}
          disabled={saving || !name.trim() || !instructions.trim() || !temperatureValid || !maxTokensValid}
        >
          {saving ? <Loader2 className="h-4 w-4 animate-spin" /> : <Save className="h-4 w-4" />}
          Save
        </Button>
      </div>
    </div>
  );
}

function RemoteProviderCreateForm({
  onCreated,
  onError,
}: {
  onCreated: (provider: ProviderConfig) => void;
  onError: (message: string) => void;
}) {
  const client = useArkClient();
  const [kind, setKind] = React.useState<"open_ai" | "open_ai_compatible">("open_ai");
  const [name, setName] = React.useState("OpenAI");
  const [baseUrl, setBaseUrl] = React.useState("");
  const [acknowledged, setAcknowledged] = React.useState(false);
  const [allowInsecureRemote, setAllowInsecureRemote] = React.useState(false);
  const [saving, setSaving] = React.useState(false);
  const insecureHttp = kind === "open_ai_compatible" && baseUrl.trim().toLowerCase().startsWith("http://");
  const valid =
    name.trim().length > 0 &&
    (kind === "open_ai" || baseUrl.trim().length > 0) &&
    acknowledged &&
    (!insecureHttp || allowInsecureRemote);

  async function create() {
    if (!valid) return;
    setSaving(true);
    try {
      const created = await client.createRemoteProvider({
        name: name.trim(),
        kind,
        baseUrl: kind === "open_ai_compatible" ? baseUrl.trim() : null,
        acknowledgeRemoteRisk: acknowledged,
        allowInsecureRemote,
      });
      onCreated(created);
    } catch (error) {
      onError(getErrorMessage(error));
    } finally {
      setSaving(false);
    }
  }

  return (
    <div className="mb-4 grid gap-3 rounded-md border border-border bg-muted/20 p-3">
      <div>
        <div className="text-sm font-medium">Add a remote provider</div>
        <p className="text-xs text-muted-foreground">
          Remote providers are optional and never enabled by default. Add the credential after creation; Ark stores it
          in the operating-system credential store.
        </p>
      </div>
      <label className="grid gap-1.5 text-sm">
        Provider adapter
        <Select
          value={kind}
          onChange={(event) => {
            const next = event.target.value as "open_ai" | "open_ai_compatible";
            setKind(next);
            setName(next === "open_ai" ? "OpenAI" : "Compatible provider");
            setAllowInsecureRemote(false);
          }}
        >
          <option value="open_ai">OpenAI (curated)</option>
          <option value="open_ai_compatible">OpenAI-compatible (advanced/unverified)</option>
        </Select>
      </label>
      <label className="grid gap-1.5 text-sm">
        Name
        <Input value={name} maxLength={100} onChange={(event) => setName(event.target.value)} />
      </label>
      <label className="grid gap-1.5 text-sm">
        Base URL
        <Input
          value={kind === "open_ai" ? "https://api.openai.com" : baseUrl}
          onChange={(event) => setBaseUrl(event.target.value)}
          readOnly={kind === "open_ai"}
          placeholder="https://provider.example.com"
        />
      </label>
      {kind === "open_ai_compatible" && (
        <p className="text-xs text-amber-700 dark:text-amber-300">
          Advanced compatible endpoints are user-supplied and unverified. Confirm their privacy, billing, model, and
          retention policies with the operator.
        </p>
      )}
      <label className="flex items-start gap-2 text-xs">
        <input
          type="checkbox"
          checked={acknowledged}
          onChange={(event) => setAcknowledged(event.target.checked)}
          className="mt-0.5"
        />
        I understand that the selected model, message, active conversation history, resolved instructions, and any
        attached or searched context sent with a request leave this device. Provider retention and charges may apply.
      </label>
      {insecureHttp && (
        <label className="flex items-start gap-2 text-xs text-amber-700 dark:text-amber-300">
          <input
            type="checkbox"
            checked={allowInsecureRemote}
            onChange={(event) => setAllowInsecureRemote(event.target.checked)}
            className="mt-0.5"
          />
          Development mode: allow unencrypted HTTP. Network observers may read or alter requests.
        </label>
      )}
      <div>
        <Button onClick={() => void create()} disabled={!valid || saving}>
          <Plus className="h-4 w-4" />
          {saving ? "Adding…" : "Add provider"}
        </Button>
      </div>
    </div>
  );
}

function ProviderForm({
  provider,
  models,
  onProviderSaved,
  onProviderDeleted,
  onRefreshProviderModels,
  onCancelProviderRefresh,
  onError,
  secretStoreStatus,
  onSecretStoreRetry,
}: {
  provider: ProviderConfig;
  models: ModelInfo[];
  onProviderSaved: (provider: ProviderConfig) => void;
  onProviderDeleted: (id: string) => void;
  onRefreshProviderModels: (providerId: string) => Promise<void>;
  onCancelProviderRefresh: (providerId: string) => Promise<void>;
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
  const [deleting, setDeleting] = React.useState(false);
  const insecureHttpDestination = baseUrl.trim().toLowerCase().startsWith("http://");
  const supportsCredential = provider.capabilities.requiresAuth || !provider.isLocal || Boolean(provider.apiKeyRef);

  React.useEffect(() => {
    setRemoteRiskMessage(null);
    setRiskAcknowledged(false);
    setConvertToRemoteProvider(!provider.isLocal);
    setAllowInsecureRemote(provider.allowInsecureRemote);
  }, [baseUrl, provider.allowInsecureRemote, provider.isLocal]);

  // FTR-009: re-syncs the draft whenever the provider's own default model changes for any
  // reason — switching tabs, or a refresh (now centralized in the controller, so this
  // component no longer receives the refreshed provider directly as a call result).
  React.useEffect(() => {
    setDefaultModelId(provider.defaultModelId ?? "");
  }, [provider.id, provider.defaultModelId]);

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
    // FTR-009: refreshProviderModels owns error reporting for every caller — see its doc
    // comment in useArkController.ts. The defaultModelId sync effect above reacts to the
    // resulting store update, so no manual follow-up is needed here either.
    setRefreshing(true);
    try {
      await onRefreshProviderModels(provider.id);
    } finally {
      setRefreshing(false);
    }
  }

  async function deleteManagedProvider() {
    const confirmed = window.confirm(
      `Delete ${provider.name}? Its saved credential, discovered models, and active defaults will be removed. Existing message provenance will be retained.`,
    );
    if (!confirmed) return;
    setDeleting(true);
    try {
      await client.deleteProvider(provider.id, true);
      onProviderDeleted(provider.id);
    } catch (error) {
      onError(getErrorMessage(error));
    } finally {
      setDeleting(false);
    }
  }

  return (
    <div className="grid gap-4">
      <div className="grid gap-1.5">
        <label className="text-sm">
          Base URL
          <Input
            value={baseUrl}
            onChange={(event) => setBaseUrl(event.target.value)}
            className="mt-1.5"
            readOnly={provider.providerType === "openai"}
          />
        </label>
        {provider.providerType === "openai" && (
          <div className="rounded-md border border-border bg-muted/40 px-3 py-2.5 text-xs text-muted-foreground">
            OpenAI is an optional remote service. Ark uses the fixed official HTTPS endpoint. Requests may be billed and
            are governed by the provider's current retention and privacy policies; Ark does not infer model context
            limits or prices from the model-list response.
          </div>
        )}
        {provider.providerType === "local_inference_host" && provider.isLocal && (
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
        {refreshing ? (
          <Button variant="secondary" onClick={() => void onCancelProviderRefresh(provider.id)}>
            <Square className="h-4 w-4" />
            Cancel refresh
          </Button>
        ) : (
          <Button variant="secondary" onClick={handleRefresh}>
            <RefreshCw className="h-4 w-4" />
            Refresh models
          </Button>
        )}
        {provider.isUserManaged && (
          <Button variant="destructive" onClick={() => void deleteManagedProvider()} disabled={deleting}>
            <Trash2 className="h-4 w-4" />
            {deleting ? "Deleting…" : "Delete provider"}
          </Button>
        )}
      </div>
      {/* ARC-003: capability-gated (pull support) — installed-model management itself now lives
          in its own Settings → Models section rather than nested here, since it's meaningful
          standalone destination-level functionality, not provider-configuration detail. */}
      {provider.capabilities.modelPull && (
        <p className="text-xs text-muted-foreground">
          Manage installed models in <span className="font-medium text-foreground">Settings → Models</span>.
        </p>
      )}
    </div>
  );
}

function useHardwareFitEvidence(): HardwareFitEvidence | null {
  const client = useArkClient();
  const [evidence, setEvidence] = React.useState<HardwareFitEvidence | null>(null);
  React.useEffect(() => {
    let active = true;
    void client
      .getHardwareFitEvidence()
      .then((value) => {
        if (active) setEvidence(value);
      })
      .catch(() => {
        if (active) setEvidence(null);
      });
    return () => {
      active = false;
    };
  }, [client]);
  return evidence;
}

function ModelInventoryPanel({ providers, models }: { providers: ProviderConfig[]; models: ModelInfo[] }) {
  const hardware = useHardwareFitEvidence();
  const entries = providers.flatMap((provider) =>
    models.filter((model) => model.providerId === provider.id).map((model) => ({ provider, model })),
  );
  if (entries.length === 0) return null;
  return (
    <Panel className="p-4">
      <h3 className="text-sm font-semibold">Your Models</h3>
      <div className="mt-3 grid gap-3 sm:grid-cols-2" role="list">
        {entries.map(({ provider, model }) => {
          const presentation = presentModel(model, provider, hardware?.availableMemoryBytes);
          return (
            <Card key={model.id} className="p-3" role="listitem">
              <div className="flex items-start justify-between gap-2">
                <h4 className="truncate text-sm font-semibold">{presentation.displayName}</h4>
                <Badge tone={model.isAvailable ? "success" : "warning"}>
                  {model.isAvailable ? "available" : "stale"}
                </Badge>
              </div>
              <p className="mt-1 text-xs text-muted-foreground">
                {presentation.sourceLabel} · {presentation.metadataConfidence} metadata
              </p>
              <p className="mt-1 text-xs text-muted-foreground" title={presentation.fitReason}>
                Hardware fit: {presentation.fit.replaceAll("_", " ")} · {presentation.fitConfidence} confidence
              </p>
              <div className="mt-2 flex flex-wrap gap-1">
                {model.supportsTools && <Badge tone="muted">tools</Badge>}
                {model.supportsVision && <Badge tone="muted">vision</Badge>}
                {model.contextWindow && <Badge tone="muted">{model.contextWindow.toLocaleString()} context</Badge>}
              </div>
            </Card>
          );
        })}
      </div>
    </Panel>
  );
}

function OllamaModelsPanel({
  provider,
  models,
  health,
  onRefreshProviderModels,
  onError,
}: {
  provider: ProviderConfig;
  models: ModelInfo[];
  health: ProviderHealth | null;
  onRefreshProviderModels: (providerId: string) => Promise<void>;
  onError: (message: string) => void;
}) {
  const client = useArkClient();
  const hardware = useHardwareFitEvidence();
  const [pullName, setPullName] = React.useState("");
  const [pulling, setPulling] = React.useState(false);
  const [cancelling, setCancelling] = React.useState(false);
  const [pullProgress, setPullProgress] = React.useState<OllamaPullProgress | null>(null);
  const [pullSpeedBytesPerSecond, setPullSpeedBytesPerSecond] = React.useState<number | null>(null);
  const pullSpeedSample = React.useRef<{ digest: string | null; completed: number; at: number } | null>(null);
  const [deletingModel, setDeletingModel] = React.useState<string | null>(null);
  const [refreshing, setRefreshing] = React.useState(false);
  const [showSuggestions, setShowSuggestions] = React.useState(false);
  const [highlightedIndex, setHighlightedIndex] = React.useState(0);
  const [checkingSpace, setCheckingSpace] = React.useState(false);
  // UX-011: a pull the disk-space check flagged as likely too large — held here so the UI can
  // show a warning with an explicit "Continue anyway" rather than silently blocking or silently
  // proceeding. Cleared on cancel, on confirm, or once the pull actually starts.
  const [diskWarning, setDiskWarning] = React.useState<{
    modelName: string;
    requiredGb: number;
    availableGb: number;
  } | null>(null);

  React.useEffect(() => {
    if (!pulling) return;

    let unlisten: (() => void) | undefined;
    void client
      .onOllamaPullProgress((event) => {
        if (event.providerId === provider.id) {
          if (event.completed != null) {
            const now = performance.now();
            const previous = pullSpeedSample.current;
            const digest = event.digest ?? null;
            if (previous && previous.digest === digest && event.completed >= previous.completed) {
              const elapsedSeconds = (now - previous.at) / 1000;
              setPullSpeedBytesPerSecond(
                elapsedSeconds > 0 ? (event.completed - previous.completed) / elapsedSeconds : null,
              );
            } else {
              setPullSpeedBytesPerSecond(null);
            }
            pullSpeedSample.current = { digest, completed: event.completed, at: now };
          } else {
            pullSpeedSample.current = null;
            setPullSpeedBytesPerSecond(null);
          }
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

  // FTR-006: pull/delete require a reachable Ollama instance — attempting either against a
  // stale/unreachable one would just fail after a timeout, so both are disabled up front with a
  // reconnect action instead, matching the "Ollama unreachable" acceptance criterion.
  const reachable = health?.isReachable ?? true;
  const stale = health ? isProviderHealthStale(health.checkedAt) : false;

  async function doPull(name: string) {
    setPulling(true);
    setPullProgress(null);
    setPullSpeedBytesPerSecond(null);
    pullSpeedSample.current = null;
    setDiskWarning(null);
    try {
      await client.pullOllamaModel(provider.id, name);
      await onRefreshProviderModels(provider.id);
      setPullName("");
    } catch (error) {
      const code = error && typeof error === "object" && "code" in error ? (error as AppErrorShape).code : undefined;
      // A deliberate cancel isn't a failure — nothing to show the user beyond the pull simply
      // stopping, which the cleared progress UI below already communicates.
      if (code !== "pull_cancelled") {
        onError(getErrorMessage(error));
      }
    } finally {
      setPulling(false);
      setCancelling(false);
      setPullProgress(null);
      setPullSpeedBytesPerSecond(null);
      pullSpeedSample.current = null;
    }
  }

  // UX-011: only curated tags carry a known approximate size, so the check is skipped (pull
  // proceeds directly) for any free-text tag not in `SUGGESTED_OLLAMA_MODELS` — this is a
  // best-effort hint against a known size, not a general disk-space gate. A failed or
  // zero-valued check (the workspace's drive couldn't be resolved) never blocks the pull either;
  // see `checkDiskSpace`'s own doc comment for why this figure is an approximation.
  async function handlePullClickFor(requestedName: string) {
    const name = requestedName.trim();
    if (!name) return;
    setShowSuggestions(false);

    const curated = SUGGESTED_OLLAMA_MODELS.find((model) => model.name === name);
    if (curated) {
      setCheckingSpace(true);
      try {
        const space = await client.checkDiskSpace();
        const requiredBytes = curated.approxSizeGb * 1024 ** 3;
        if (space.availableBytes > 0 && space.availableBytes < requiredBytes) {
          setDiskWarning({
            modelName: name,
            requiredGb: curated.approxSizeGb,
            availableGb: space.availableBytes / 1024 ** 3,
          });
          return;
        }
      } catch {
        // Best-effort — proceed as if no warning applies.
      } finally {
        setCheckingSpace(false);
      }
    }

    await doPull(name);
  }

  async function handlePullClick() {
    await handlePullClickFor(pullName);
  }

  function selectSuggestion(model: SuggestedOllamaModel) {
    setPullName(model.name);
    setShowSuggestions(false);
  }

  async function handleCancelPull() {
    setCancelling(true);
    try {
      await client.cancelOllamaPull(provider.id);
    } catch (error) {
      onError(getErrorMessage(error));
      setCancelling(false);
    }
  }

  async function handleDelete(model: ModelInfo) {
    const sizeBytes = presentModel(model, provider, hardware?.availableMemoryBytes).sizeBytes;
    const sizeLabel = sizeBytes ? formatBytes(sizeBytes) : null;
    const isDefaultModel = provider.defaultModelId === model.name;
    const confirmed = window.confirm(
      `Delete model "${model.name}" from Ollama${sizeLabel ? ` (${sizeLabel} on disk)` : ""}? This cannot be undone.` +
        (isDefaultModel
          ? `\n\nThis is this provider's default model — conversations without their own override will lose it.`
          : ""),
    );
    if (!confirmed) return;

    setDeletingModel(model.name);
    try {
      await client.deleteOllamaModel(provider.id, model.name);
      await onRefreshProviderModels(provider.id);
    } catch (error) {
      onError(getErrorMessage(error));
    } finally {
      setDeletingModel(null);
    }
  }

  async function handleReconnect() {
    setRefreshing(true);
    try {
      await onRefreshProviderModels(provider.id);
    } catch (error) {
      onError(getErrorMessage(error));
    } finally {
      setRefreshing(false);
    }
  }

  const pullPercent =
    pullProgress?.total && pullProgress.completed
      ? Math.round((pullProgress.completed / pullProgress.total) * 100)
      : null;

  const availableModels = models.filter((m) => m.isAvailable);

  const suggestionQuery = pullName.trim().toLowerCase();
  const filteredSuggestions = (
    suggestionQuery.length === 0
      ? SUGGESTED_OLLAMA_MODELS
      : SUGGESTED_OLLAMA_MODELS.filter(
          (model) =>
            model.name.toLowerCase().includes(suggestionQuery) || model.label.toLowerCase().includes(suggestionQuery),
        )
  ).slice(0, 6);

  return (
    <div className="mt-4 border-t border-border pt-4">
      <div className="mb-3 flex items-center gap-2">
        <HardDrive className="h-4 w-4" />
        <h3 className="text-sm font-semibold">Your Models</h3>
        <span className="ml-auto text-xs text-muted-foreground">
          {availableModels.length} model{availableModels.length !== 1 ? "s" : ""}
        </span>
      </div>

      {!reachable && (
        <div className="mb-3 flex items-center justify-between gap-2 rounded-md border border-destructive/30 bg-destructive/5 px-3 py-2 text-xs">
          <span>
            Ollama is unreachable — showing the last-known model list
            {health?.checkedAt ? ` (checked ${formatRelativeTime(health.checkedAt)})` : ""}. Pull and delete are
            disabled until it reconnects. If Ollama isn't running, start it (
            <code className="rounded bg-muted px-1 py-0.5">ollama serve</code>, or launch the Ollama app) and reconnect.
          </span>
          <Button size="sm" variant="secondary" onClick={() => void handleReconnect()} disabled={refreshing}>
            {refreshing ? <Loader2 className="h-3.5 w-3.5 animate-spin" /> : <RefreshCw className="h-3.5 w-3.5" />}
            Reconnect
          </Button>
        </div>
      )}
      {reachable && stale && health && (
        <div className="mb-3 text-xs text-amber-600 dark:text-amber-400">
          Model list last confirmed {formatRelativeTime(health.checkedAt)} (stale).
        </div>
      )}

      {availableModels.length === 0 ? (
        <p className="text-sm text-muted-foreground">No models installed. Pull one below to get started.</p>
      ) : (
        <div
          className="mb-5 grid gap-3 sm:grid-cols-2 xl:grid-cols-3"
          role="list"
          aria-label={`${provider.name} installed models`}
        >
          {availableModels.map((model) => {
            const presentation = presentModel(model, provider, hardware?.availableMemoryBytes);
            const sizeLabel = presentation.sizeBytes ? formatBytes(presentation.sizeBytes) : null;
            const detailParts = [
              presentation.parameterSize,
              presentation.quantization,
              presentation.family,
              presentation.contextWindow ? `${presentation.contextWindow.toLocaleString()} token context` : undefined,
            ].filter((part): part is string => Boolean(part));
            return (
              <Card key={model.id} className="flex min-w-0 items-start gap-3 p-3" role="listitem">
                <span className="min-w-0 flex-1">
                  <span className="block text-sm font-medium">{model.displayName ?? model.name}</span>
                  {detailParts.length > 0 && (
                    <span className="block text-xs text-muted-foreground">{detailParts.join(" · ")}</span>
                  )}
                  <span className="block text-xs text-muted-foreground">{presentation.sourceLabel}</span>
                  <span className="block text-xs text-muted-foreground">
                    Metadata confidence: {presentation.metadataConfidence}
                  </span>
                  <span className="block text-xs text-muted-foreground" title={presentation.fitReason}>
                    Hardware fit: {presentation.fit.replaceAll("_", " ")} · {presentation.fitConfidence} confidence ·{" "}
                    {presentation.fitMethodVersion}
                  </span>
                  {presentation.licenseSummary && (
                    <span className="block truncate text-xs text-muted-foreground" title={presentation.licenseSummary}>
                      License: {presentation.licenseSummary}
                    </span>
                  )}
                </span>
                {sizeLabel && <span className="text-xs text-muted-foreground">{sizeLabel}</span>}
                <button
                  type="button"
                  onClick={() => void handleDelete(model)}
                  disabled={deletingModel === model.name || !reachable}
                  aria-label={`Delete ${model.name}`}
                  className="rounded p-1 text-muted-foreground hover:text-destructive focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring disabled:opacity-40"
                >
                  {deletingModel === model.name ? (
                    <Loader2 className="h-3.5 w-3.5 animate-spin" />
                  ) : (
                    <Trash2 className="h-3.5 w-3.5" />
                  )}
                </button>
              </Card>
            );
          })}
        </div>
      )}

      <div className="grid gap-2">
        {diskWarning && (
          <div className="grid gap-2 rounded-md border border-warning/50 bg-warning/10 p-3 text-xs" role="alert">
            <span>
              "{diskWarning.modelName}" is about {diskWarning.requiredGb}GB, but the workspace drive only has about{" "}
              {diskWarning.availableGb.toFixed(1)}GB free. The pull may fail partway through if space runs out.
            </span>
            <div className="flex gap-2">
              <Button size="sm" onClick={() => void doPull(diskWarning.modelName)}>
                Continue anyway
              </Button>
              <Button size="sm" variant="secondary" onClick={() => setDiskWarning(null)}>
                Cancel
              </Button>
            </div>
          </div>
        )}

        <div className="flex items-center justify-between gap-2">
          <div>
            <h3 className="text-sm font-semibold">Curated Ollama Library</h3>
            <p className="text-xs text-muted-foreground">
              Reviewed offline suggestions. Sizes are approximate; hardware fit is unknown until Ark can measure the
              execution device.
            </p>
          </div>
        </div>
        <div
          className="grid max-h-[28rem] grid-cols-[repeat(auto-fit,minmax(14rem,1fr))] gap-3 overflow-y-auto pr-1"
          role="list"
          aria-label="Curated Ollama Library"
        >
          {SUGGESTED_OLLAMA_MODELS.map((suggestion) => {
            const installed = availableModels.some((item) => item.name === suggestion.name);
            const fit = assessHardwareFit(
              suggestion.approxSizeGb * 1024 ** 3,
              hardware?.availableMemoryBytes ?? null,
              provider.destinationClass !== "loopback",
            );
            return (
              <Card key={suggestion.name} className="flex min-w-0 flex-col gap-2 p-3" role="listitem">
                <div>
                  <div className="flex items-start justify-between gap-2">
                    <h4 className="text-sm font-semibold">{suggestion.label}</h4>
                    <Badge tone={installed ? "success" : "muted"}>
                      {installed ? "installed" : suggestion.category}
                    </Badge>
                  </div>
                  <p className="mt-1 text-xs text-muted-foreground">{suggestion.description}</p>
                </div>
                <dl className="grid grid-cols-2 gap-1 text-xs text-muted-foreground">
                  <div>
                    <dt className="sr-only">Download size</dt>
                    <dd>~{suggestion.approxSizeGb} GB</dd>
                  </div>
                  <div>
                    <dt className="sr-only">Hardware fit</dt>
                    <dd title={fit.reason}>
                      Fit: {fit.category.replaceAll("_", " ")} · {fit.confidence} confidence
                    </dd>
                  </div>
                </dl>
                <details className="text-xs text-muted-foreground">
                  <summary className="cursor-pointer">Source and provenance</summary>
                  <div className="mt-1 grid gap-1">
                    <span>Reviewed by Ark on {suggestion.reviewedAt}; approximate metadata.</span>
                    <button
                      type="button"
                      className="w-fit text-primary underline"
                      onClick={() => void client.openExternalUrl(suggestion.sourceUrl)}
                    >
                      Open Ollama library source
                    </button>
                  </div>
                </details>
                <Button
                  size="sm"
                  className="mt-auto"
                  variant={installed ? "secondary" : "primary"}
                  disabled={installed || pulling || !reachable}
                  onClick={() => {
                    setPullName(suggestion.name);
                    void handlePullClickFor(suggestion.name);
                  }}
                >
                  {installed ? "Installed" : "Pull"}
                </Button>
              </Card>
            );
          })}
        </div>
        <p className="text-xs text-muted-foreground">Need another model? Enter any Ollama tag below.</p>
        <div className="flex gap-2">
          <div className="relative flex-1">
            <input
              type="text"
              value={pullName}
              onChange={(e) => {
                setPullName(e.target.value);
                setShowSuggestions(true);
                setHighlightedIndex(0);
              }}
              onFocus={() => setShowSuggestions(true)}
              onBlur={() => {
                // Deferred so a click on a suggestion below registers before the list unmounts.
                setTimeout(() => setShowSuggestions(false), 150);
              }}
              onKeyDown={(e) => {
                const suggestions = filteredSuggestions;
                if (showSuggestions && suggestions.length > 0) {
                  if (e.key === "ArrowDown") {
                    e.preventDefault();
                    setHighlightedIndex((i) => Math.min(i + 1, suggestions.length - 1));
                    return;
                  }
                  if (e.key === "ArrowUp") {
                    e.preventDefault();
                    setHighlightedIndex((i) => Math.max(i - 1, 0));
                    return;
                  }
                  if (e.key === "Escape") {
                    setShowSuggestions(false);
                    return;
                  }
                  if (e.key === "Enter" && suggestions[highlightedIndex]) {
                    e.preventDefault();
                    selectSuggestion(suggestions[highlightedIndex]);
                    return;
                  }
                }
                if (e.key === "Enter" && !pulling && pullName.trim() && reachable) {
                  e.preventDefault();
                  void handlePullClick();
                }
              }}
              placeholder="Search suggested models, or type any tag (e.g. llama3.2:3b)"
              disabled={pulling || !reachable}
              className="h-9 w-full rounded-md border border-input bg-background px-3 text-sm outline-none focus-visible:ring-2 focus-visible:ring-ring disabled:opacity-50"
              aria-label="Model name to pull"
              role="combobox"
              aria-expanded={showSuggestions && filteredSuggestions.length > 0}
              aria-controls="ollama-suggested-models-listbox"
              aria-autocomplete="list"
            />
            {showSuggestions && filteredSuggestions.length > 0 && (
              <ul
                id="ollama-suggested-models-listbox"
                role="listbox"
                aria-label="Suggested Ollama models"
                className="absolute z-10 mt-1 max-h-64 w-full overflow-auto rounded-md border border-border bg-popover shadow-md"
              >
                {filteredSuggestions.map((model, index) => (
                  <li key={model.name} role="option" aria-selected={index === highlightedIndex}>
                    <button
                      type="button"
                      onMouseDown={(e) => e.preventDefault()}
                      onClick={() => selectSuggestion(model)}
                      className={cn(
                        "flex w-full flex-col items-start gap-0.5 px-3 py-2 text-left text-xs hover:bg-accent",
                        index === highlightedIndex && "bg-accent",
                      )}
                    >
                      <span className="text-sm font-medium text-foreground">{model.label}</span>
                      <span className="text-muted-foreground">
                        {model.name} · ~{model.approxSizeGb}GB — {model.description}
                      </span>
                    </button>
                  </li>
                ))}
              </ul>
            )}
          </div>
          {pulling ? (
            <Button variant="secondary" onClick={() => void handleCancelPull()} disabled={cancelling}>
              {cancelling ? <Loader2 className="h-4 w-4 animate-spin" /> : null}
              Cancel
            </Button>
          ) : (
            <Button onClick={() => void handlePullClick()} disabled={!pullName.trim() || !reachable || checkingSpace}>
              {checkingSpace ? <Loader2 className="h-4 w-4 animate-spin" /> : <Download className="h-4 w-4" />}
              Pull
            </Button>
          )}
        </div>

        {pulling && pullProgress && (
          <div className="space-y-1">
            <div className="flex items-center justify-between text-xs text-muted-foreground">
              <span className="truncate">{pullProgress.status}</span>
              <span className="flex shrink-0 items-center gap-2">
                {pullProgress.completed != null && (
                  <span>
                    {formatBytes(pullProgress.completed)}
                    {pullProgress.total != null ? ` / ${formatBytes(pullProgress.total)}` : ""}
                  </span>
                )}
                {pullSpeedBytesPerSecond != null && pullSpeedBytesPerSecond > 0 && (
                  <span>{formatBytes(pullSpeedBytesPerSecond)}/s</span>
                )}
                {pullPercent !== null && <span>{pullPercent}%</span>}
              </span>
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

const MAX_WORKSPACE_IMPORT_FILE_BYTES = 50 * 1024 * 1024;

function DataPortabilityPanel({ projects, onError }: { projects: Project[]; onError: (message: string) => void }) {
  const client = useArkClient();
  const fileInputRef = React.useRef<HTMLInputElement | null>(null);
  const [scopeProjectId, setScopeProjectId] = React.useState("");
  const [exporting, setExporting] = React.useState<"json" | "markdown" | null>(null);
  const [exportConfirmation, setExportConfirmation] = React.useState<"json" | "markdown" | null>(null);

  const [importJson, setImportJson] = React.useState<string | null>(null);
  const [preview, setPreview] = React.useState<WorkspaceImportPreview | null>(null);
  const [includedIds, setIncludedIds] = React.useState<Set<string>>(new Set());
  const [previewing, setPreviewing] = React.useState(false);
  const [importing, setImporting] = React.useState(false);
  const [importResult, setImportResult] = React.useState<WorkspaceImportResult | null>(null);

  const scopeLabel = scopeProjectId
    ? (projects.find((project) => project.id === scopeProjectId)?.name ?? "project")
    : "workspace";

  async function handleExport(format: "json" | "markdown") {
    setExporting(format);
    try {
      const projectId = scopeProjectId || null;
      if (format === "json") {
        const json = await client.exportWorkspaceJson(projectId);
        downloadText(`ark-${safeFilename(scopeLabel)}-export.json`, json, "application/json;charset=utf-8");
      } else {
        const markdown = await client.exportWorkspaceMarkdown(projectId);
        downloadText(`ark-${safeFilename(scopeLabel)}-export.md`, markdown, "text/markdown;charset=utf-8");
      }
    } catch (error) {
      onError(getErrorMessage(error));
    } finally {
      setExporting(null);
      setExportConfirmation(null);
    }
  }

  async function handleChooseFile(event: React.ChangeEvent<HTMLInputElement>) {
    const file = event.target.files?.[0];
    event.target.value = "";
    if (!file) return;

    if (file.size > MAX_WORKSPACE_IMPORT_FILE_BYTES) {
      onError(
        `"${file.name}" is ${(file.size / (1024 * 1024)).toFixed(1)} MB, which exceeds the ${MAX_WORKSPACE_IMPORT_FILE_BYTES / (1024 * 1024)} MB import limit.`,
      );
      return;
    }

    setPreviewing(true);
    setPreview(null);
    setImportResult(null);
    try {
      const json = await file.text();
      const nextPreview = await client.previewWorkspaceImport(json);
      setImportJson(json);
      setPreview(nextPreview);
      // FTR-008: entries whose content already matches a local conversation default to
      // unchecked (skip) — see WorkspaceImportPreviewEntry's doc comment in import_export.rs.
      setIncludedIds(
        new Set(nextPreview.entries.filter((entry) => !entry.duplicateOfLocalId).map((entry) => entry.conversationId)),
      );
    } catch (error) {
      onError(getErrorMessage(error));
    } finally {
      setPreviewing(false);
    }
  }

  function toggleIncluded(conversationId: string) {
    setIncludedIds((current) => {
      const next = new Set(current);
      if (next.has(conversationId)) {
        next.delete(conversationId);
      } else {
        next.add(conversationId);
      }
      return next;
    });
  }

  async function handleImport() {
    if (!importJson) return;
    setImporting(true);
    try {
      const result = await client.importWorkspaceJson(importJson, Array.from(includedIds));
      setImportResult(result);
      setPreview(null);
      setImportJson(null);
    } catch (error) {
      onError(getErrorMessage(error));
    } finally {
      setImporting(false);
    }
  }

  return (
    <Panel className="p-4">
      <div className="mb-2 flex items-center gap-2">
        <FileText className="h-4 w-4" />
        <h2 className="text-sm font-semibold">Data portability</h2>
      </div>
      <p className="text-sm text-muted-foreground">
        Export every conversation in the workspace (or a single project) as one bundle, or import a bundle exported from
        another Ark workspace.
      </p>
      <div className="mt-3 grid gap-4">
        <div className="grid gap-2">
          <label className="grid gap-1.5 text-sm">
            Scope
            <Select value={scopeProjectId} onChange={(event) => setScopeProjectId(event.target.value)}>
              <option value="">Entire workspace</option>
              {projects.map((project) => (
                <option key={project.id} value={project.id}>
                  {project.name}
                </option>
              ))}
            </Select>
          </label>
          <div className="flex flex-wrap gap-2">
            <Button variant="secondary" onClick={() => setExportConfirmation("json")} disabled={exporting !== null}>
              {exporting === "json" ? <Loader2 className="h-4 w-4 animate-spin" /> : <Download className="h-4 w-4" />}
              Export JSON
            </Button>
            <Button variant="secondary" onClick={() => setExportConfirmation("markdown")} disabled={exporting !== null}>
              {exporting === "markdown" ? (
                <Loader2 className="h-4 w-4 animate-spin" />
              ) : (
                <Download className="h-4 w-4" />
              )}
              Export Markdown
            </Button>
          </div>
          {exportConfirmation && (
            <div role="alert" className="grid gap-2 rounded-md border border-amber-500/30 bg-amber-500/10 p-3 text-xs">
              <div className="font-medium text-amber-800 dark:text-amber-200">Review this plaintext export</div>
              <p className="text-muted-foreground">
                This file contains every conversation and attachment in the selected{" "}
                {scopeProjectId ? "project" : "workspace"}, plus provider/model provenance. Credentials and caches are
                excluded. Anyone who can read the destination file can read this content; choose a protected location.
              </p>
              <div className="flex flex-wrap gap-2">
                <Button size="sm" onClick={() => void handleExport(exportConfirmation)}>
                  <Download className="h-4 w-4" />
                  Export {exportConfirmation === "json" ? "JSON" : "Markdown"}
                </Button>
                <Button size="sm" variant="ghost" onClick={() => setExportConfirmation(null)}>
                  Cancel
                </Button>
              </div>
            </div>
          )}
        </div>

        <div className="grid gap-2 border-t border-border pt-3">
          <div className="text-sm font-medium">Import a bundle</div>
          <Button
            variant="secondary"
            onClick={() => fileInputRef.current?.click()}
            disabled={previewing}
            className="w-fit"
          >
            {previewing ? <Loader2 className="h-4 w-4 animate-spin" /> : null}
            Choose file
          </Button>
          <input
            ref={fileInputRef}
            type="file"
            accept="application/json,.json"
            className="hidden"
            onChange={handleChooseFile}
          />

          {preview && (
            <div className="grid gap-2 rounded-md border border-border bg-muted/30 p-3 text-xs">
              <div className="text-muted-foreground">
                {preview.entries.length} conversation{preview.entries.length === 1 ? "" : "s"} in bundle (scope:{" "}
                {preview.scope}) · {includedIds.size} selected to import
              </div>
              <ul className="grid max-h-64 gap-1 overflow-y-auto">
                {preview.entries.map((entry) => (
                  <li key={entry.conversationId} className="flex items-center gap-2">
                    <input
                      type="checkbox"
                      checked={includedIds.has(entry.conversationId)}
                      onChange={() => toggleIncluded(entry.conversationId)}
                      aria-label={`Import "${entry.title}"`}
                    />
                    <span className="min-w-0 flex-1 truncate text-foreground">{entry.title}</span>
                    <span className="shrink-0 text-muted-foreground">
                      {entry.messageCount} msgs
                      {entry.attachmentCount > 0 ? ` · ${entry.attachmentCount} files` : ""}
                    </span>
                    {entry.duplicateOfLocalId && <Badge tone="warning">already in workspace</Badge>}
                  </li>
                ))}
              </ul>
              {preview.providerMappings.length > 0 && (
                <div className="text-muted-foreground">
                  Providers:{" "}
                  {preview.providerMappings
                    .map((mapping) => `${mapping.sourceProviderId ?? "unspecified"} → ${mapping.targetProviderId}`)
                    .join(", ")}
                </div>
              )}
              <Button
                onClick={() => void handleImport()}
                disabled={importing || includedIds.size === 0}
                className="w-fit"
              >
                {importing ? <Loader2 className="h-4 w-4 animate-spin" /> : null}
                Import {includedIds.size} selected
              </Button>
            </div>
          )}

          {importResult && (
            <p role="status" className="text-sm text-emerald-600 dark:text-emerald-300">
              Imported {importResult.importedCount} conversation{importResult.importedCount === 1 ? "" : "s"}
              {importResult.skippedCount > 0 ? `, skipped ${importResult.skippedCount} not selected for import` : ""}.
            </p>
          )}
        </div>
      </div>
    </Panel>
  );
}

function CompanionApiPanel({ onError }: { onError: (message: string) => void }) {
  const client = useArkClient();
  const [status, setStatus] = React.useState<CompanionApiStatus | null>(null);
  const [loading, setLoading] = React.useState(true);
  const [toggling, setToggling] = React.useState(false);
  const [regenerating, setRegenerating] = React.useState(false);
  const [revealedToken, setRevealedToken] = React.useState<string | null>(null);

  React.useEffect(() => {
    let cancelled = false;
    client
      .getCompanionApiStatus()
      .then((next) => {
        if (!cancelled) setStatus(next);
      })
      .catch((error) => onError(getErrorMessage(error)))
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  async function toggle() {
    if (!status) return;
    setToggling(true);
    try {
      setStatus(await client.setCompanionApiEnabled(!status.enabled));
    } catch (error) {
      onError(getErrorMessage(error));
    } finally {
      setToggling(false);
    }
  }

  async function regenerate() {
    setRegenerating(true);
    try {
      const reveal = await client.regenerateCompanionApiToken();
      setStatus(reveal.status);
      setRevealedToken(reveal.token);
    } catch (error) {
      onError(getErrorMessage(error));
    } finally {
      setRegenerating(false);
    }
  }

  return (
    <Panel className="p-4">
      <div className="mb-2 flex items-center gap-2">
        <Network className="h-4 w-4" />
        <h2 className="text-sm font-semibold">Companion API</h2>
      </div>
      <p className="text-sm text-muted-foreground">
        A local API for integrations and the future mobile companion. Off by default. Loopback (this device) only —
        network/LAN pairing is not available yet. Every request requires the bearer token below; there is no
        unauthenticated route, including health checks.
      </p>
      <div className="mt-3 grid gap-3">
        {loading ? (
          <div className="flex items-center gap-2 text-xs text-muted-foreground">
            <Loader2 className="h-3.5 w-3.5 animate-spin" /> Loading status…
          </div>
        ) : (
          status && (
            <>
              <div className="flex flex-wrap items-center gap-2">
                <Badge tone={status.running ? "success" : "muted"}>{status.running ? "running" : "stopped"}</Badge>
                {status.running && status.port && (
                  <span className="text-xs text-muted-foreground">http://127.0.0.1:{status.port}</span>
                )}
                <Button
                  variant="secondary"
                  onClick={() => void toggle()}
                  disabled={toggling || (!status.enabled && !status.tokenConfigured)}
                  className="ml-auto"
                >
                  {toggling ? <Loader2 className="h-4 w-4 animate-spin" /> : null}
                  {status.enabled ? "Disable" : "Enable"}
                </Button>
              </div>

              <div className="flex flex-wrap items-center gap-2 border-t border-border pt-3">
                <span className="text-xs text-muted-foreground">
                  {status.tokenConfigured
                    ? "A bearer token is configured. The API contract is at authenticated GET /v1/openapi.json."
                    : "Generate and save a bearer token before enabling the API."}
                </span>
                <Button
                  variant="secondary"
                  onClick={() => void regenerate()}
                  disabled={regenerating}
                  className="ml-auto"
                >
                  {regenerating ? <Loader2 className="h-4 w-4 animate-spin" /> : <RefreshCw className="h-4 w-4" />}
                  {status.tokenConfigured ? "Regenerate token" : "Generate token"}
                </Button>
              </div>

              {revealedToken && (
                <div className="grid gap-2 rounded-md border border-warning/50 bg-warning/10 p-3" role="alert">
                  <div className="text-sm font-medium">Save this token now</div>
                  <p className="text-xs text-muted-foreground">
                    Ark shows it once and does not store a second recoverable copy. The previous token, if any, stops
                    working immediately.
                  </p>
                  <code className="select-all break-all rounded bg-background p-2 text-xs">{revealedToken}</code>
                  <Button variant="secondary" onClick={() => setRevealedToken(null)}>
                    I saved this token
                  </Button>
                </div>
              )}
            </>
          )
        )}
      </div>
    </Panel>
  );
}

/** CMP-003: the Tools settings panel — shows every built-in tool's declared publisher/scope/trust
 * status, lets the user proactively grant or immediately revoke access, and shows the persisted,
 * tamper-evident audit trail with a one-click integrity check. Today there is exactly one
 * built-in tool ("Notes"); this panel is written to scale to more without change. */
function ToolsPanel({ onError }: { onError: (message: string) => void }) {
  const client = useArkClient();
  const [tools, setTools] = React.useState<ToolStatus[] | null>(null);
  const [events, setEvents] = React.useState<AuditEvent[]>([]);
  const [ttlMinutes, setTtlMinutes] = React.useState("5");
  const [busyToolId, setBusyToolId] = React.useState<string | null>(null);
  const [integrityResult, setIntegrityResult] = React.useState<boolean | null>(null);
  const [checkingIntegrity, setCheckingIntegrity] = React.useState(false);
  const [showTrail, setShowTrail] = React.useState(false);

  const refresh = React.useCallback(async () => {
    try {
      const [nextTools, nextEvents] = await Promise.all([client.listTools(), client.listToolAuditEvents()]);
      setTools(nextTools);
      setEvents(nextEvents);
    } catch (error) {
      onError(getErrorMessage(error));
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  React.useEffect(() => {
    void refresh();
  }, [refresh]);

  async function grant(toolId: string) {
    const parsed = Number.parseInt(ttlMinutes, 10);
    if (!Number.isFinite(parsed) || parsed < 1 || parsed > 60) {
      onError("Grant duration must be between 1 and 60 minutes.");
      return;
    }
    setBusyToolId(toolId);
    try {
      await client.grantToolCapability(toolId, parsed);
      await refresh();
    } catch (error) {
      onError(getErrorMessage(error));
    } finally {
      setBusyToolId(null);
    }
  }

  async function revoke(grantId: string, toolId: string) {
    setBusyToolId(toolId);
    try {
      await client.revokeToolCapability(grantId);
      await refresh();
    } catch (error) {
      onError(getErrorMessage(error));
    } finally {
      setBusyToolId(null);
    }
  }

  async function checkIntegrity() {
    setCheckingIntegrity(true);
    try {
      setIntegrityResult(await client.verifyToolAuditTrail());
    } catch (error) {
      onError(getErrorMessage(error));
    } finally {
      setCheckingIntegrity(false);
    }
  }

  return (
    <Panel className="p-4">
      <div className="mb-2 flex items-center gap-2">
        <Wrench className="h-4 w-4" />
        <h2 className="text-sm font-semibold">Tools</h2>
      </div>
      <p className="text-sm text-muted-foreground">
        Built-in, chat-safe tools Ark can use. Each declares exactly what it can read/write/reach over the network —
        writes need a preview and your approval unless you grant access below. No external tool servers are connected in
        this build.
      </p>

      {!tools ? (
        <div className="mt-3 flex items-center gap-2 text-xs text-muted-foreground">
          <Loader2 className="h-3.5 w-3.5 animate-spin" /> Loading tools…
        </div>
      ) : (
        <div className="mt-3 grid gap-3">
          {tools.map((tool) => (
            <div key={tool.definition.id} className="rounded-md border border-border p-3">
              <div className="flex flex-wrap items-center gap-2">
                <span className="text-sm font-medium">{tool.definition.name}</span>
                <Badge tone="muted">{tool.definition.publisher}</Badge>
                <Badge tone={tool.definition.scope.tier === "chat_safe" ? "success" : "warning"}>
                  {tool.definition.scope.tier === "chat_safe" ? "chat-safe" : "repository-execution"}
                </Badge>
              </div>
              <p className="mt-1 text-xs text-muted-foreground">{tool.definition.description}</p>
              {tool.definition.id === "web_search" && (
                <div className="mt-2 rounded-md border border-warning/40 bg-warning/10 p-2 text-xs text-muted-foreground">
                  Queries leave this device for Brave Search. Brave currently requires account/payment details, offers a
                  monthly credit, and may retain query records for up to 90 days. Ark sends only the query after
                  explicit tool approval; review current terms before enabling.
                </div>
              )}
              <div className="mt-2 flex flex-wrap gap-1 text-xs text-muted-foreground">
                <span>Scope: {tool.definition.scope.data}</span>
                <span aria-hidden="true">·</span>
                <span>
                  {[
                    tool.definition.scope.read && "read",
                    tool.definition.scope.write && "write",
                    tool.definition.scope.network && "network",
                    tool.definition.scope.secret && "secret",
                  ]
                    .filter(Boolean)
                    .join(", ")}
                </span>
              </div>

              {tool.definition.scope.secret && <ToolSecretField toolId={tool.definition.id} onError={onError} />}

              <div className="mt-3 flex flex-wrap items-center gap-2 border-t border-border pt-2">
                {tool.activeGrant ? (
                  <>
                    <Badge tone="success">
                      Granted until {new Date(tool.activeGrant.expiresAt).toLocaleTimeString()}
                    </Badge>
                    <Button
                      variant="secondary"
                      className="ml-auto"
                      disabled={busyToolId === tool.definition.id}
                      onClick={() => void revoke(tool.activeGrant!.id, tool.definition.id)}
                    >
                      {busyToolId === tool.definition.id ? <Loader2 className="h-4 w-4 animate-spin" /> : null}
                      Revoke access
                    </Button>
                  </>
                ) : (
                  <>
                    <Badge tone="muted">No active grant — writes will ask for approval</Badge>
                    <label className="ml-auto flex items-center gap-1 text-xs text-muted-foreground">
                      Grant for
                      <input
                        type="number"
                        min={1}
                        max={60}
                        value={ttlMinutes}
                        onChange={(event) => setTtlMinutes(event.target.value)}
                        className="w-14 rounded border border-border bg-background px-1 py-0.5 text-xs"
                        aria-label="Grant duration in minutes"
                      />
                      min
                    </label>
                    <Button
                      variant="secondary"
                      disabled={busyToolId === tool.definition.id}
                      onClick={() => void grant(tool.definition.id)}
                    >
                      {busyToolId === tool.definition.id ? <Loader2 className="h-4 w-4 animate-spin" /> : null}
                      Grant access
                    </Button>
                  </>
                )}
              </div>
            </div>
          ))}

          <div className="border-t border-border pt-3">
            <div className="flex flex-wrap items-center gap-2">
              <Button variant="secondary" onClick={() => setShowTrail((value) => !value)}>
                {showTrail ? "Hide" : "Show"} audit trail ({events.length})
              </Button>
              <Button variant="secondary" disabled={checkingIntegrity} onClick={() => void checkIntegrity()}>
                {checkingIntegrity ? <Loader2 className="h-4 w-4 animate-spin" /> : null}
                Verify integrity
              </Button>
              {integrityResult !== null && (
                <Badge tone={integrityResult ? "success" : "danger"}>
                  {integrityResult ? "Trail verified — unmodified" : "Trail failed verification"}
                </Badge>
              )}
            </div>
            {showTrail && (
              <ul className="mt-2 grid gap-1 text-xs text-muted-foreground">
                {events.length === 0 && <li>No tool activity yet.</li>}
                {events.map((event) => (
                  <li key={event.sequence} className="flex flex-wrap gap-1">
                    <span className="font-mono">#{event.sequence}</span>
                    <span className="font-medium text-foreground">{event.kind}</span>
                    <span>{event.toolId}</span>
                    <span>— {event.redactedDetail}</span>
                    <span className="ml-auto">{new Date(event.timestamp).toLocaleTimeString()}</span>
                  </li>
                ))}
              </ul>
            )}
          </div>
        </div>
      )}
    </Panel>
  );
}

/** CMP-004: a small, self-contained credential entry for any secret-scoped built-in tool —
 * shown inline in its `ToolsPanel` card. Mirrors `ProviderForm`'s masked-input save/delete
 * state shape, adapted to `upsertToolSecret`/`getToolSecretMetadata`/`deleteToolSecret` instead
 * of the provider-secret equivalents. */
function ToolSecretField({ toolId, onError }: { toolId: string; onError: (message: string) => void }) {
  const client = useArkClient();
  const [secretDraft, setSecretDraft] = React.useState("");
  const [secretMetadata, setSecretMetadata] = React.useState<SecretMetadata | null>(null);
  const [busy, setBusy] = React.useState(false);
  const [loaded, setLoaded] = React.useState(false);

  React.useEffect(() => {
    let active = true;
    void client
      .getToolSecretMetadata(toolId)
      .then((metadata) => {
        if (active) setSecretMetadata(metadata);
      })
      .catch((error) => {
        if (active) onError(getErrorMessage(error));
      })
      .finally(() => {
        if (active) setLoaded(true);
      });
    return () => {
      active = false;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [client, toolId]);

  async function saveSecret() {
    if (!secretDraft) return;
    const secret = secretDraft;
    setSecretDraft("");
    setBusy(true);
    try {
      setSecretMetadata(await client.upsertToolSecret(toolId, secret));
    } catch (error) {
      onError(getErrorMessage(error));
    } finally {
      setBusy(false);
    }
  }

  async function deleteSecret() {
    setBusy(true);
    try {
      await client.deleteToolSecret(toolId);
      setSecretMetadata(null);
    } catch (error) {
      onError(getErrorMessage(error));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="mt-3 grid gap-2 rounded-md border border-border bg-muted/20 p-3">
      <div className="text-xs font-medium">API credential</div>
      {loaded && secretMetadata && (
        <div className="flex items-center gap-2 text-xs">
          <Badge tone={secretMetadata.available ? "success" : "warning"}>
            {secretMetadata.available ? "configured" : "reconnection required"}
          </Badge>
          <span aria-label="Saved credential">{secretMetadata.masked}</span>
        </div>
      )}
      <label className="grid gap-1.5 text-xs">
        {secretMetadata ? "Replace credential" : "Credential"}
        <Input
          type="password"
          value={secretDraft}
          onChange={(event) => setSecretDraft(event.target.value)}
          autoComplete="new-password"
          autoCapitalize="none"
          spellCheck={false}
          disabled={busy}
          className="text-xs"
        />
      </label>
      <div className="flex flex-wrap gap-2">
        <Button size="sm" onClick={() => void saveSecret()} disabled={!secretDraft || busy}>
          <Save className="h-3.5 w-3.5" />
          Save credential
        </Button>
        {secretMetadata && (
          <Button size="sm" variant="secondary" onClick={() => void deleteSecret()} disabled={busy}>
            <Trash2 className="h-3.5 w-3.5" />
            Remove credential
          </Button>
        )}
      </div>
    </div>
  );
}

/** CMP-006: device-scoped, opt-in — same `DeviceSettings` struct and shape as the crash-capture
 * toggle just below it, which is why this lives here in Advanced rather than in Appearance
 * (which today renders only the theme control) or a new top-level tab. */
function NotificationsPanel({
  completionNotificationsEnabled,
  onCompletionNotificationsEnabledChange,
}: {
  completionNotificationsEnabled: boolean;
  onCompletionNotificationsEnabledChange: (enabled: boolean) => void;
}) {
  return (
    <Panel className="p-4">
      <div className="mb-2 flex items-center gap-2">
        <Bell className="h-4 w-4" />
        <h2 className="text-sm font-semibold">Notifications</h2>
      </div>
      <p className="text-sm text-muted-foreground">
        Shows a native OS notification when a response finishes, fails, or is interrupted while Ark's window isn't
        focused. The notification never includes the conversation title, your message, or the response — just that
        something finished. Enabling this will prompt for OS notification permission.
      </p>

      <label className="mt-3 flex items-center gap-2 text-sm">
        <input
          type="checkbox"
          checked={completionNotificationsEnabled}
          onChange={(event) => onCompletionNotificationsEnabledChange(event.target.checked)}
          className="h-4 w-4"
        />
        Notify when a response finishes (off by default)
      </label>
    </Panel>
  );
}

/** PERF-001: device-scoped, opt-in — same `DeviceSettings` struct/shape as the two toggles above
 * it. No permission prompt, unlike notifications: this only gates writes into the existing local
 * diagnostics log, visible in the diagnostics bundle's "Recent performance metrics" section. */
function PerfMetricsPanel({
  perfMetricsEnabled,
  onPerfMetricsEnabledChange,
}: {
  perfMetricsEnabled: boolean;
  onPerfMetricsEnabledChange: (enabled: boolean) => void;
}) {
  return (
    <Panel className="p-4">
      <div className="mb-2 flex items-center gap-2">
        <Gauge className="h-4 w-4" />
        <h2 className="text-sm font-semibold">Performance metrics</h2>
      </div>
      <p className="text-sm text-muted-foreground">
        Records local timing and count measurements — startup duration, response speed, database batch counts — to help
        diagnose slowness. Never includes prompts, responses, or conversation content. Recorded metrics appear in the
        diagnostics bundle above and are never sent anywhere automatically.
      </p>

      <label className="mt-3 flex items-center gap-2 text-sm">
        <input
          type="checkbox"
          checked={perfMetricsEnabled}
          onChange={(event) => onPerfMetricsEnabledChange(event.target.checked)}
          className="h-4 w-4"
        />
        Record local performance metrics (off by default)
      </label>
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
  managedModelDirectory,
  onManagedModelDirectoryChange,
  onRefreshProviderModels,
  onError,
}: {
  status: BuiltInRuntimeStatus;
  onStatusChange: (status: BuiltInRuntimeStatus) => void;
  /** ARC-006: the device-scoped last-selected model path (see `App.tsx`'s `builtInModelPath`
   * state) — this component owns only the in-progress text-field draft, not the persisted
   * value itself, so a keystroke doesn't trigger a backend write on every character. */
  modelPath: string | null;
  onModelPathChange: (path: string) => void;
  managedModelDirectory: string | null;
  onManagedModelDirectoryChange: (path: string | null) => Promise<void>;
  onRefreshProviderModels: (providerId: string) => Promise<void>;
  onError: (message: string) => void;
}) {
  const client = useArkClient();
  const [modelPathDraft, setModelPathDraft] = React.useState(modelPath ?? "");
  const [modelSource, setModelSource] = React.useState(status.modelProvenance?.source ?? "");
  const [modelLicense, setModelLicense] = React.useState(status.modelProvenance?.license ?? "");
  const [managedModels, setManagedModels] = React.useState<ManagedModelStatus[]>([]);
  const [selectedModelId, setSelectedModelId] = React.useState("");
  const [storageDraft, setStorageDraft] = React.useState(managedModelDirectory ?? "");
  const [catalogLoading, setCatalogLoading] = React.useState(true);
  const [storageSaving, setStorageSaving] = React.useState(false);
  const [downloadProgress, setDownloadProgress] = React.useState<ManagedModelDownloadProgress | null>(null);
  const [downloading, setDownloading] = React.useState(false);
  const [deleting, setDeleting] = React.useState(false);
  const [deleteConfirmation, setDeleteConfirmation] = React.useState(false);
  const [pendingPreflight, setPendingPreflight] = React.useState<ManagedModelPreflight | null>(null);
  const [overrideReason, setOverrideReason] = React.useState("");
  const [starting, setStarting] = React.useState(false);
  const [stopping, setStopping] = React.useState(false);
  const [refreshing, setRefreshing] = React.useState(false);

  const refreshManagedModels = React.useCallback(async () => {
    setCatalogLoading(true);
    try {
      const next = await client.listManagedModels();
      setManagedModels(next);
      setSelectedModelId((current) =>
        next.some((candidate) => candidate.model.id === current) ? current : (next[0]?.model.id ?? ""),
      );
    } catch (error) {
      onError(getErrorMessage(error));
    } finally {
      setCatalogLoading(false);
    }
  }, [client, onError]);

  React.useEffect(() => {
    void refreshManagedModels();
  }, [refreshManagedModels]);

  React.useEffect(() => {
    let disposed = false;
    let unsubscribe: (() => void) | undefined;
    void client
      .onManagedModelDownloadProgress((event) => {
        if (!disposed) setDownloadProgress(event);
      })
      .then((next) => {
        if (disposed) next();
        else unsubscribe = next;
      })
      .catch((error) => {
        if (!disposed) onError(getErrorMessage(error));
      });
    return () => {
      disposed = true;
      unsubscribe?.();
    };
  }, [client, onError]);

  React.useEffect(() => {
    setStorageDraft(managedModelDirectory ?? "");
  }, [managedModelDirectory]);

  const selectedModel = managedModels.find((candidate) => candidate.model.id === selectedModelId) ?? null;

  function hasErrorCode(error: unknown, code: string): boolean {
    return Boolean(
      error && typeof error === "object" && "code" in error && (error as { code?: unknown }).code === code,
    );
  }

  async function saveStorageDirectory() {
    setStorageSaving(true);
    try {
      await onManagedModelDirectoryChange(storageDraft.trim() || null);
      setPendingPreflight(null);
      await refreshManagedModels();
    } catch {
      // The controller owns rollback and user-visible error reporting for device-setting writes.
    } finally {
      setStorageSaving(false);
    }
  }

  async function performManagedOperation(
    operation: ManagedModelOperation,
    acknowledgeWarning: boolean,
    advancedOverride: boolean,
  ) {
    if (!selectedModel) return;
    setPendingPreflight(null);
    if (operation === "download") {
      setDownloading(true);
      setDownloadProgress({
        schemaVersion: 1,
        modelId: selectedModel.model.id,
        status: selectedModel.partialBytes > 0 ? "resuming" : "starting",
        completedBytes: selectedModel.partialBytes,
        totalBytes: selectedModel.model.sizeBytes,
        resumed: selectedModel.partialBytes > 0,
      });
      try {
        const installed = await client.downloadManagedModel(
          selectedModel.model.id,
          acknowledgeWarning,
          advancedOverride,
          overrideReason.trim() || null,
        );
        setManagedModels((current) =>
          current.map((candidate) => (candidate.model.id === installed.model.id ? installed : candidate)),
        );
      } catch (error) {
        if (!hasErrorCode(error, "model_download_cancelled")) onError(getErrorMessage(error));
        await refreshManagedModels();
      } finally {
        setDownloading(false);
      }
      return;
    }

    setStarting(true);
    try {
      const next = await client.startManagedModel(
        selectedModel.model.id,
        acknowledgeWarning,
        advancedOverride,
        overrideReason.trim() || null,
      );
      onStatusChange(next);
      onModelPathChange(next.modelPath ?? selectedModel.modelPath);
      await onRefreshProviderModels("built_in");
    } catch (error) {
      const startError = getErrorMessage(error);
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

  async function prepareManagedOperation(operation: ManagedModelOperation) {
    if (!selectedModel) return;
    try {
      const preflight = await client.preflightManagedModel(selectedModel.model.id, operation);
      setOverrideReason("");
      if (preflight.risk === "safe") {
        await performManagedOperation(operation, false, false);
      } else {
        setPendingPreflight(preflight);
      }
    } catch (error) {
      onError(getErrorMessage(error));
    }
  }

  async function cancelDownload() {
    if (!selectedModel) return;
    try {
      await client.cancelManagedModelDownload(selectedModel.model.id);
    } catch (error) {
      onError(getErrorMessage(error));
    }
  }

  async function deleteManagedModel() {
    if (!selectedModel) return;
    if (!deleteConfirmation) {
      setDeleteConfirmation(true);
      return;
    }
    setDeleting(true);
    try {
      await client.deleteManagedModel(selectedModel.model.id);
      setDeleteConfirmation(false);
      setDownloadProgress(null);
      setPendingPreflight(null);
      await refreshManagedModels();
    } catch (error) {
      onError(getErrorMessage(error));
    } finally {
      setDeleting(false);
    }
  }

  async function handleManualStart() {
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
      await onRefreshProviderModels("built_in");
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
      await onRefreshProviderModels("built_in");
    } catch (err) {
      onError(getErrorMessage(err));
    } finally {
      setStopping(false);
    }
  }

  async function handleRefresh() {
    setRefreshing(true);
    try {
      await onRefreshProviderModels("built_in");
    } finally {
      setRefreshing(false);
    }
  }

  const modelFileName = status.modelPath?.split(/[/\\]/).pop() ?? null;
  const displayedProgress = downloadProgress?.modelId === selectedModel?.model.id ? downloadProgress : null;
  const progressPercent = displayedProgress
    ? Math.min(100, Math.round((displayedProgress.completedBytes / displayedProgress.totalBytes) * 100))
    : 0;

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
          {import.meta.env.DEV ? (
            <p>
              Run <code className="rounded bg-background px-1 font-mono text-[11px]">scripts/setup-llama.ps1</code> (or{" "}
              <code className="rounded bg-background px-1 font-mono text-[11px]">setup-llama.sh</code> on macOS/Linux)
              from the repo root to install the reviewed runtime, then reopen Settings.
            </p>
          ) : (
            <p>
              This Ark package is missing its verified runtime resource. Reinstall the package; Ark will not run an
              unverified replacement.
            </p>
          )}
        </div>
      )}

      {status.binaryInstalled && !status.binaryVerified && (
        <div role="alert" className="rounded-md border border-destructive/30 bg-destructive/10 px-3 py-2.5 text-xs">
          The runtime exists but its reviewed provenance or installed-file hashes did not verify.{" "}
          {import.meta.env.DEV ? "Re-run the setup script" : "Reinstall this Ark package"}; Ark will not execute it.
        </div>
      )}

      <section className="grid gap-3 rounded-md border border-border p-3" aria-labelledby="managed-models-title">
        <div>
          <div id="managed-models-title" className="font-medium">
            Verified model catalog
          </div>
          <p className="mt-1 text-xs text-muted-foreground">
            Ark downloads only publisher-pinned files whose exact size and SHA-256 are reviewed with this build.
          </p>
        </div>

        <label className="grid gap-1.5 text-sm">
          Model storage directory
          <div className="flex flex-wrap gap-2">
            <Input
              value={storageDraft}
              onChange={(event) => setStorageDraft(event.target.value)}
              placeholder={selectedModel?.storageDirectory ?? "Ark application data/models"}
              disabled={storageSaving || downloading || status.running}
              className="min-w-64 flex-1"
            />
            <Button
              variant="secondary"
              onClick={() => void saveStorageDirectory()}
              disabled={storageSaving || downloading || status.running}
            >
              {storageSaving ? "Saving…" : "Save location"}
            </Button>
          </div>
          <span className="text-xs text-muted-foreground">
            Leave blank to use Ark's private application-data directory. Existing model files are never moved
            implicitly.
          </span>
        </label>

        {catalogLoading ? (
          <div className="flex items-center gap-2 text-sm text-muted-foreground">
            <Loader2 className="h-4 w-4 animate-spin" /> Loading reviewed catalog…
          </div>
        ) : selectedModel ? (
          <>
            <label className="grid gap-1.5 text-sm">
              Managed model
              <Select
                value={selectedModel.model.id}
                onChange={(event) => {
                  setSelectedModelId(event.target.value);
                  setPendingPreflight(null);
                  setDeleteConfirmation(false);
                }}
                disabled={downloading || starting}
              >
                {managedModels.map((candidate) => (
                  <option key={candidate.model.id} value={candidate.model.id}>
                    {candidate.model.displayName} · {candidate.model.quantization} ·{" "}
                    {formatBytes(candidate.model.sizeBytes)}
                  </option>
                ))}
              </Select>
            </label>

            <div className="grid gap-2 rounded-md bg-muted/40 px-3 py-2.5 text-xs">
              <div className="flex flex-wrap items-center gap-2">
                <span className="font-medium text-foreground">{selectedModel.model.displayName}</span>
                <Badge>{selectedModel.model.license}</Badge>
                <Badge>{selectedModel.model.quantization}</Badge>
                <span className="text-muted-foreground">
                  {selectedModel.installed && selectedModel.verified
                    ? "Installed and verified"
                    : selectedModel.partialBytes > 0
                      ? `Partial download · ${formatBytes(selectedModel.partialBytes)}`
                      : "Not installed"}
                </span>
              </div>
              <p className="text-muted-foreground">{selectedModel.model.description}</p>
              <div className="text-muted-foreground">
                {selectedModel.model.publisher} · {selectedModel.model.architecture} ·{" "}
                {selectedModel.model.parameterCount} parameters · {selectedModel.model.contextWindow.toLocaleString()}{" "}
                token context · {selectedModel.model.compatibility.format} · {selectedModel.model.compatibility.runtime}{" "}
                {selectedModel.model.compatibility.runtimeVersion}
              </div>
              <div className="break-all text-muted-foreground">
                Source {selectedModel.model.sourceRepository} · SHA-256 {selectedModel.model.sha256}
              </div>
              <div className="break-all text-muted-foreground">Storage {selectedModel.modelPath}</div>
            </div>

            {displayedProgress && (downloading || displayedProgress.status === "complete") && (
              <div className="grid gap-1.5" aria-live="polite">
                <div className="flex justify-between text-xs text-muted-foreground">
                  <span>{displayedProgress.status.replaceAll("_", " ")}</span>
                  <span>
                    {formatBytes(displayedProgress.completedBytes)} / {formatBytes(displayedProgress.totalBytes)} ·{" "}
                    {progressPercent}%
                  </span>
                </div>
                <progress
                  value={displayedProgress.completedBytes}
                  max={displayedProgress.totalBytes}
                  className="h-2 w-full accent-primary"
                />
                {displayedProgress.resumed && (
                  <span className="text-xs text-muted-foreground">Resumed from the retained partial download.</span>
                )}
              </div>
            )}

            {pendingPreflight && pendingPreflight.modelId === selectedModel.model.id && (
              <div
                role="alert"
                className={cn(
                  "grid gap-2 rounded-md border px-3 py-2.5 text-xs",
                  pendingPreflight.risk === "blocked"
                    ? "border-destructive/40 bg-destructive/10"
                    : "border-amber-500/40 bg-amber-500/10",
                )}
              >
                <div className="font-medium">
                  {pendingPreflight.risk === "blocked"
                    ? "Hardware-fit check blocked this operation"
                    : "Hardware-fit warning"}
                </div>
                {pendingPreflight.advisories.map((advisory) => (
                  <p key={advisory}>{advisory}</p>
                ))}
                {pendingPreflight.operation === "load" ? (
                  <p className="text-muted-foreground">
                    Available memory {formatBytes(pendingPreflight.availableMemoryBytes)} · conservative minimum{" "}
                    {formatBytes(pendingPreflight.minimumAvailableMemoryBytes)} · recommended{" "}
                    {formatBytes(pendingPreflight.recommendedAvailableMemoryBytes)}
                  </p>
                ) : (
                  <p className="text-muted-foreground">
                    Available disk {formatBytes(pendingPreflight.availableDiskBytes)} · model plus reserve{" "}
                    {formatBytes(pendingPreflight.requiredDiskBytes)}
                  </p>
                )}
                {pendingPreflight.risk === "blocked" && (
                  <label className="grid gap-1.5">
                    Advanced override reason
                    <Textarea
                      value={overrideReason}
                      onChange={(event) => setOverrideReason(event.target.value)}
                      placeholder="Explain the hardware knowledge that makes this safe in your environment."
                      maxLength={512}
                    />
                  </label>
                )}
                <div className="flex flex-wrap gap-2">
                  <Button
                    size="sm"
                    variant={pendingPreflight.risk === "blocked" ? "destructive" : "primary"}
                    disabled={pendingPreflight.risk === "blocked" && overrideReason.trim().length < 12}
                    onClick={() =>
                      void performManagedOperation(
                        pendingPreflight.operation,
                        pendingPreflight.risk === "warning",
                        pendingPreflight.risk === "blocked",
                      )
                    }
                  >
                    {pendingPreflight.risk === "blocked" ? "Use advanced override" : "Acknowledge and continue"}
                  </Button>
                  <Button variant="ghost" size="sm" onClick={() => setPendingPreflight(null)}>
                    Cancel
                  </Button>
                </div>
              </div>
            )}

            <div className="flex flex-wrap gap-2">
              {downloading ? (
                <Button variant="secondary" onClick={() => void cancelDownload()}>
                  Cancel download
                </Button>
              ) : !selectedModel.installed || !selectedModel.verified ? (
                <Button onClick={() => void prepareManagedOperation("download")}>
                  <Download className="h-4 w-4" />
                  {selectedModel.partialBytes > 0 ? "Resume and verify" : "Download and verify"}
                </Button>
              ) : status.running ? (
                <Button variant="secondary" onClick={handleStop} disabled={stopping}>
                  {stopping ? "Stopping…" : "Unload model"}
                </Button>
              ) : (
                <Button
                  onClick={() => void prepareManagedOperation("load")}
                  disabled={starting || !status.binaryVerified}
                >
                  {starting ? "Loading…" : "Load model"}
                </Button>
              )}
              {(selectedModel.installed || selectedModel.partialBytes > 0) && !status.running && (
                <Button
                  variant={deleteConfirmation ? "destructive" : "ghost"}
                  onClick={() => void deleteManagedModel()}
                  disabled={deleting || downloading}
                >
                  <Trash2 className="h-4 w-4" />
                  {deleting ? "Deleting…" : deleteConfirmation ? "Confirm delete" : "Delete local files"}
                </Button>
              )}
            </div>
          </>
        ) : (
          <p role="alert" className="text-sm text-destructive">
            No reviewed managed models are available in this build.
          </p>
        )}
      </section>

      <details className="rounded-md border border-border p-3">
        <summary className="cursor-pointer text-sm font-medium">Advanced: load a manually obtained GGUF</summary>
        <div className="mt-3 grid gap-3">
          <p className="text-xs text-muted-foreground">
            Manual files are treated as untrusted and validated before launch. Ark records their observed digest and the
            source/license you supply, but cannot compare them to the reviewed catalog.
          </p>
          <label className="grid gap-1.5 text-sm">
            Model file
            <Input
              value={modelPathDraft}
              onChange={(event) => setModelPathDraft(event.target.value)}
              placeholder="C:\\Models\\model.gguf"
              disabled={status.running || starting}
            />
          </label>
          <label className="grid gap-1.5 text-sm">
            Model source
            <Input
              value={modelSource}
              onChange={(event) => setModelSource(event.target.value)}
              placeholder="https://publisher.example/model"
              maxLength={2048}
              disabled={status.running || starting}
            />
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
          <Button
            variant="secondary"
            onClick={handleManualStart}
            disabled={
              status.running ||
              starting ||
              !modelPathDraft.trim() ||
              !modelSource.trim() ||
              !modelLicense.trim() ||
              !status.binaryVerified
            }
          >
            {starting ? "Loading…" : "Load manual model"}
          </Button>
        </div>
      </details>

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
          process for you. Development builds install the pinned binary with the verified setup script; qualified
          release packages carry that same verified runtime as an immutable application resource. Managed catalog models
          download once, verify locally, and then work without internet. For GPU acceleration, use the Ollama or Local
          Inference Host provider instead.
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
