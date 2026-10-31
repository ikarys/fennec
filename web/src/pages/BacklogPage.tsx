import { useCallback, useEffect, useState } from 'react';
import { getBacklog, type BacklogIdea, type BacklogQueue, type BacklogView } from '../lib/api';
import { startQueue, stopQueue } from '../lib/ideas';
import { formatRelative } from '../lib/format';
import { Badge } from '../components/ui/Badge';
import { Card } from '../components/ui/Card';
import { SectionLabel } from '../components/ui/SectionLabel';
import { Button, buttonClass } from '../components/ui/Button';
import { EyeIcon, MoonIcon, PlayIcon, StopIcon } from '../components/Icons';
import { IdeaMeta } from '../components/project/ideas/IdeaMeta';
import { IdeaStatusBadge } from '../components/project/ideas/IdeaStatusBadge';

function headerSummary(view: BacklogView): string {
  const parts: string[] = [];

  if (view.total.yours > 0) parts.push(`${view.total.yours} à toi`);
  if (view.total.running > 0) parts.push(`${view.total.running} en cours`);
  if (view.total.ready > 0)
    parts.push(`${view.total.ready} prête${view.total.ready > 1 ? 's' : ''}`);
  if (view.total.done > 0)
    parts.push(`${view.total.done} terminée${view.total.done > 1 ? 's' : ''}`);

  parts.push(`sur ${view.total.projects} projet${view.total.projects > 1 ? 's' : ''}`);
  return parts.join(' · ');
}

function IdeaRow({ idea, caps }: { idea: BacklogIdea; caps: BacklogView['caps'] }) {
  return (
    <li className="flex items-start gap-2 py-2">
      <div className="min-w-0 grow">
        <a
          href={`#/project/${encodeURIComponent(idea.project.id)}/tasks`}
          className="text-xs text-muted hover:text-accent-strong"
        >
          {idea.project.name}
        </a>
        <p className="truncate text-sm" title={idea.text}>
          {idea.text}
        </p>
        <div className="mt-1 flex flex-wrap items-center gap-2 text-xs">
          <IdeaStatusBadge status={idea.status} />
          <IdeaMeta idea={idea} caps={caps} />
        </div>
      </div>
      {idea.sessionId ? (
        <a
          href={'#/s/' + encodeURIComponent(idea.sessionId)}
          title="Ouvrir la session"
          aria-label="Ouvrir la session"
          className={buttonClass({ variant: 'ghost', size: 'sm', iconOnly: true })}
        >
          <EyeIcon className="h-4 w-4" />
        </a>
      ) : null}
    </li>
  );
}

/** A project's queue, startable (by day or in night mode) or stoppable from here. */
function QueueRow({
  queue,
  first,
  onChanged,
}: {
  queue: BacklogQueue;
  first: boolean;
  onChanged: () => void;
}) {
  const [busy, setBusy] = useState(false);
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
    <li
      className={`flex flex-wrap items-center gap-x-3 gap-y-1 py-2 ${
        first ? '' : 'border-t border-border'
      }`}
    >
      <a
        href={`#/project/${encodeURIComponent(queue.project.id)}/tasks/ready`}
        className="font-medium text-text hover:text-accent-strong"
      >
        {queue.project.name}
      </a>
      {queue.running ? (
        <>
          <Badge tone="accent" dot pulse>
            file en cours
          </Badge>
          {queue.autonomous ? (
            <span className="flex items-center gap-1.5 text-xs text-muted">
              <MoonIcon className="h-3.5 w-3.5" />
              mode nuit
            </span>
          ) : null}
        </>
      ) : (
        <Badge tone="neutral">à l'arrêt</Badge>
      )}
      <span className="text-xs text-muted tabular-nums">
        {queue.ready} prête{queue.ready > 1 ? 's' : ''}
      </span>
      {queue.next ? (
        <span className="min-w-0 flex-1 truncate text-xs text-muted" title={queue.next.text}>
          Prochaine : {queue.next.text}
        </span>
      ) : null}
      <div className="ml-auto flex shrink-0 items-center gap-1">
        {queue.running ? (
          <Button
            variant="secondary"
            size="sm"
            iconOnly
            disabled={busy}
            title="Arrêter la file après la tâche en cours"
            aria-label="Arrêter la file"
            onClick={() => act(() => stopQueue(queue.project.id))}
          >
            <StopIcon className="h-4 w-4" />
          </Button>
        ) : (
          <>
            <Button
              variant="ghost"
              size="sm"
              iconOnly
              disabled={busy}
              title="Lancer la file en mode nuit : l'agent tranche seul ses questions"
              aria-label="Lancer la file en mode nuit"
              onClick={() => act(() => startQueue(queue.project.id, true))}
            >
              <MoonIcon className="h-4 w-4" />
            </Button>
            <Button
              variant="secondary"
              size="sm"
              iconOnly
              disabled={busy}
              title="Lancer la file"
              aria-label="Lancer la file"
              onClick={() => act(() => startQueue(queue.project.id, false))}
            >
              <PlayIcon className="h-4 w-4" />
            </Button>
          </>
        )}
      </div>
    </li>
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

  const isEmpty =
    view !== null &&
    view.yours.length === 0 &&
    view.running.length === 0 &&
    view.queues.length === 0 &&
    view.done.length === 0;

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
          <div className="mt-4 flex flex-col gap-4">
            {view.yours.length > 0 ? (
              <Card className="p-4">
                <SectionLabel>À toi</SectionLabel>
                <ul className="mt-2 flex flex-col">
                  {view.yours.map((idea) => (
                    <IdeaRow key={idea.id} idea={idea} caps={view.caps} />
                  ))}
                </ul>
              </Card>
            ) : null}

            {view.running.length > 0 ? (
              <Card className="p-4">
                <SectionLabel>En cours</SectionLabel>
                <ul className="mt-2 flex flex-col">
                  {view.running.map((idea) => (
                    <IdeaRow key={idea.id} idea={idea} caps={view.caps} />
                  ))}
                </ul>
              </Card>
            ) : null}

            {view.queues.length > 0 ? (
              <Card className="p-4">
                <SectionLabel>Files</SectionLabel>
                <ul className="mt-2 flex flex-col">
                  {view.queues.map((queue, index) => (
                    <QueueRow key={queue.project.id} queue={queue} first={index === 0} onChanged={load} />
                  ))}
                </ul>
              </Card>
            ) : null}

            {view.done.length > 0 ? (
              <Card className="p-4">
                <SectionLabel>Terminées récemment</SectionLabel>
                <ul className="mt-2 flex flex-col">
                  {view.done.map((idea) => (
                    <li key={idea.id} className="flex items-start gap-2 py-2">
                      <IdeaStatusBadge status={idea.status} />
                      <div className="min-w-0 grow">
                        <p className="line-clamp-2 md:line-clamp-1 text-sm" title={idea.text}>
                          {idea.text}
                        </p>
                        <div className="mt-0.5 flex items-center gap-2 text-xs">
                          <a
                            href={`#/project/${encodeURIComponent(idea.project.id)}/tasks`}
                            className="text-muted hover:text-accent-strong"
                          >
                            {idea.project.name}
                          </a>
                          <span className="text-faint">{formatRelative(idea.updatedAt)}</span>
                        </div>
                      </div>
                    </li>
                  ))}
                </ul>
                {view.total.done > view.done.length ? (
                  <p className="mt-2 text-xs text-muted">{view.total.done} au total</p>
                ) : null}
              </Card>
            ) : null}

            {isEmpty ? (
              <Card className="p-4">
                <p className="text-sm text-muted">
                  Aucune tâche pour l'instant : les idées s'ajoutent depuis la page d'un projet.
                </p>
              </Card>
            ) : null}
          </div>
        ) : null}
      </div>
    </main>
  );
}
