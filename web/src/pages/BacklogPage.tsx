import { useCallback, useEffect, useState } from 'react';
import { getBacklog, type BacklogProject, type BacklogView } from '../lib/api';
import { startQueue, stopQueue } from '../lib/ideas';
import { Badge } from '../components/ui/Badge';
import { Card } from '../components/ui/Card';
import { Button } from '../components/ui/Button';
import { MoonIcon, PlayIcon, StopIcon } from '../components/Icons';

function headerSummary(view: BacklogView): string {
  const parts: string[] = [];

  if (view.total.yours > 0) parts.push(`${view.total.yours} à décider`);
  if (view.total.running > 0) parts.push(`${view.total.running} en cours`);
  if (view.total.ready > 0)
    parts.push(`${view.total.ready} prête${view.total.ready > 1 ? 's' : ''}`);
  if (view.total.done > 0)
    parts.push(`${view.total.done} terminée${view.total.done > 1 ? 's' : ''}`);

  parts.push(`sur ${view.total.projects} projet${view.total.projects > 1 ? 's' : ''}`);
  return parts.join(' · ');
}

function Count({
  value,
  href,
  attention = false,
}: {
  value: number;
  href: string;
  attention?: boolean;
}) {
  if (value === 0) {
    return <span className="text-faint">0</span>;
  }

  return (
    <a
      href={href}
      className={`hover:underline ${attention ? 'font-semibold text-accent-strong' : 'text-text'}`}
    >
      {value}
    </a>
  );
}

function ProjectRow({
  row,
  first,
  onChanged,
}: {
  row: BacklogProject;
  first: boolean;
  onChanged: () => void;
}) {
  const [busy, setBusy] = useState(false);
  const projectId = row.project.id;

  const act = (action: () => Promise<unknown>) => {
    setBusy(true);
    action()
      .catch(() => undefined)
      .finally(() => {
        setBusy(false);
        onChanged();
      });
  };

  return (
    <tr className={`hover:bg-raised ${first ? '' : 'border-t border-border'}`}>
      <td className="px-3 py-2">
        <a
          href={`#/project/${encodeURIComponent(projectId)}/tasks`}
          className="font-medium text-text hover:text-accent-strong"
        >
          {row.project.name}
        </a>
        {row.queue.next ? (
          <span
            className="block max-w-[22rem] truncate text-xs text-muted"
            title={row.queue.next.text}
          >
            Prochaine : {row.queue.next.title}
          </span>
        ) : null}
      </td>
      <td className="px-3 py-2 text-right">
        <Count
          value={row.yours}
          href={`#/project/${encodeURIComponent(projectId)}/tasks/yours`}
          attention
        />
      </td>
      <td className="px-3 py-2 text-right">
        <Count
          value={row.running}
          href={`#/project/${encodeURIComponent(projectId)}/tasks`}
        />
      </td>
      <td className="px-3 py-2 text-right">
        <Count
          value={row.ready}
          href={`#/project/${encodeURIComponent(projectId)}/tasks/ready`}
        />
      </td>
      <td className="px-3 py-2 text-right">
        <Count
          value={row.done}
          href={`#/project/${encodeURIComponent(projectId)}/tasks/done`}
        />
      </td>
      <td className="whitespace-nowrap px-3 py-2 text-right">
        <div className="flex items-center justify-end gap-1">
          {row.queue.running ? (
            <>
              <Badge tone="accent" dot pulse>
                en cours
              </Badge>
              {row.queue.autonomous ? (
                <span className="text-muted" title="mode nuit">
                  <MoonIcon className="h-3.5 w-3.5" />
                </span>
              ) : null}
              <Button
                variant="secondary"
                size="sm"
                iconOnly
                disabled={busy}
                title="Arrêter la file après la tâche en cours"
                aria-label="Arrêter la file après la tâche en cours"
                onClick={() => act(() => stopQueue(projectId))}
              >
                <StopIcon className="h-4 w-4" />
              </Button>
            </>
          ) : (
            <>
              <Button
                variant="ghost"
                size="sm"
                iconOnly
                disabled={row.ready === 0 || busy}
                title="Lancer la file en mode nuit : l'agent tranche seul ses questions"
                aria-label="Lancer la file en mode nuit : l'agent tranche seul ses questions"
                onClick={() => act(() => startQueue(projectId, true))}
              >
                <MoonIcon className="h-4 w-4" />
              </Button>
              <Button
                variant="secondary"
                size="sm"
                iconOnly
                disabled={row.ready === 0 || busy}
                title="Lancer la file"
                aria-label="Lancer la file"
                onClick={() => act(() => startQueue(projectId, false))}
              >
                <PlayIcon className="h-4 w-4" />
              </Button>
            </>
          )}
        </div>
      </td>
    </tr>
  );
}

export default function BacklogPage() {
  const [view, setView] = useState<BacklogView | null>(null);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(() => {
    getBacklog()
      .then((data) => {
        setView(data);
        setError(null);
      })
      .catch((fetchError) => {
        setError(fetchError instanceof Error ? fetchError.message : 'Impossible de charger le backlog');
      });
  }, []);

  useEffect(load, [load]);

  return (
    <main className="grow overflow-y-auto px-4 py-6 md:px-6 md:py-8">
      <div className="mx-auto max-w-[960px]">
        <div>
          <h1 className="text-xl font-semibold">Backlog</h1>
          {view ? <p className="mt-0.5 text-sm text-muted">{headerSummary(view)}</p> : null}
        </div>

        {error ? <p className="mt-4 text-err">{error}</p> : null}
        {!view && !error ? <div className="mt-4 text-muted">Chargement…</div> : null}

        {view ? (
          <div className="mt-4">
            {view.projects.length === 0 ? (
              <Card className="p-4">
                <p className="text-sm text-muted">
                  Aucune tâche pour l'instant : les idées s'ajoutent depuis la page d'un projet.
                </p>
              </Card>
            ) : (
              <Card className="overflow-hidden">
                <table className="w-full text-sm tabular-nums">
                  <thead>
                    <tr>
                      <th className="px-3 py-2 text-left text-xs font-medium text-muted">Projet</th>
                      <th className="px-3 py-2 text-right text-xs font-medium text-muted">À décider</th>
                      <th className="px-3 py-2 text-right text-xs font-medium text-muted">En cours</th>
                      <th className="px-3 py-2 text-right text-xs font-medium text-muted">Prêtes</th>
                      <th className="px-3 py-2 text-right text-xs font-medium text-muted">Terminées</th>
                      <th className="px-3 py-2 text-right text-xs font-medium text-muted">File</th>
                    </tr>
                  </thead>
                  <tbody>
                    {view.projects.map((row, index) => (
                      <ProjectRow
                        key={row.project.id}
                        row={row}
                        first={index === 0}
                        onChanged={load}
                      />
                    ))}
                  </tbody>
                </table>
              </Card>
            )}
          </div>
        ) : null}
      </div>
    </main>
  );
}
