import {
  Activity,
  ArrowLeft,
  Bot,
  Code2,
  FolderTree,
  GitCompare,
  Minimize2,
  Plus,
  Send,
  Square,
  Terminal,
  User,
  Wrench,
  X,
} from "lucide-react";
import * as React from "react";
import { ActivityIndicator } from "../../ui/activityIndicator";
import { getErrorMessage, normalizeError } from "../../lib/arkErrors";
import {
  CODE_TIMELINE_RUN_PAGE_SIZE,
  classifyCodeInvocation,
  codeClarificationQuestion,
  codeInvocationStateLabel,
  codeProposalLabel,
  windowCodeRuns,
} from "../../lib/codeTimeline";
import { useArkClient } from "../../lib/useArkClient";
import { entityCollection, entityList, type CodeState } from "../../state/arkStores";
import { useStore } from "../../state/externalStore";
import { useArkStores } from "../../state/useArkStores";
import type {
  CodeRepositorySupport,
  CodeRunDetail,
  CodeRunState,
  ModelInfo,
  Project,
  ProviderConfig,
  RepositorySearchResult,
} from "../../types/ark";
import { Badge } from "../../ui/badge";
import { Button } from "../../ui/button";
import { Input } from "../../ui/input";
import { Select } from "../../ui/select";
import { Textarea } from "../../ui/textarea";

interface CodeViewProps {
  projects: Project[];
  providers: ProviderConfig[];
  models: ModelInfo[];
  onBack: () => void;
  onError: (message: string | null) => void;
}

const TERMINAL_RUN_STATES: CodeRunState[] = ["completed", "failed", "cancelled", "interrupted"];

function runStateTone(state: CodeRunState): "success" | "warning" | "danger" | "muted" {
  if (state === "completed") return "success";
  if (state === "failed" || state === "interrupted" || state === "cancelled") return "danger";
  if (state === "queued") return "muted";
  return "warning";
}

function EditDiff({ diff }: { diff: string }) {
  return (
    <pre className="max-h-80 overflow-auto rounded-md border border-border bg-background p-3 text-xs">
      {diff.split("\n").map((line, index) => (
        <span
          // Diff lines are not stable entities; their position within this immutable preview is.
          key={index}
          className={`block whitespace-pre-wrap break-words ${
            line.startsWith("+ ")
              ? "bg-emerald-500/10 text-emerald-700 dark:text-emerald-300"
              : line.startsWith("- ")
                ? "bg-red-500/10 text-red-700 dark:text-red-300"
                : "text-muted-foreground"
          }`}
        >
          {line || " "}
        </span>
      ))}
    </pre>
  );
}

export function CodeView({ projects, providers, models, onBack, onError }: CodeViewProps) {
  const client = useArkClient();
  const stores = useArkStores();
  const code = useStore(stores.code);
  const boundProjects = React.useMemo(() => projects.filter((project) => project.repositoryPath), [projects]);
  const [projectId, setProjectId] = React.useState(boundProjects[0]?.id ?? "");
  const [title, setTitle] = React.useState("");
  const [runDetails, setRunDetails] = React.useState<CodeRunDetail[]>([]);
  const [visibleRunCount, setVisibleRunCount] = React.useState(CODE_TIMELINE_RUN_PAGE_SIZE);
  const visibleRunDetails = React.useMemo(
    () => windowCodeRuns(runDetails, visibleRunCount),
    [runDetails, visibleRunCount],
  );
  const runDetail = runDetails[runDetails.length - 1] ?? null;
  const activeRunId = runDetail?.run.id;
  const activeRunState = runDetail?.run.state;
  const [runBusy, setRunBusy] = React.useState(false);
  const [decisionBusyId, setDecisionBusyId] = React.useState<string | null>(null);
  const [taskText, setTaskText] = React.useState(code.composerDraft);
  const [gitSetupState, setGitSetupState] = React.useState<"required" | "initialized" | null>(null);
  const [supportPane, setSupportPane] = React.useState<"repository" | "changes" | "output" | "run" | null>(null);
  const [repositorySupport, setRepositorySupport] = React.useState<CodeRepositorySupport | null>(null);
  const [repositorySearch, setRepositorySearch] = React.useState("");
  const [repositorySearchResult, setRepositorySearchResult] = React.useState<RepositorySearchResult | null>(null);
  const composerRef = React.useRef<HTMLTextAreaElement>(null);
  const timelineRef = React.useRef<HTMLDivElement>(null);
  const prependAnchorRef = React.useRef<{ height: number; top: number } | null>(null);
  const restoreAttemptedRef = React.useRef(false);
  const toolCapableModels = React.useMemo(
    () => models.filter((model) => model.isAvailable && model.toolCallingMode !== "unsupported"),
    [models],
  );
  const enabledProviders = React.useMemo(() => providers.filter((provider) => provider.isEnabled), [providers]);
  const [runProviderId, setRunProviderId] = React.useState(enabledProviders[0]?.id ?? "");
  const runModelsForProvider = React.useMemo(
    () => toolCapableModels.filter((model) => model.providerId === runProviderId),
    [toolCapableModels, runProviderId],
  );
  const [runModelId, setRunModelId] = React.useState(runModelsForProvider[0]?.name ?? "");

  React.useLayoutEffect(() => {
    const anchor = prependAnchorRef.current;
    const timeline = timelineRef.current;
    if (!anchor || !timeline) return;
    timeline.scrollTop = anchor.top + (timeline.scrollHeight - anchor.height);
    prependAnchorRef.current = null;
  }, [visibleRunDetails.length]);

  React.useEffect(() => {
    if (runModelsForProvider.some((model) => model.name === runModelId)) return;
    setRunModelId(runModelsForProvider[0]?.name ?? "");
  }, [runModelId, runModelsForProvider]);

  const patchCode = React.useCallback(
    (patch: Partial<CodeState>) => stores.code.set({ ...stores.code.getSnapshot(), ...patch }),
    [stores.code],
  );

  const selectSession = React.useCallback(
    async (id: string) => {
      patchCode({ activeId: id, isLoading: true });
      setRunDetails([]);
      setVisibleRunCount(CODE_TIMELINE_RUN_PAGE_SIZE);
      try {
        const detail = await client.getCodeSession(id);
        patchCode({ detail, isLoading: false });
        setProjectId(detail.session.projectId);
        const details = await Promise.all(detail.runs.map((run) => client.getCodeRunDetail(run.id)));
        setRunDetails(details);
        const latest = details[details.length - 1];
        if (latest?.run.state === "queued") {
          const started = await client.startCodeAgentRun(id, latest.run.id);
          setRunDetails((current) => current.map((item) => (item.run.id === started.run.id ? started : item)));
        }
      } catch (error) {
        patchCode({ isLoading: false });
        onError(getErrorMessage(error));
      }
    },
    [client, onError, patchCode],
  );

  React.useEffect(() => {
    patchCode({ composerDraft: taskText });
  }, [patchCode, taskText]);

  const loadSessions = React.useCallback(async () => {
    patchCode({ isLoading: true });
    try {
      const sessions = await client.listCodeSessions(false);
      patchCode({ sessions: entityCollection(sessions), isLoading: false });
    } catch (error) {
      patchCode({ isLoading: false });
      onError(getErrorMessage(error));
    }
  }, [client, onError, patchCode]);

  React.useEffect(() => {
    void loadSessions();
  }, [loadSessions]);

  React.useEffect(() => {
    if (
      restoreAttemptedRef.current ||
      code.isLoading ||
      !code.activeId ||
      !code.sessions.byId[code.activeId] ||
      runDetails.length > 0
    )
      return;
    restoreAttemptedRef.current = true;
    void selectSession(code.activeId);
  }, [code.activeId, code.isLoading, code.sessions.byId, runDetails.length, selectSession]);

  React.useEffect(() => {
    if (runDetails.length === 0) return;
    if (timelineRef.current) timelineRef.current.scrollTop = code.scrollTop;
    if (code.composerFocused) window.requestAnimationFrame(() => composerRef.current?.focus());
  }, [code.composerFocused, code.scrollTop, runDetails.length]);

  async function createSession() {
    const normalizedTitle = title.trim();
    if (!projectId || !normalizedTitle) return;
    try {
      const session = await client.createCodeSession({
        projectId,
        title: normalizedTitle,
        idempotencyKey: crypto.randomUUID(),
      });
      setTitle("");
      patchCode({ sessions: entityCollection([session, ...entityList(stores.code.getSnapshot().sessions)]) });
      await selectSession(session.id);
    } catch (error) {
      onError(getErrorMessage(error));
    }
  }

  const activeSession = code.activeId ? code.sessions.byId[code.activeId] : undefined;
  const activeProjectId = activeSession?.projectId ?? projectId;

  React.useEffect(() => {
    if (!activeSession || !activeRunId) return;
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void client
      .onCodeRunUpdated((event) => {
        if (disposed || event.sessionId !== activeSession.id || event.runId !== activeRunId) return;
        void client
          .getCodeRunDetail(event.runId)
          .then((detail) => {
            if (!disposed) {
              setRunDetails((current) => current.map((item) => (item.run.id === detail.run.id ? detail : item)));
            }
          })
          .catch((error) => {
            if (!disposed) onError(getErrorMessage(error));
          });
      })
      .then((stop) => {
        if (disposed) stop();
        else unlisten = stop;
      });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [activeRunId, activeSession, client, onError]);

  // Events are refetch notifications, not the source of truth. Reconcile any active run from
  // SQLite as a fallback for a notification emitted before this view subscribed, a suspended
  // webview, or a transient listener failure.
  React.useEffect(() => {
    if (!activeSession || !activeRunId || !activeRunState || TERMINAL_RUN_STATES.includes(activeRunState)) return;
    let disposed = false;
    let timer: number | undefined;

    const refresh = async () => {
      try {
        const detail = await client.getCodeRunDetail(activeRunId);
        if (disposed) return;
        setRunDetails((current) => current.map((item) => (item.run.id === detail.run.id ? detail : item)));
        if (!TERMINAL_RUN_STATES.includes(detail.run.state)) {
          timer = window.setTimeout(() => void refresh(), 750);
        }
      } catch (error) {
        if (!disposed) onError(getErrorMessage(error));
      }
    };

    timer = window.setTimeout(() => void refresh(), 500);
    return () => {
      disposed = true;
      if (timer !== undefined) window.clearTimeout(timer);
    };
  }, [activeRunId, activeRunState, activeSession, client, onError]);

  React.useEffect(() => {
    if (runDetail?.run.terminalReason === "clarification_requested") {
      window.requestAnimationFrame(() => composerRef.current?.focus());
    }
  }, [runDetail?.run.terminalReason]);

  async function startRun() {
    const normalizedTask = taskText.trim();
    if (
      runBusy ||
      !activeSession ||
      !runProviderId ||
      !runModelId ||
      !normalizedTask ||
      (runDetail && !TERMINAL_RUN_STATES.includes(runDetail.run.state))
    )
      return;
    setRunBusy(true);
    try {
      const run = await client.createCodeRun({
        sessionId: activeSession.id,
        parentRunId: runDetail && TERMINAL_RUN_STATES.includes(runDetail.run.state) ? runDetail.run.id : null,
        providerId: runProviderId,
        modelId: runModelId,
        task: normalizedTask,
        idempotencyKey: crypto.randomUUID(),
      });
      const detail = await client.startCodeAgentRun(activeSession.id, run.id);
      setRunDetails((current) => [...current, detail]);
      setTaskText("");
      setGitSetupState(null);
    } catch (error) {
      if (normalizeError(error).code === "git_repository_required") setGitSetupState("required");
      onError(getErrorMessage(error));
    } finally {
      setRunBusy(false);
    }
  }

  async function initializeGitRepository() {
    if (!activeProjectId) return;
    setRunBusy(true);
    try {
      await client.initializeProjectGitRepository(activeProjectId);
      setGitSetupState("initialized");
    } catch (error) {
      onError(getErrorMessage(error));
    } finally {
      setRunBusy(false);
    }
  }

  async function openSupportPane(pane: "repository" | "changes" | "output" | "run") {
    if (supportPane === pane) {
      setSupportPane(null);
      return;
    }
    setSupportPane(pane);
    if ((pane === "repository" || pane === "changes") && runDetail) {
      try {
        setRepositorySupport(await client.getCodeRunRepositorySupport(runDetail.run.id));
      } catch (error) {
        onError(getErrorMessage(error));
      }
    }
  }

  async function searchRepository() {
    if (!runDetail || !repositorySearch.trim()) return;
    try {
      setRepositorySearchResult(await client.searchCodeRunRepository(runDetail.run.id, repositorySearch.trim()));
    } catch (error) {
      onError(getErrorMessage(error));
    }
  }

  async function cancelRun() {
    if (!activeSession || !runDetail) return;
    setRunBusy(true);
    try {
      const detail = await client.cancelCodeAgentRun(activeSession.id, runDetail.run.id);
      setRunDetails((current) => current.map((item) => (item.run.id === detail.run.id ? detail : item)));
    } catch (error) {
      onError(getErrorMessage(error));
    } finally {
      setRunBusy(false);
    }
  }

  async function steerRun() {
    const instruction = taskText.trim();
    if (!activeSession || !runDetail || !instruction || runBusy) return;
    setRunBusy(true);
    try {
      let stopped = await client.cancelCodeAgentRun(activeSession.id, runDetail.run.id);
      for (let attempt = 0; attempt < 100 && !TERMINAL_RUN_STATES.includes(stopped.run.state); attempt += 1) {
        await new Promise((resolve) => window.setTimeout(resolve, 100));
        stopped = await client.getCodeRunDetail(runDetail.run.id);
      }
      if (!TERMINAL_RUN_STATES.includes(stopped.run.state)) {
        throw new Error("Ark Code did not acknowledge cancellation before the steering deadline.");
      }
      const run = await client.createCodeRun({
        sessionId: activeSession.id,
        parentRunId: stopped.run.id,
        providerId: runProviderId,
        modelId: runModelId,
        task: instruction,
        idempotencyKey: crypto.randomUUID(),
      });
      const next = await client.startCodeAgentRun(activeSession.id, run.id);
      setRunDetails((current) => [...current.map((item) => (item.run.id === stopped.run.id ? stopped : item)), next]);
      setTaskText("");
    } catch (error) {
      onError(getErrorMessage(error));
    } finally {
      setRunBusy(false);
    }
  }

  async function decideTool(invocationId: string, approve: boolean, focusComposer = false) {
    if (!activeSession || !runDetail || decisionBusyId) return;
    const invocation = runDetail.invocations.find((item) => item.id === invocationId);
    if (!invocation || invocation.state !== "proposed") return;
    setDecisionBusyId(invocationId);
    try {
      const detail = approve
        ? await client.codeApproveEdit({
            sessionId: activeSession.id,
            runId: runDetail.run.id,
            invocationId,
            callHash: invocation.callHash,
            previewHash: invocation.previewHash ?? "",
            preconditionHash: invocation.preconditionHash ?? "",
          })
        : await client.codeRejectEdit({
            sessionId: activeSession.id,
            runId: runDetail.run.id,
            invocationId,
            callHash: invocation.callHash,
          });
      setRunDetails((current) => current.map((item) => (item.run.id === detail.run.id ? detail : item)));
      if (focusComposer) window.requestAnimationFrame(() => composerRef.current?.focus());
    } catch (error) {
      onError(getErrorMessage(error));
      try {
        const current = await client.getCodeRunDetail(runDetail.run.id);
        setRunDetails((details) => details.map((item) => (item.run.id === current.run.id ? current : item)));
      } catch {
        // Keep the original actionable error; polling will reconcile transient refresh failures.
      }
    } finally {
      setDecisionBusyId(null);
    }
  }

  return (
    <main className="min-w-0 flex-1 overflow-hidden flex flex-col bg-background">
      <header className="shrink-0 z-10 flex min-h-14 items-center gap-3 border-b border-border bg-card/95 px-4 backdrop-blur">
        <Button variant="ghost" size="icon" onClick={onBack} aria-label="Back to Ark Chat">
          <ArrowLeft className="h-4 w-4" />
        </Button>
        <Code2 className="h-5 w-5 text-primary" />
        <div>
          <h1 className="text-sm font-semibold">Ark Code</h1>
          <p className="text-xs text-muted-foreground">Repository investigation and approved edits</p>
        </div>
      </header>

      <div className="mx-auto grid w-full max-w-6xl gap-5 p-4 flex-1 min-h-0 overflow-hidden lg:grid-cols-[280px_minmax(0,1fr)]">
        <aside className="min-h-0 overflow-y-auto space-y-4 rounded-lg border border-border bg-card p-4">
          <div>
            <h2 className="text-sm font-semibold">Sessions</h2>
            <p className="mt-1 text-xs text-muted-foreground">
              Persistent Ark Code work belongs to an existing Project.
            </p>
          </div>
          {boundProjects.length === 0 ? (
            <p className="text-sm text-muted-foreground">Bind a Repository to a Project in Settings first.</p>
          ) : (
            <div className="space-y-2">
              <Select className="w-full" value={projectId} onChange={(event) => setProjectId(event.target.value)}>
                {boundProjects.map((project) => (
                  <option key={project.id} value={project.id}>
                    {project.name}
                  </option>
                ))}
              </Select>
              <Input
                value={title}
                onChange={(event) => setTitle(event.target.value)}
                placeholder="Session title"
                maxLength={120}
              />
              <Button className="w-full justify-start" onClick={() => void createSession()} disabled={!title.trim()}>
                <Plus className="h-4 w-4" /> New session
              </Button>
            </div>
          )}
          <div className="space-y-1">
            {entityList(code.sessions).map((session) => (
              <button
                key={session.id}
                type="button"
                onClick={() => void selectSession(session.id)}
                className={`w-full rounded-md px-3 py-2 text-left text-sm outline-none focus-visible:ring-2 focus-visible:ring-ring ${
                  code.activeId === session.id
                    ? "bg-primary/15 text-foreground"
                    : "text-muted-foreground hover:bg-muted"
                }`}
              >
                <span className="block truncate font-medium">{session.title}</span>
              </button>
            ))}
            {!code.isLoading && code.sessions.ids.length === 0 && (
              <p className="py-3 text-xs text-muted-foreground">No Ark Code sessions yet.</p>
            )}
          </div>
        </aside>

        <section className="flex min-h-0 min-w-0 flex-col overflow-hidden rounded-lg border border-border bg-card">
          {!activeSession ? (
            <div className="grid flex-1 place-items-center p-8 text-center text-sm text-muted-foreground">
              Create or select a coding session, then describe the result you want.
            </div>
          ) : (
            <>
              <div className="border-b border-border px-5 py-3">
                <div className="flex items-center justify-between gap-3">
                  <div className="min-w-0">
                    <h2 className="truncate font-semibold">{activeSession.title}</h2>
                    <p className="truncate text-xs text-muted-foreground">
                      {projects.find((project) => project.id === activeProjectId)?.repositoryPath}
                    </p>
                  </div>
                  {runDetail && <Badge tone={runStateTone(runDetail.run.state)}>{runDetail.run.state}</Badge>}
                </div>
                {runDetail && (
                  <nav className="mt-3 flex flex-wrap gap-1" aria-label="Ark Code supporting views">
                    <Button
                      size="sm"
                      variant={supportPane === "repository" ? "secondary" : "ghost"}
                      onClick={() => void openSupportPane("repository")}
                    >
                      <FolderTree className="h-4 w-4" /> Repository
                    </Button>
                    <Button
                      size="sm"
                      variant={supportPane === "changes" ? "secondary" : "ghost"}
                      onClick={() => void openSupportPane("changes")}
                    >
                      <GitCompare className="h-4 w-4" /> Changes
                    </Button>
                    <Button
                      size="sm"
                      variant={supportPane === "output" ? "secondary" : "ghost"}
                      onClick={() => void openSupportPane("output")}
                    >
                      <Terminal className="h-4 w-4" /> Output
                    </Button>
                    <Button
                      size="sm"
                      variant={supportPane === "run" ? "secondary" : "ghost"}
                      onClick={() => void openSupportPane("run")}
                    >
                      <Activity className="h-4 w-4" /> Run
                    </Button>
                  </nav>
                )}
              </div>

              {supportPane && runDetail && (
                <section
                  className="max-h-72 overflow-auto border-b border-border bg-muted/20 p-4 text-xs"
                  aria-label={`${supportPane} supporting view`}
                >
                  <div className="mb-3 flex items-center justify-between">
                    <h3 className="font-medium capitalize">{supportPane}</h3>
                    <Button
                      size="icon"
                      variant="ghost"
                      aria-label="Close supporting view"
                      onClick={() => setSupportPane(null)}
                    >
                      <X className="h-4 w-4" />
                    </Button>
                  </div>
                  {supportPane === "repository" && (
                    <div className="space-y-3">
                      <form
                        className="flex gap-2"
                        onSubmit={(event) => {
                          event.preventDefault();
                          void searchRepository();
                        }}
                      >
                        <Input
                          value={repositorySearch}
                          onChange={(event) => setRepositorySearch(event.target.value)}
                          placeholder="Search isolated Repository"
                          aria-label="Search isolated Repository"
                        />
                        <Button type="submit" size="sm" disabled={!repositorySearch.trim()}>
                          Search
                        </Button>
                      </form>
                      {repositorySearchResult ? (
                        <div className="space-y-1">
                          {repositorySearchResult.matches.map((match) => (
                            <p key={`${match.path}:${match.lineNumber}`}>
                              <code>
                                {match.path}:{match.lineNumber}
                              </code>{" "}
                              {match.line}
                            </p>
                          ))}
                        </div>
                      ) : (
                        <div className="grid grid-cols-2 gap-x-4 gap-y-1">
                          {repositorySupport?.repositoryMap.entries.map((entry) => (
                            <code key={entry.path} className="truncate">
                              {entry.path}
                            </code>
                          ))}
                        </div>
                      )}
                    </div>
                  )}
                  {supportPane === "changes" && (
                    <div className="space-y-3">
                      <pre className="whitespace-pre-wrap break-words">
                        {repositorySupport?.gitStatus.porcelain || "Working tree clean"}
                      </pre>
                      <EditDiff
                        diff={
                          [repositorySupport?.gitDiff.staged, repositorySupport?.gitDiff.workingTree]
                            .filter(Boolean)
                            .join("\n") || "No uncommitted diff"
                        }
                      />
                    </div>
                  )}
                  {supportPane === "output" && (
                    <div className="space-y-2">
                      {runDetail.observations
                        .filter((observation) =>
                          runDetail.invocations.some(
                            (invocation) =>
                              invocation.stepId === observation.stepId &&
                              invocation.toolName === "run_verification_command",
                          ),
                        )
                        .map((observation) => (
                          <pre
                            key={observation.id}
                            className="max-h-52 overflow-auto whitespace-pre-wrap break-words rounded-md border border-border bg-background p-2"
                          >
                            {observation.content}
                          </pre>
                        ))}
                      {!runDetail.invocations.some(
                        (invocation) => invocation.toolName === "run_verification_command",
                      ) && <p className="text-muted-foreground">No verification command output yet.</p>}
                    </div>
                  )}
                  {supportPane === "run" && (
                    <div className="grid gap-2 sm:grid-cols-2">
                      <p>
                        Steps: {runDetail.run.stepsUsed} / {runDetail.run.maxSteps}
                      </p>
                      <p>
                        Tokens: {runDetail.run.actualTokens} / {runDetail.run.maxTokens}
                      </p>
                      <p>
                        Active time: {Math.round(runDetail.run.activeElapsedMs / 1000)}s /{" "}
                        {Math.round(runDetail.run.maxActiveMs / 1000)}s
                      </p>
                      <p>Recovery: {runDetail.run.recoveryOutcome ?? "none"}</p>
                      <details className="sm:col-span-2">
                        <summary className="cursor-pointer font-medium">Lifecycle diagnostics</summary>
                        <ol className="mt-2 space-y-1">
                          {runDetail.events.map((event) => (
                            <li key={event.sequence}>
                              {event.sequence}. {event.kind}: {event.summary}
                            </li>
                          ))}
                        </ol>
                      </details>
                    </div>
                  )}
                </section>
              )}

              <div
                ref={timelineRef}
                onScroll={(event) => patchCode({ scrollTop: event.currentTarget.scrollTop })}
                role="log"
                aria-label="Ark Code conversation"
                aria-live="polite"
                className="flex-1 space-y-5 overflow-y-auto bg-background p-5"
              >
                {runDetails.length === 0 && (
                  <div className="mx-auto max-w-md py-16 text-center">
                    <Code2 className="mx-auto h-8 w-8 text-muted-foreground" />
                    <h3 className="mt-3 font-medium">What should Ark Code build or investigate?</h3>
                    <p className="mt-1 text-sm text-muted-foreground">
                      Ark Code will inspect this Repository automatically and report its work here.
                    </p>
                  </div>
                )}

                {visibleRunDetails.length < runDetails.length && (
                  <Button
                    type="button"
                    variant="ghost"
                    className="mx-auto"
                    onClick={() => {
                      const timeline = timelineRef.current;
                      if (timeline) {
                        prependAnchorRef.current = {
                          height: timeline.scrollHeight,
                          top: timeline.scrollTop,
                        };
                      }
                      setVisibleRunCount((count) => count + CODE_TIMELINE_RUN_PAGE_SIZE);
                    }}
                  >
                    Load earlier activity
                  </Button>
                )}

                {visibleRunDetails.map((detail) => (
                  <React.Fragment key={detail.run.id}>
                    <article className="ml-auto flex max-w-[85%] items-start gap-2">
                      <div className="rounded-xl rounded-tr-sm bg-primary px-4 py-3 text-sm text-primary-foreground">
                        {detail.run.task}
                      </div>
                      <User className="mt-2 h-4 w-4 shrink-0 text-muted-foreground" aria-hidden="true" />
                    </article>

                    {detail.steps.map((step) => {
                      const invocation = detail.invocations.find((item) => item.stepId === step.id);
                      const observations = detail.observations.filter((item) => item.stepId === step.id);
                      return (
                        <div key={step.id} className="space-y-2">
                          {step.streamingText && (
                            <article
                              className="flex max-w-[90%] items-start gap-2"
                              aria-label="Ark Code response streaming"
                            >
                              <Bot className="mt-2 h-4 w-4 shrink-0 text-primary" aria-hidden="true" />
                              <div className="whitespace-pre-wrap rounded-xl rounded-tl-sm border border-border bg-card px-4 py-3 text-sm text-foreground">
                                {step.streamingText}
                                <span
                                  className="ml-1 inline-block h-4 w-0.5 animate-pulse bg-primary align-text-bottom"
                                  aria-hidden="true"
                                />
                              </div>
                            </article>
                          )}
                          {invocation && classifyCodeInvocation(invocation) === "clarification" ? (
                            <article className="flex max-w-[90%] items-start gap-2" role="status">
                              <Bot className="mt-2 h-4 w-4 shrink-0 text-primary" aria-hidden="true" />
                              <div className="rounded-xl rounded-tl-sm border border-border bg-card px-4 py-3 text-sm text-foreground">
                                <p className="mb-1 text-xs font-medium text-muted-foreground">
                                  Ark Code needs clarification
                                </p>
                                {codeClarificationQuestion(invocation.canonicalArgumentsJson) ??
                                  "Please clarify how Ark Code should continue."}
                              </div>
                            </article>
                          ) : invocation && classifyCodeInvocation(invocation) === "approval" ? (
                            <section className="space-y-3 rounded-lg border border-border bg-card p-3 text-xs">
                              <div className="flex items-center gap-2 font-medium text-foreground">
                                <Wrench className="h-3.5 w-3.5" aria-hidden="true" />
                                {codeProposalLabel(invocation.toolName)}
                                <Badge
                                  tone={
                                    invocation.state === "applied"
                                      ? "success"
                                      : invocation.state === "denied" || invocation.state === "interrupted"
                                        ? "danger"
                                        : "warning"
                                  }
                                >
                                  {codeInvocationStateLabel(invocation)}
                                </Badge>
                              </div>
                              {invocation.toolName === "run_verification_command" ? (
                                <pre className="max-h-80 overflow-auto whitespace-pre-wrap break-words rounded-md border border-border bg-background p-3 text-xs text-muted-foreground">
                                  {invocation.preview ?? ""}
                                </pre>
                              ) : (
                                <EditDiff diff={invocation.preview ?? ""} />
                              )}
                              {invocation.state === "proposed" && (
                                <div className="flex justify-end gap-2">
                                  <Button
                                    variant="ghost"
                                    onClick={() => void decideTool(invocation.id, false)}
                                    disabled={decisionBusyId === invocation.id}
                                  >
                                    Reject
                                  </Button>
                                  <Button
                                    variant="secondary"
                                    onClick={() => void decideTool(invocation.id, false, true)}
                                    disabled={decisionBusyId === invocation.id}
                                  >
                                    Revise instruction
                                  </Button>
                                  <Button
                                    onClick={() => void decideTool(invocation.id, true)}
                                    disabled={
                                      decisionBusyId === invocation.id ||
                                      !invocation.previewHash ||
                                      !invocation.preconditionHash
                                    }
                                  >
                                    Approve
                                  </Button>
                                </div>
                              )}
                              {invocation.verificationOutcome && (
                                <p className="text-muted-foreground">
                                  Verification: {invocation.verificationOutcome.replaceAll("_", " ")}
                                </p>
                              )}
                            </section>
                          ) : invocation ? (
                            <details className="rounded-lg border border-border bg-card px-3 py-2 text-xs">
                              <summary className="flex cursor-pointer list-none items-center gap-2 font-medium text-foreground">
                                <Wrench className="h-3.5 w-3.5" aria-hidden="true" />
                                {invocation.toolName}
                                <Badge tone={invocation.state === "applied" ? "success" : "warning"}>
                                  {codeInvocationStateLabel(invocation)}
                                </Badge>
                              </summary>
                              <pre className="mt-2 max-h-52 overflow-auto whitespace-pre-wrap break-words border-t border-border pt-2 text-muted-foreground">
                                {invocation.canonicalArgumentsJson}
                              </pre>
                            </details>
                          ) : null}
                          {observations.map((observation) =>
                            observation.kind === "model_text" ? (
                              <article key={observation.id} className="flex max-w-[90%] items-start gap-2">
                                <Bot className="mt-2 h-4 w-4 shrink-0 text-primary" aria-hidden="true" />
                                <div className="whitespace-pre-wrap rounded-xl rounded-tl-sm border border-border bg-card px-4 py-3 text-sm text-foreground">
                                  {observation.content}
                                </div>
                              </article>
                            ) : observation.kind === "system" ? (
                              <div
                                key={observation.id}
                                role="status"
                                className="flex items-start gap-2 rounded-lg border border-border/80 bg-muted/30 px-3 py-2 text-xs text-muted-foreground"
                              >
                                <Minimize2 className="mt-0.5 h-3.5 w-3.5 shrink-0" aria-hidden="true" />
                                <span>
                                  <strong className="font-medium text-foreground">Context compacted.</strong>{" "}
                                  {observation.content}
                                </span>
                              </div>
                            ) : observation.kind === "completion_rejected" ? (
                              <div
                                key={observation.id}
                                role="status"
                                className="flex items-start gap-2 rounded-lg border border-amber-500/30 bg-amber-500/5 px-3 py-2 text-xs text-amber-700 dark:text-amber-400"
                              >
                                <span className="mt-0.5 shrink-0 font-medium">↩</span>
                                <span>
                                  <strong className="font-medium">Response not accepted.</strong> {observation.content}
                                </span>
                              </div>
                            ) : (
                              <details
                                key={observation.id}
                                className="rounded-lg border border-border/80 bg-muted/30 px-3 py-2 text-xs"
                              >
                                <summary className="cursor-pointer font-medium text-muted-foreground">
                                  {observation.kind === "tool_error" ? "Tool error" : "Tool result"}
                                </summary>
                                <pre className="mt-2 max-h-64 overflow-auto whitespace-pre-wrap break-words text-muted-foreground">
                                  {observation.content}
                                </pre>
                              </details>
                            ),
                          )}
                        </div>
                      );
                    })}

                    {!TERMINAL_RUN_STATES.includes(detail.run.state) && (
                      <ActivityIndicator
                        state={
                          detail.run.state === "awaiting_approval"
                            ? "approval"
                            : detail.run.state === "executing_tool"
                              ? "tool"
                              : detail.run.state === "observing"
                                ? "provider"
                                : "preparing"
                        }
                      />
                    )}
                    {detail.run.terminalReason && (
                      <div className="rounded-md border border-border bg-card px-3 py-2 text-xs text-muted-foreground">
                        Stopped: {detail.run.terminalReason.replaceAll("_", " ")}
                      </div>
                    )}
                  </React.Fragment>
                ))}
              </div>

              <form
                className="space-y-2 border-t border-border bg-card p-4"
                onSubmit={(event) => {
                  event.preventDefault();
                  void startRun();
                }}
              >
                <Textarea
                  ref={composerRef}
                  onFocus={() => patchCode({ composerFocused: true })}
                  onBlur={() => patchCode({ composerFocused: false })}
                  value={taskText}
                  onChange={(event) => setTaskText(event.target.value)}
                  placeholder={runDetail ? "Continue this coding conversation…" : "Describe the result you want…"}
                  rows={3}
                  aria-label="Message Ark Code"
                  onKeyDown={(event) => {
                    if (event.key !== "Enter" || event.shiftKey || event.nativeEvent.isComposing) return;
                    event.preventDefault();
                    if (runDetail && !TERMINAL_RUN_STATES.includes(runDetail.run.state)) void steerRun();
                    else void startRun();
                  }}
                />
                <div className="flex flex-wrap items-center gap-2">
                  <Select
                    className="min-w-40 flex-1"
                    value={runProviderId}
                    disabled={Boolean(runDetail && !TERMINAL_RUN_STATES.includes(runDetail.run.state))}
                    aria-label="Ark Code provider"
                    onChange={(event) => {
                      setRunProviderId(event.target.value);
                      setRunModelId("");
                    }}
                  >
                    {enabledProviders.map((provider) => (
                      <option key={provider.id} value={provider.id}>
                        {provider.name}
                      </option>
                    ))}
                  </Select>
                  <Select
                    className="min-w-40 flex-1"
                    value={runModelId}
                    disabled={Boolean(runDetail && !TERMINAL_RUN_STATES.includes(runDetail.run.state))}
                    aria-label="Ark Code model"
                    onChange={(event) => setRunModelId(event.target.value)}
                  >
                    {runModelsForProvider.map((model) => (
                      <option key={model.id} value={model.name}>
                        {model.displayName ?? model.name}
                      </option>
                    ))}
                  </Select>
                  {runDetail && !TERMINAL_RUN_STATES.includes(runDetail.run.state) ? (
                    <>
                      {taskText.trim() && (
                        <Button type="button" disabled={runBusy} onClick={() => void steerRun()}>
                          <Send className="h-4 w-4" /> Stop &amp; steer
                        </Button>
                      )}
                      <Button type="button" variant="secondary" disabled={runBusy} onClick={() => void cancelRun()}>
                        <Square className="h-4 w-4" /> Stop
                      </Button>
                    </>
                  ) : (
                    <>
                      {runDetail && runDetail.run.state !== "completed" && (
                        <Button
                          type="button"
                          variant="ghost"
                          disabled={runBusy}
                          onClick={() => {
                            setTaskText(runDetail.run.task);
                            window.requestAnimationFrame(() => composerRef.current?.focus());
                          }}
                        >
                          Retry instruction
                        </Button>
                      )}
                      <Button type="submit" disabled={runBusy || !taskText.trim() || !runProviderId || !runModelId}>
                        <Send className="h-4 w-4" /> {runDetail ? "Continue" : "Send"}
                      </Button>
                    </>
                  )}
                </div>
                {enabledProviders.length === 0 && (
                  <p className="text-xs text-muted-foreground">
                    Enable a provider with an available, tool-capable model in Settings first.
                  </p>
                )}
                {gitSetupState === "required" && (
                  <div className="flex flex-wrap items-center justify-between gap-2 rounded-md border border-border bg-muted/30 p-3 text-xs text-muted-foreground">
                    <span>This Project is not a Git Repository. Initialization changes only its Git metadata.</span>
                    <Button
                      type="button"
                      variant="secondary"
                      disabled={runBusy}
                      onClick={() => void initializeGitRepository()}
                    >
                      Initialize Git
                    </Button>
                  </div>
                )}
                {gitSetupState === "initialized" && (
                  <p className="rounded-md border border-border bg-muted/30 p-3 text-xs text-muted-foreground">
                    Git is initialized. Create the Repository&apos;s first commit, then send this request again; Ark
                    will never commit your existing work without review.
                  </p>
                )}
              </form>
            </>
          )}
        </section>
      </div>
    </main>
  );
}
