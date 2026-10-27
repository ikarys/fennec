import type { WorkflowNode } from '../../../lib/workflow';
import { TemplateChips } from '../TemplateChips';
import { FieldRow, Select, TextInput } from '../../ui/Field';

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
      <FieldRow label="Valeur">
        <TextInput
          type="text"
          value={(node.value as string) ?? ''}
          onChange={(event) => onChange('value', event.target.value)}
          className="w-full"
        />
        <TemplateChips names={names} onInsert={insertTemplate} />
      </FieldRow>
      <FieldRow label="Test">
        <Select
          value={(node.test as string) ?? 'equals'}
          onChange={(event) => onChange('test', event.target.value)}
          className="w-full"
        >
          <option value="equals">equals</option>
          <option value="contains">contains</option>
          <option value="empty">empty</option>
        </Select>
      </FieldRow>
      {(node.test as string) !== 'empty' && (
        <FieldRow label="Comparer à">
          <TextInput
            type="text"
            value={(node.against as string) ?? ''}
            onChange={(event) => onChange('against', event.target.value)}
            className="w-full"
          />
        </FieldRow>
      )}
    </>
  );
}
