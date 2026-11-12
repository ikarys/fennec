import type { WorkflowNode } from '../../../lib/workflow';
import { PortListEditor } from '../PortListEditor';
import { TemplateChips } from '../TemplateChips';
import { FieldRow, TextArea } from '../../ui/Field';
import { useT } from '../../../i18n';

interface HumanFieldsProps {
  node: WorkflowNode;
  names: string[];
  onChange: (field: string, value: unknown) => void;
}

export function HumanFields({ node, names, onChange }: HumanFieldsProps) {
  const t = useT();
  const insertTemplate = (name: string) => {
    const current = (node.question as string) ?? '';
    onChange('question', current + `{{${name}}}`);
  };

  return (
    <>
      <FieldRow label="Question">
        <TextArea
          value={(node.question as string) ?? ''}
          onChange={(event) => onChange('question', event.target.value)}
          rows={2}
          className="w-full"
        />
        <TemplateChips names={names} onInsert={insertTemplate} />
      </FieldRow>
      <PortListEditor
        label={t('humanChoicesLabel')}
        values={(node.choices as string[]) ?? []}
        onChange={(values) => onChange('choices', values)}
        onRename={(oldValue, newValue) => {
          const next = ((node.choices as string[]) ?? []).map((v) => (v === oldValue ? newValue : v));
          onChange('choices', next);
        }}
        onRemove={(value) => {
          const next = ((node.choices as string[]) ?? []).filter((v) => v !== value);
          onChange('choices', next);
        }}
      />
    </>
  );
}
