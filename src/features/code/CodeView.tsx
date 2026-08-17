import { ArrowLeft, Code2, FileSearch, FolderTree, GitCompare, GitCommitHorizontal, Plus } from "lucide-react";
import * as React from "react";
import { getErrorMessage } from "../../lib/arkErrors";
import { useArkClient } from "../../lib/useArkClient";
import { entityCollection, entityList, type CodeState } from "../../state/arkStores";
import { useStore } from "../../state/externalStore";
import { useArkStores } from "../../state/useArkStores";
import type { Project } from "../../types/ark";
import { Button } from "../../ui/button";
import { Input } from "../../ui/input";
import { Select } from "../../ui/select";

interface CodeViewProps {
  projects: Project[];
  onBack: () => void;
  onError: (message: string | null) => void;
}

interface ToolCard {
  id: string;
  title: string;
  content: string;
}

export function CodeView({ projects, onBack, onError }: CodeViewProps) {
  const client = useArkClient();
  const stores = useArkStores();
  const code = useStore(stores.code);
  const boundProjects = React.useMemo(() => projects.filter((project) => project.repositoryPath), [projects]);
  const [projectId, setProjectId] = React.useState(boundProjects[0]?.id ?? "");
  const [title, setTitle] = React.useState("");
  const [searchQuery, setSearchQuery] = React.useState("");
  const [filePath, setFilePath] = React.useState("");
  const [cards, setCards] = React.useState<ToolCard[]>([]);
  const [toolBusy, setToolBusy] = React.useState(false);

  const patchCode = React.useCallback(
    (patch: Partial<CodeState>) => stores.code.set({ ...stores.code.getSnapshot(), ...patch }),
    [stores.code],
  );

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

  async function selectSession(id: string) {
    patchCode({ activeId: id, isLoading: true });
    try {
      const detail = await client.getCodeSession(id);
      patchCode({ detail, isLoading: false });
      setProjectId(detail.session.projectId);
    } catch (error) {
      patchCode({ isLoading: false });
      onError(getErrorMessage(error));
    }
  }

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

  async function runTool(title: string, operation: () => Promise<unknown>) {
    setToolBusy(true);
    try {
      const result = await operation();
      setCards((current) => [{ id: crypto.randomUUID(), title, content: JSON.stringify(result, null, 2) }, ...current]);
    } catch (error) {
      onError(getErrorMessage(error));
    } finally {
      setToolBusy(false);
    }
  }

  const activeSession = code.activeId ? code.sessions.byId[code.activeId] : undefined;
  const activeProjectId = activeSession?.projectId ?? projectId;

  return (
    <main className="min-w-0 flex-1 overflow-y-auto bg-background">
      <header className="sticky top-0 z-10 flex min-h-14 items-center gap-3 border-b border-border bg-card/95 px-4 backdrop-blur">
        <Button variant="ghost" size="icon" onClick={onBack} aria-label="Back to Ark Chat">
          <ArrowLeft className="h-4 w-4" />
        </Button>
        <Code2 className="h-5 w-5 text-primary" />
        <div>
          <h1 className="text-sm font-semibold">Ark Code</h1>
          <p className="text-xs text-muted-foreground">Read-only Repository investigation</p>
        </div>
      </header>

      <div className="mx-auto grid w-full max-w-6xl gap-5 p-4 lg:grid-cols-[280px_minmax(0,1fr)]">
        <aside className="space-y-4 rounded-lg border border-border bg-card p-4">
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

        <section className="min-w-0 space-y-4">
          {!activeSession ? (
            <div className="rounded-lg border border-dashed border-border p-8 text-center text-sm text-muted-foreground">
              Create or select a session to inspect its bound Repository.
            </div>
          ) : (
            <>
              <div className="rounded-lg border border-border bg-card p-4">
                <h2 className="font-semibold">{activeSession.title}</h2>
                <p className="mt-1 break-all text-xs text-muted-foreground">
                  {projects.find((project) => project.id === activeProjectId)?.repositoryPath}
                </p>
                <div className="mt-4 flex flex-wrap gap-2">
                  <Button
                    variant="secondary"
                    disabled={toolBusy}
                    onClick={() => void runTool("Repository map", () => client.codeRepositoryMap(activeProjectId))}
                  >
                    <FolderTree className="h-4 w-4" /> Repository map
                  </Button>
                  <Button
                    variant="secondary"
                    disabled={toolBusy}
                    onClick={() => void runTool("Git status", () => client.codeGitStatus(activeProjectId))}
                  >
                    <GitCommitHorizontal className="h-4 w-4" /> Git status
                  </Button>
                  <Button
                    variant="secondary"
                    disabled={toolBusy}
                    onClick={() => void runTool("Git diff", () => client.codeGitDiff(activeProjectId))}
                  >
                    <GitCompare className="h-4 w-4" /> Git diff
                  </Button>
                </div>
                <div className="mt-3 flex gap-2">
                  <Input
                    value={searchQuery}
                    onChange={(event) => setSearchQuery(event.target.value)}
                    placeholder="Search Repository text"
                    maxLength={256}
                  />
                  <Button
                    variant="secondary"
                    disabled={toolBusy || !searchQuery.trim()}
                    onClick={() =>
                      void runTool(`Search: ${searchQuery}`, () =>
                        client.codeSearch({ projectId: activeProjectId, query: searchQuery }),
                      )
                    }
                  >
                    <FileSearch className="h-4 w-4" /> Search
                  </Button>
                </div>
                <div className="mt-3 flex gap-2">
                  <Input
                    value={filePath}
                    onChange={(event) => setFilePath(event.target.value)}
                    placeholder="Relative file path, e.g. src/main.rs"
                  />
                  <Button
                    variant="secondary"
                    disabled={toolBusy || !filePath.trim()}
                    onClick={() =>
                      void runTool(`Read: ${filePath}`, () =>
                        client.codeReadFile({ projectId: activeProjectId, path: filePath }),
                      )
                    }
                  >
                    Read file
                  </Button>
                </div>
              </div>

              {cards.map((card) => (
                <article key={card.id} className="overflow-hidden rounded-lg border border-border bg-card">
                  <h3 className="border-b border-border px-4 py-2 text-sm font-medium">{card.title}</h3>
                  <pre className="max-h-96 overflow-auto whitespace-pre-wrap break-words p-4 text-xs text-muted-foreground">
                    {card.content}
                  </pre>
                </article>
              ))}
            </>
          )}
        </section>
      </div>
    </main>
  );
}
