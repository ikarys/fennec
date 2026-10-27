import type { WorkflowNode } from '../../../lib/workflow';
import { TemplateChips } from '../TemplateChips';

interface ConditionFieldsProps {
  node: WorkflowNode;
  names: string[];
  onChange: (field: string, value: unknown) => void;
}

export function ConditionFields({ node, names, onChange }: ConditionFieldsProps) {
  const insertTemplate = (name: string) => {
    const current = (node.value as string) ?? '';
    onChange('value', current + `{{${name}}}`);
  };

  return (
    <>
      <div>
        <label className="mb-1 block text-[0.8125rem] text-muted">Valeur</label>
        <input
          type="text"
          value={(node.value as string) ?? ''}
          onChange={(event) => onChange('value', event.target.value)}
          className="w-full rounded-md border border-border bg-panel px-2 py-1 text-[0.8125rem]"
        />
        <TemplateChips names={names} onInsert={insertTemplate} />
      </div>
      <div>
        <label className="mb-1 block text-[0.8125rem] text-muted">Test</label>
        <select
          value={(node.test as string) ?? 'equals'}
          onChange={(event) => onChange('test', event.target.value)}
          className="w-full rounded-md border border-border bg-panel px-2 py-1 text-[0.8125rem]"
        >
          <option value="equals">equals</option>
          <option value="contains">contains</option>
          <option value="empty">empty</option>
        </select>
      </div>
      {(node.test as string) !== 'empty' && (
        <div>
          <label className="mb-1 block text-[0.8125rem] text-muted">Comparer à</label>
          <input
            type="text"
            value={(node.against as string) ?? ''}
            onChange={(event) => onChange('against', event.target.value)}
            className="w-full rounded-md border border-border bg-panel px-2 py-1 text-[0.8125rem]"
          />
        </div>
      )}
    </>
  );
}
