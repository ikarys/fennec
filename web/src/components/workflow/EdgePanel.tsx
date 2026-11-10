import type { WorkflowEdge, WorkflowIssue } from '../../lib/workflow';
import { PanelFooter } from './PanelFooter';
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
            className="rounded-md"
          />
          {t('edgePauseBeforeTarget')}
        </label>
      </div>

      <PanelFooter issues={edgeIssues} deleteLabel={t('edgeDelete')} onDelete={onDelete} titleClass="text-xs" />
    </div>
  );
}
