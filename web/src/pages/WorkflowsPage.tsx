import { useCallback, useEffect, useState } from 'react';
import { useT } from '../i18n';
import { ChevronRightIcon, WorkflowsIcon } from '../components/ui/Icons';
import { PageHeader } from '../components/ui/PageHeader';
import type { MsgKey } from '../i18n/messages';
import type { Project } from '../lib/api';
import { getWorkflow, listWorkflows } from '../lib/api';
import type { Origin, WorkflowDef, WorkflowSummary } from '../lib/workflow';
import { blankWorkflow } from '../lib/workflow';
import { WorkflowEditor } from '../components/workflow/WorkflowEditor';
import { Badge, type BadgeTone } from '../components/ui/Badge';
import { Button } from '../components/ui/Button';
import { ListCard, listRowClass } from '../components/ui/ListCard';
import { Select, TextInput } from '../components/ui/Field';

interface WorkflowsPageProps {
  projects: Project[];
  projectId: string | null;
  workflowId: string | null;
  isNew: boolean;
}

const ID_PATTERN = /^[a-z0-9][a-z0-9-]*$/;

const ORIGIN_LABELS: Record<Origin, MsgKey> = {
  project: 'workflowOriginProject',
  global: 'workflowOriginGlobal',
  default: 'workflowOriginDefault'
};

const ORIGIN_TONES: Record<Origin, BadgeTone> = { project: 'accent', global: 'ok', default: 'info' };

function OriginBadge({ origin }: { origin: Origin }) {
  const t = useT();
  return <Badge tone={ORIGIN_TONES[origin]} appearance="pill">{t(ORIGIN_LABELS[origin])}</Badge>;
}

function WorkflowEditorView({ projectId, workflowId, isNew }: { projectId: string | null; workflowId: string; isNew: boolean }) {
  const t = useT();
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
        if (!cancelled) setError(err instanceof Error ? err.message : t('workflowLoadError'));
      });
    return () => {
      cancelled = true;
    };
  }, [projectId, workflowId, isNew, t]);

  if (error) return <div className="p-6 text-err">{error}</div>;
  if (!initial) return <div className="p-6 text-muted">{t('loading')}</div>;
  return <WorkflowEditor initialDef={initial.def} initialOrigin={initial.origin} projectId={projectId} />;
}

export default function WorkflowsPage({ projects, projectId, workflowId, isNew }: WorkflowsPageProps) {
  const t = useT();
  // The projects load after the first render: the default is derived, not frozen at mount.
  const [chosenProjectId, setSelectedProjectId] = useState<string | null>(projectId ?? null);
  const selectedProjectId = chosenProjectId ?? projects[0]?.id ?? null;
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
        if (!cancelled) setListError(error instanceof Error ? error.message : t('workflowListError'));
      });
    return () => {
      cancelled = true;
    };
  }, [selectedProjectId, t]);

  const handleNewWorkflow = useCallback(
    (event: React.FormEvent) => {
      event.preventDefault();
      const id = newId.trim();
      if (!ID_PATTERN.test(id)) {
        setNewIdError(t('workflowIdPatternError'));
        return;
      }
      if (!selectedProjectId) return;
      window.location.hash = `#/workflows/${encodeURIComponent(selectedProjectId)}/${encodeURIComponent(id)}?new`;
    },
    [newId, selectedProjectId, t]
  );

  const selectedProject = projects.find((project) => project.id === selectedProjectId);

  if (workflowId) {
    return <WorkflowEditorView projectId={selectedProjectId} workflowId={workflowId} isNew={isNew} />;
  }

  return (
    <main className="grow overflow-y-auto px-4 py-6 md:px-6 md:py-8">
      <div className="mx-auto max-w-page">
        <PageHeader
          Icon={WorkflowsIcon}
          title={t('workflows')}
          subtitle={t('workflowsPageDescription')}
          actions={
            <Select
              aria-label={t('workflowProjectLabel')}
              value={selectedProjectId ?? ''}
              onChange={(event) => setSelectedProjectId(event.target.value || null)}
              className="w-40"
            >
              {projects.map((project) => (
                <option key={project.id} value={project.id}>
                  {project.name}
                </option>
              ))}
            </Select>
          }
        />

        <form onSubmit={handleNewWorkflow} className="mt-6 flex items-center gap-2">
          <TextInput
            type="text"
            value={newId}
            onChange={(event) => {
              setNewId(event.target.value);
              setNewIdError(null);
            }}
            placeholder={t('workflowNewIdPlaceholder')}
            className="min-w-0 flex-1 sm:max-w-xs"
          />
          <Button variant="primary" size="md" type="submit" disabled={!selectedProjectId} className="shrink-0">
            {t('workflowNewButton')}
          </Button>
        </form>
        {newIdError && <div className="mt-1 text-xs text-err">{newIdError}</div>}

        {selectedProjectId && (
          <div className="mt-4">
            {listError && <div className="text-err">{listError}</div>}
            {workflows === null && !listError && <div className="text-muted">{t('loading')}</div>}
            {workflows && workflows.length === 0 && <div className="text-muted">{t('workflowNoneYet')}</div>}
            {workflows && workflows.length > 0 && selectedProjectId && (
              <ListCard title={selectedProject?.name ?? t('workflows')} count={workflows.length}>
                {workflows.map((wf, index) => (
                  <button
                    key={wf.id}
                    type="button"
                    onClick={() => {
                      window.location.hash = `#/workflows/${encodeURIComponent(selectedProjectId)}/${encodeURIComponent(wf.id)}`;
                    }}
                    className={listRowClass({}, `group w-full text-left text-sm ${index > 0 ? 'border-t border-border' : ''}`)}
                  >
                    <span className="grid size-8 shrink-0 place-items-center rounded-md bg-accent-soft text-accent">
                      <WorkflowsIcon className="h-4 w-4" />
                    </span>
                    <span className="min-w-0 grow">
                      <span className="block truncate text-sm font-medium group-hover:text-accent">{wf.name}</span>
                      {wf.description && <span className="block truncate text-xs text-muted">{wf.description}</span>}
                    </span>
                    <OriginBadge origin={wf.origin} />
                    <ChevronRightIcon className="h-3.5 w-3.5 shrink-0 text-faint group-hover:text-accent" />
                  </button>
                ))}
              </ListCard>
            )}
          </div>
        )}
      </div>
    </main>
  );
}
