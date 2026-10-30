import { useEffect, useState } from 'react';
import { getBacklog, type BacklogProject, type BacklogView } from '../lib/api';
import { formatRelative } from '../lib/format';
import { Badge } from '../components/ui/Badge';
import { Card } from '../components/ui/Card';

function ProjectRow({ project }: { project: BacklogProject }) {
  const href = `#/project/${encodeURIComponent(project.id)}/tasks`;
  return (
    <Card className="p-4">
      <div className="flex items-center justify-between gap-3">
        <a
          href={href}
          title={project.name}
          className="min-w-0 truncate text-[1.0625rem] font-semibold text-text hover:text-accent-strong"
        >
          {project.name}
        </a>
        <Badge tone={project.doneCount > 0 ? 'ok' : 'neutral'}>
          {project.doneCount} terminée{project.doneCount > 1 ? 's' : ''}
        </Badge>
      </div>

      {project.tasks.length > 0 ? (
        <ul className="mt-3 space-y-1.5">
          {project.tasks.map((task) => (
            <li key={task.id} className="flex items-baseline gap-2 text-sm">
              <Badge tone={task.status === 'integrated' ? 'ok' : 'neutral'} dot>
                {task.status === 'integrated' ? 'intégré' : 'complété'}
              </Badge>
              <span className="min-w-0 flex-1 truncate" title={task.text}>
                {task.text}
              </span>
              <span className="shrink-0 text-xs text-muted">{formatRelative(task.updatedAt)}</span>
            </li>
          ))}
        </ul>
      ) : (
        <p className="mt-3 text-sm text-muted">Aucune tâche terminée.</p>
      )}
    </Card>
  );
}

export default function BacklogPage() {
  const [view, setView] = useState<BacklogView | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    setView(null);
    setError(null);
    getBacklog()
      .then((data) => {
        if (!cancelled) setView(data);
      })
      .catch((fetchError) => {
        if (!cancelled) {
          setError(fetchError instanceof Error ? fetchError.message : 'Impossible de charger le backlog');
        }
      });
    return () => {
      cancelled = true;
    };
  }, []);

  return (
    <div className="min-h-0 grow overflow-y-auto bg-bg">
      <div className="mx-auto flex w-full max-w-5xl flex-col gap-6 p-6">
        <div>
          <h1 className="text-xl font-semibold">Backlog</h1>
          {view ? (
            <p className="mt-0.5 text-sm text-muted">
              {view.total.done} tâche{view.total.done > 1 ? 's' : ''} terminée
              {view.total.done > 1 ? 's' : ''} sur {view.total.projects} projet
              {view.total.projects > 1 ? 's' : ''}
            </p>
          ) : null}
        </div>

        {error ? <p className="text-err">{error}</p> : null}
        {!view && !error ? <div className="text-muted">Chargement…</div> : null}

        {view ? (
          <div className="flex flex-col gap-3">
            {view.projects.map((project) => (
              <ProjectRow key={project.id} project={project} />
            ))}
          </div>
        ) : null}
      </div>
    </div>
  );
}
