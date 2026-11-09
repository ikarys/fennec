import { useCallback, useEffect, useState } from 'react';
import { getBacklog, type BacklogProject, type BacklogView } from '../lib/api';
import { startQueue, stopQueue } from '../lib/ideas';
import { Badge, type BadgeTone } from '../components/ui/Badge';
import { Card } from '../components/ui/Card';
import { Button } from '../components/ui/Button';
import { BacklogIcon, MoonIcon, PlayIcon, StopIcon } from '../components/ui/Icons';
import { useT } from '../i18n';
import { PageHeader } from '../components/ui/PageHeader';
import type { MsgKey } from '../i18n/messages';

function headerSummary(
  view: BacklogView,
  t: (key: MsgKey, vars?: Record<string, string | number>) => string
): string {
  const parts: string[] = [];

  if (view.total.yours > 0) parts.push(t('backlogYoursCount', { count: view.total.yours }));
  if (view.total.running > 0) parts.push(t('backlogRunningCount', { count: view.total.running }));
  if (view.total.ready > 0)
    parts.push(t('backlogReadyCount', { count: view.total.ready }));
  if (view.total.done > 0)
    parts.push(t('backlogDoneCount', { count: view.total.done }));

  parts.push(t('backlogProjectsWithTasks', { count: view.total.projects }));
  return parts.join(' · ');
}

function Count({
  value,
  href,
  tone,
}: {
  value: number;
  href: string;
  tone: BadgeTone;
}) {
  if (value === 0) {
    return <span className="text-faint">0</span>;
  }

  return (
    <a href={href} className="inline-flex">
      <Badge tone={tone} appearance="pill">
        {value}
      </Badge>
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
  const t = useT();
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
    <tr className={`transition-colors group hover:bg-accent-soft ${first ? '' : 'border-t border-border'}`}>
      <td className="px-3 py-2 pl-4">
        <div className="flex items-center gap-3">
          <span className="grid size-8 shrink-0 place-items-center rounded-md bg-accent-soft text-accent">
            <BacklogIcon className="h-4 w-4" />
          </span>
          <div className="min-w-0">
            <a
              href={`#/project/${encodeURIComponent(projectId)}/tasks`}
              className="font-medium text-text hover:text-accent-strong group-hover:text-accent"
            >
              {row.project.name}
            </a>
            {row.queue.next ? (
              <span
                className="block max-w-[22rem] truncate text-xs text-muted"
                title={row.queue.next.text}
              >
                {t('nextLabel', { title: row.queue.next.title })}
              </span>
            ) : null}
          </div>
        </div>
      </td>
      <td className="px-3 py-2 text-right">
        <Count
          value={row.yours}
          href={`#/project/${encodeURIComponent(projectId)}/tasks/yours`}
          tone="attention"
        />
      </td>
      <td className="px-3 py-2 text-right">
        <Count
          value={row.running}
          href={`#/project/${encodeURIComponent(projectId)}/tasks`}
          tone="accent"
        />
      </td>
      <td className="px-3 py-2 text-right">
        <Count
          value={row.ready}
          href={`#/project/${encodeURIComponent(projectId)}/tasks/ready`}
          tone="info"
        />
      </td>
      <td className="px-3 py-2 text-right">
        <Count
          value={row.done}
          href={`#/project/${encodeURIComponent(projectId)}/tasks/done`}
          tone="ok"
        />
      </td>
      <td className="whitespace-nowrap px-3 py-2 text-right pr-4">
        <div className="flex items-center justify-end gap-1">
          {row.queue.running ? (
            <>
              <Badge tone="accent" dot pulse>
                {t('backlogRunningBadge')}
              </Badge>
              {row.queue.autonomous ? (
                <span className="text-muted" title={t('backlogNightModeTitle')}>
                  <MoonIcon className="h-3.5 w-3.5" />
                </span>
              ) : null}
              <Button
                variant="secondary"
                size="sm"
                iconOnly
                disabled={busy}
                title={t('stopQueueAfter')}
                aria-label={t('stopQueueAfter')}
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
                title={t('backlogStartNightQueueTitle')}
                aria-label={t('backlogStartNightQueueTitle')}
                onClick={() => act(() => startQueue(projectId, true))}
              >
                <MoonIcon className="h-4 w-4" />
              </Button>
              <Button
                variant="secondary"
                size="sm"
                iconOnly
                disabled={row.ready === 0 || busy}
                title={t('startQueue')}
                aria-label={t('startQueue')}
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
  const t = useT();
  const [view, setView] = useState<BacklogView | null>(null);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(() => {
    getBacklog()
      .then((data) => {
        setView(data);
        setError(null);
      })
      .catch((fetchError) => {
        setError(fetchError instanceof Error ? fetchError.message : t('backlogLoadError'));
      });
  }, [t]);

  useEffect(load, [load]);

  return (
    <main className="grow overflow-y-auto px-4 py-6 md:px-6 md:py-8">
      <div className="mx-auto max-w-page">
        <PageHeader Icon={BacklogIcon} title={t('tasks')} subtitle={view ? headerSummary(view, t) : undefined} />

        {error ? <p className="mt-4 text-err">{error}</p> : null}
        {!view && !error ? <div className="mt-4 text-muted">{t('loading')}</div> : null}

        {view ? (
          <div className="mt-4">
            {view.projects.length === 0 ? (
              <Card className="p-4">
                <p className="text-sm text-muted">
                  {t('backlogEmpty')}
                </p>
              </Card>
            ) : (
              <Card className="overflow-hidden">
                <table className="w-full text-sm tabular-nums">
                  <thead>
                    <tr className="border-b border-border bg-raised">
                      <th className="px-3 py-2 pl-4 text-xs font-medium uppercase tracking-wide text-faint text-left">{t('homeProject')}</th>
                      <th className="px-3 py-2 text-xs font-medium uppercase tracking-wide text-faint text-right">{t('tasksTabYours')}</th>
                      <th className="px-3 py-2 text-xs font-medium uppercase tracking-wide text-faint text-right">{t('stageRunning')}</th>
                      <th className="px-3 py-2 text-xs font-medium uppercase tracking-wide text-faint text-right">{t('tasksTabReady')}</th>
                      <th className="px-3 py-2 text-xs font-medium uppercase tracking-wide text-faint text-right">{t('tasksTabDone')}</th>
                      <th className="px-3 py-2 pr-4 text-xs font-medium uppercase tracking-wide text-faint text-right">{t('queue')}</th>
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
