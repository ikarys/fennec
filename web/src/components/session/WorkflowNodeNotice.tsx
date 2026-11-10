// The strip shown in a workflow node's session: the workflow drives it, the human answers in the parent.
import { useT } from '../../i18n';
import { buttonClass } from '../ui/Button';

export function WorkflowNodeNotice({ parentId, running }: { parentId: string; running: boolean }) {
  const t = useT();
  return (
    <div className="mx-auto flex max-w-page items-center gap-3 border-t border-border px-1 py-2">
      <span
        className={`h-2 w-2 shrink-0 rounded-full ${running ? "bg-accent" : "bg-ok"}`}
      />
      <div className="min-w-0 grow">
        <div className="text-sm font-medium text-text">
          {running
            ? t('sessionPageWorkflowNodeRunning')
            : t('sessionPageWorkflowNodeFinished')}
        </div>
        <div className="text-xs text-muted">
          {t('sessionPageWorkflowDrivenSessionHint')}
        </div>
      </div>
      <a
        href={`#/s/${parentId}`}
        className={buttonClass(
          { variant: "primary", size: "sm" },
          "shrink-0",
        )}
      >
        {t('sessionPageBackToWorkflow')}
      </a>
    </div>
  );
}
