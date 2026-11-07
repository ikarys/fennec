import type { WorkflowEdge, WorkflowIssue } from '../../lib/workflow';
import { Button } from '../ui/Button';
import { TrashIcon } from '../ui/Icons';
import { FieldRow, TextInput } from '../ui/Field';
import { useT } from '../../i18n';

interface EdgePanelProps {
  edge: WorkflowEdge;
  issues: WorkflowIssue[];
  onChange: (edge: WorkflowEdge) => void;
  onDelete: () => void;
}

export function EdgePanel({ edge, issues, onChange, onDelete }: EdgePanelProps) {
  const t = useT();
  const edgeIssues = issues;

  return (
    <div className="flex flex-col gap-3 p-3">
      <div>
        <div className="mb-1 text-xs text-muted">{t('edgeLabel')}</div>
        <div className="rounded-lg bg-raised px-2 py-1 text-xs">
          {edge.from}.{edge.port} → {edge.to}
        </div>
      </div>

      <FieldRow label={t('edgeMaxPasses')} hint={t('edgeMaxPassesHint')}>
        <TextInput
          type="number"
          min={1}
          value={edge.maxPasses ?? ''}
          onChange={(event) => {
            const value = event.target.value;
            if (value === '') {
              onChange({ ...edge, maxPasses: undefined });
            } else {
              const n = Number(value);
              if (n >= 1) {
                onChange({ ...edge, maxPasses: n });
              }
            }
          }}
          className="w-full"
        />
      </FieldRow>

      <div>
        <label className="flex items-center gap-2 text-xs">
          <input
            type="checkbox"
            checked={edge.pause === true}
            onChange={(event) => onChange({ ...edge, pause: event.target.checked })}
            className="rounded"
          />
          {t('edgePauseBeforeTarget')}
        </label>
      </div>

      {edgeIssues.length > 0 && (
        <div className="flex flex-col gap-1">
          <div className="text-xs font-medium text-err">{t('edgeIssues')}</div>
          {edgeIssues.map((issue, index) => (
            <div key={index} className="rounded-md bg-del-bg px-2 py-1 text-xs text-err">
              {issue.message}
            </div>
          ))}
        </div>
      )}

      <Button
        variant="danger"
        size="sm"
        type="button"
        iconOnly
        onClick={onDelete}
        aria-label={t('edgeDelete')}
        title={t('edgeDelete')}
        className="mt-2 self-start"
      >
        <TrashIcon className="h-4 w-4" />
      </Button>
    </div>
  );
}
