import type { WorkflowNode } from '../../../lib/workflow';
import { PortListEditor } from '../PortListEditor';
import { TemplateChips } from '../TemplateChips';

interface HumanFieldsProps {
  node: WorkflowNode;
  names: string[];
  onChange: (field: string, value: unknown) => void;
}

export function HumanFields({ node, names, onChange }: HumanFieldsProps) {
  const insertTemplate = (name: string) => {
    const current = (node.question as string) ?? '';
    onChange('question', current + `{{${name}}}`);
  };

  return (
    <>
      <div>
        <label className="mb-1 block text-[13px] text-muted">Question</label>
        <textarea
          value={(node.question as string) ?? ''}
          onChange={(event) => onChange('question', event.target.value)}
          rows={2}
          className="w-full rounded-md border border-border bg-panel px-2 py-1 text-[13px]"
        />
        <TemplateChips names={names} onInsert={insertTemplate} />
      </div>
      <PortListEditor
        label="Choix"
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
