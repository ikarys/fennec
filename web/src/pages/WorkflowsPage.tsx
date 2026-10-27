import { useCallback, useEffect, useState } from 'react';
import type { Project } from '../lib/api';
import { getWorkflow, listWorkflows } from '../lib/api';
import type { Origin, WorkflowDef, WorkflowSummary } from '../lib/workflow';
import { blankWorkflow } from '../lib/workflow';
import { WorkflowEditor } from '../components/workflow/WorkflowEditor';

interface WorkflowsPageProps {
  projects: Project[];
  projectId: string | null;
  workflowId: string | null;
  isNew: boolean;
}

const ID_PATTERN = /^[a-z0-9][a-z0-9-]*$/;

const ORIGIN_LABELS: Record<Origin, string> = {
  project: 'Projet',
  global: 'Global',
  default: 'Par défaut'
};

function OriginBadge({ origin }: { origin: Origin }) {
  const styles: Record<Origin, string> = {
    project: 'bg-accent-soft text-accent',
    global: 'bg-ok-soft text-ok',
    default: 'bg-raised text-muted'
  };
  return <span className={`rounded-md px-1.5 py-0.5 text-[0.6875rem] font-medium ${styles[origin]}`}>{ORIGIN_LABELS[origin]}</span>;
}

function WorkflowEditorView({ projectId, workflowId, isNew }: { projectId: string | null; workflowId: string; isNew: boolean }) {
  const [initial, setInitial] = useState<{ def: WorkflowDef; origin: Origin } | null>(
    isNew ? { def: blankWorkflow(workflowId), origin: 'project' } : null
  );
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (isNew || !projectId) return;
    let cancelled = false;
    getWorkflow(projectId, workflowId)
      .then((data) => {
        if (!cancelled) setInitial({ def: data.workflow, origin: data.origin });
      })
      .catch((err) => {
        if (!cancelled) setError(err instanceof Error ? err.message : 'Impossible de charger le workflow');
      });
    return () => {
      cancelled = true;
    };
  }, [projectId, workflowId, isNew]);

  if (error) return <div className="p-6 text-err">{error}</div>;
  if (!initial) return <div className="p-6 text-muted">Chargement…</div>;
  return <WorkflowEditor initialDef={initial.def} initialOrigin={initial.origin} projectId={projectId} />;
}

export default function WorkflowsPage({ projects, projectId, workflowId, isNew }: WorkflowsPageProps) {
  const [selectedProjectId, setSelectedProjectId] = useState<string | null>(projectId ?? projects[0]?.id ?? null);
  const [workflows, setWorkflows] = useState<WorkflowSummary[] | null>(null);
  const [listError, setListError] = useState<string | null>(null);
  const [newId, setNewId] = useState('');
  const [newIdError, setNewIdError] = useState<string | null>(null);

  useEffect(() => {
    if (projectId) {
      setSelectedProjectId(projectId);
    }
  }, [projectId]);

  useEffect(() => {
    if (!selectedProjectId) {
      setWorkflows(null);
      return;
    }
    let cancelled = false;
    setWorkflows(null);
    setListError(null);
    listWorkflows(selectedProjectId)
      .then((data) => {
        if (!cancelled) setWorkflows(data);
      })
      .catch((error) => {
        if (!cancelled) setListError(error instanceof Error ? error.message : 'Impossible de charger les workflows');
      });
    return () => {
      cancelled = true;
    };
  }, [selectedProjectId]);

  const handleNewWorkflow = useCallback(
    (event: React.FormEvent) => {
      event.preventDefault();
      const id = newId.trim();
      if (!ID_PATTERN.test(id)) {
        setNewIdError("L'identifiant doit correspondre à ^[a-z0-9][a-z0-9-]*$");
        return;
      }
      if (!selectedProjectId) return;
      window.location.hash = `#/workflows/${encodeURIComponent(selectedProjectId)}/${encodeURIComponent(id)}?new`;
    },
    [newId, selectedProjectId]
  );

  if (workflowId) {
    return <WorkflowEditorView projectId={selectedProjectId} workflowId={workflowId} isNew={isNew} />;
  }

  return (
    <main className="grow overflow-y-auto px-6 py-8">
      <div className="mx-auto max-w-[720px]">
        <h1 className="text-[1.375rem] font-semibold tracking-[-0.02em]">Workflows</h1>
        <p className="mt-0.5 text-muted">Dessinez un graphe, reliez ses ports, enregistrez-le dans le projet ou en global.</p>

        <div className="mt-6 flex items-center gap-3">
          <label htmlFor="wf-project" className="text-[0.8125rem] text-muted">
            Projet
          </label>
          <select
            id="wf-project"
            value={selectedProjectId ?? ''}
            onChange={(event) => setSelectedProjectId(event.target.value || null)}
            className="rounded-lg border border-border bg-panel px-2 py-1 text-[0.8125rem]"
          >
            {projects.map((project) => (
              <option key={project.id} value={project.id}>
                {project.name}
              </option>
            ))}
          </select>
        </div>

        {selectedProjectId && (
          <div className="mt-4">
            {listError && <div className="text-err">{listError}</div>}
            {workflows === null && !listError && <div className="text-muted">Chargement…</div>}
            {workflows && workflows.length === 0 && <div className="text-muted">Aucun workflow pour l’instant.</div>}
            {workflows && workflows.length > 0 && selectedProjectId && (
              <ul className="flex flex-col gap-1">
                {workflows.map((wf) => (
                  <li key={wf.id}>
                    <button
                      type="button"
                      onClick={() => {
                        window.location.hash = `#/workflows/${encodeURIComponent(selectedProjectId)}/${encodeURIComponent(wf.id)}`;
                      }}
                      className="flex w-full items-center gap-3 rounded-lg px-3 py-2 text-left hover:bg-raised"
                    >
                      <span className="min-w-0 grow">
                        <span className="block text-[0.8125rem] font-medium">{wf.name}</span>
                        {wf.description && <span className="block truncate text-xs text-muted">{wf.description}</span>}
                      </span>
                      <OriginBadge origin={wf.origin} />
                    </button>
                  </li>
                ))}
              </ul>
            )}
          </div>
        )}

        <form onSubmit={handleNewWorkflow} className="mt-6 flex items-center gap-2">
          <input
            type="text"
            value={newId}
            onChange={(event) => {
              setNewId(event.target.value);
              setNewIdError(null);
            }}
            placeholder="nouveau-workflow"
            className="rounded-lg border border-border bg-panel px-2 py-1 text-[0.8125rem]"
          />
          <button
            type="submit"
            disabled={!selectedProjectId}
            className="rounded-lg bg-accent px-3 py-1 text-[0.8125rem] font-medium text-on-accent disabled:opacity-60"
          >
            Nouveau workflow
          </button>
        </form>
        {newIdError && <div className="mt-1 text-xs text-err">{newIdError}</div>}
      </div>
    </main>
  );
}
