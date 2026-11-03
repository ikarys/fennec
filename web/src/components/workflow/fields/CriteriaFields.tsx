import { useT } from '../../../i18n';
import type { WorkflowDef, WorkflowNode } from '../../../lib/workflow';
import { FieldRow, Select } from '../../ui/Field';

interface CriteriaFieldsProps {
  node: WorkflowNode;
  def: WorkflowDef;
  names: string[];
  onChange: (field: string, value: unknown) => void;
}

export function CriteriaFields({ node, def, onChange }: CriteriaFieldsProps) {
  const t = useT();
  const agentNodes = def.nodes.filter((agentNode) => agentNode.kind === 'agent');

  return (
    <div className="flex flex-col gap-1.5">
      <FieldRow label={t('criteriaPlanNodeLabel')}>
        <Select
          value={(node.plan as string) ?? ''}
          onChange={(event) => onChange('plan', event.target.value)}
          className="w-full"
        >
          <option value="">—</option>
          {agentNodes.map((agentNode) => (
            <option key={agentNode.id} value={agentNode.id}>
              {agentNode.name ?? agentNode.id}
            </option>
          ))}
        </Select>
      </FieldRow>
      <p className="m-0 text-xs text-muted">
        {t('criteriaFieldsHint')}
      </p>
    </div>
  );
}
