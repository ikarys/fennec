import type { WorkflowNode } from '../../../lib/workflow';
import { useT } from '../../../i18n';
import { TemplateChips } from '../TemplateChips';
import { FieldRow, Select, TextInput } from '../../ui/Field';

interface ConditionFieldsProps {
  node: WorkflowNode;
  names: string[];
  onChange: (field: string, value: unknown) => void;
}

export function ConditionFields({ node, names, onChange }: ConditionFieldsProps) {
  const t = useT();
  const insertTemplate = (name: string) => {
    const current = (node.value as string) ?? '';
    onChange('value', current + `{{${name}}}`);
  };

  return (
    <>
      <FieldRow label={t('conditionValueLabel')}>
        <TextInput
          type="text"
          value={(node.value as string) ?? ''}
          onChange={(event) => onChange('value', event.target.value)}
          className="w-full"
        />
        <TemplateChips names={names} onInsert={insertTemplate} />
      </FieldRow>
      <FieldRow label={t('conditionTestLabel')}>
        <Select
          value={(node.test as string) ?? 'equals'}
          onChange={(event) => onChange('test', event.target.value)}
          className="w-full"
        >
          <option value="equals">{t('conditionTestEquals')}</option>
          <option value="contains">{t('conditionTestContains')}</option>
          <option value="empty">{t('conditionTestEmpty')}</option>
        </Select>
      </FieldRow>
      {(node.test as string) !== 'empty' && (
        <FieldRow label={t('conditionCompareLabel')}>
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
