/**
 * The tasks the agent proposed, a row of the decision bar: the human adds
 * them to the backlog one by one or all at once.
 */
import { useState } from 'react';

import { acceptProposedTask, type ProposedTask } from '../../lib/api';
import { DecisionRow } from './DecisionBar';
import { PlusIcon, StarIcon } from '../ui/Icons';
import { Badge } from '../ui/Badge';
import { Button } from '../ui/Button';
import { useT } from '../../i18n';

export function ProposedTasksDecision({
  tasks,
  sessionId,
  onAccepted,
  open,
  onToggle,
}: {
  tasks: ProposedTask[];
  sessionId: string;
  onAccepted: () => void;
  open: boolean;
  onToggle: () => void;
}) {
  const t = useT();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

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

  const reviewButton = (
    <Button variant="secondary" size="sm" onClick={onToggle}>
      {t('decisionReview')}
    </Button>
  );

  return (
    <DecisionRow
      icon={<StarIcon className="h-3.5 w-3.5" />}
      title={t('whichTasksQuestion')}
      meta={`${t('backlogBadge')} · ${remaining.length}`}
      primary={reviewButton}
      open={open}
      onToggle={onToggle}
      footer={
        <>
          <span className="flex-1" />
          <Button
            variant="primary"
            size="sm"
            onClick={acceptAll}
            disabled={remaining.length === 0 || busy}
            title={t('addAllTitle')}
          >
            {t('addAll')}
          </Button>
        </>
      }
    >
      <p className="m-0 mb-2 text-xs text-muted">{t('proposedTasksHint')}</p>
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
      {error ? <div className="mt-1.5 text-xs text-err">{error}</div> : null}
    </DecisionRow>
  );
}
