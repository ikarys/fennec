import { useT } from '../../i18n';
import { Button } from '../ui/Button';
import { StarIcon } from '../ui/Icons';

/** Where the decision bar stood once its plan went to the backlog: the queue runs it from there. */
export function HandedPlanNote({
  projectId,
  onUndo
}: {
  projectId: string;
  /** Absent once the queue started the idea: too late to take the plan back. */
  onUndo?: () => void;
}) {
  const t = useT();
  return (
    <div className="shrink-0 px-6 pt-2">
      <div className="mx-auto flex max-w-page items-center gap-2 rounded-lg border border-border border-l-2 border-l-ok bg-panel px-3 py-1.5 text-xs text-muted">
        <StarIcon className="h-3.5 w-3.5 shrink-0 text-ok" />
        <span className="min-w-0 flex-1 truncate">{t(onUndo ? 'handedPlanWaiting' : 'handedPlanStarted')}</span>
        <a href={'#/project/' + encodeURIComponent(projectId) + '/tasks'} className="shrink-0 text-accent-strong underline">
          {t('viewTasks')}
        </a>
        {onUndo && (
          <Button variant="ghost" size="sm" onClick={onUndo} title={t('handedPlanUndoTitle')}>
            {t('handedPlanUndo')}
          </Button>
        )}
      </div>
    </div>
  );
}
