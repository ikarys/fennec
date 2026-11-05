/**
 * The tasks an agent proposed, offered for the backlog inside the thread.
 *
 * It sits right after the audit card so the human sees the proposed tasks where
 * they read the session, not hidden in the right-hand context panel.
 */
import { useState } from 'react';

import { acceptProposedTask, type ProposedTask } from '../../lib/api';
import { PlusIcon } from '../Icons';
import { Badge } from '../ui/Badge';
import { Button } from '../ui/Button';
import { Card } from '../ui/Card';
import { useT } from '../../i18n';

export function ProposedTasksCard({
  tasks,
  sessionId,
  projectId,
  onAccepted,
}: {
  tasks: ProposedTask[];
  sessionId: string | null;
  projectId: string | null;
  onAccepted: () => void;
}) {
  const t = useT();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  if (tasks.length === 0 || !sessionId) return null;

  const allAccepted = tasks.every((task) => task.ideaId !== null);

  if (allAccepted) {
    const count = tasks.length;
    return (
      <Card className="p-4">
        <p className="m-0 flex items-center gap-2 text-sm text-muted">
          {count === 1 ? t('taskInBacklog') : t('tasksInBacklog', { count })}
          {projectId ? (
            <a
              href={'#/project/' + encodeURIComponent(projectId) + '/tasks'}
              className="text-xs text-accent-strong underline"
            >
              {t('viewTasks')}
            </a>
          ) : null}
        </p>
      </Card>
    );
  }

  const remaining = tasks.filter((task) => task.ideaId === null);

  const acceptOne = async (callId: string) => {
    if (busy) return;
    setBusy(true);
    setError(null);
    try {
      await acceptProposedTask(sessionId, callId);
      onAccepted();
    } catch (err) {
      setError(err instanceof Error ? err.message : t('unknownError'));
    } finally {
      setBusy(false);
    }
  };

  const acceptAll = async () => {
    if (remaining.length === 0 || busy) return;
    setBusy(true);
    setError(null);
    try {
      for (const task of remaining) {
        await acceptProposedTask(sessionId, task.callId);
        onAccepted();
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : t('unknownError'));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Card className="flex flex-col gap-3 p-4">
      <div className="flex flex-col gap-1">
        <p className="m-0 text-sm font-medium text-text">{t('whichTasksQuestion')}</p>
        <p className="m-0 text-xs text-muted">
          {t('proposedTasksHint')}
        </p>
      </div>
      <ul className="m-0 flex list-none flex-col gap-2 p-0">
        {tasks.map((task, index) => (
          <li key={task.callId} className="flex items-center gap-2 text-sm">
            <span className="tabular-nums text-xs text-muted">{index + 1}.</span>
            <span className="min-w-0 text-text" title={task.text}>
              {task.title}
            </span>
            {task.ideaId ? (
              <Badge tone="ok">{t('inBacklogBadge')}</Badge>
            ) : (
              <Button
                variant="secondary"
                size="sm"
                iconOnly
                title={t('addToBacklog')}
                aria-label={t('addToBacklog')}
                onClick={() => acceptOne(task.callId)}
                disabled={busy}
              >
                <PlusIcon />
              </Button>
            )}
          </li>
        ))}
      </ul>
      <div className="flex items-center gap-2">
        <Button
          variant="primary"
          size="md"
          onClick={acceptAll}
          disabled={remaining.length === 0 || busy}
          title={t('addAllTitle')}
        >
          {t('addAll')}
        </Button>
      </div>
      {error ? <p className="m-0 text-xs text-err">{error}</p> : null}
    </Card>
  );
}
