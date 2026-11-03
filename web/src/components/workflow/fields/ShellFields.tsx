import type { WorkflowNode } from '../../../lib/workflow';
import { TemplateChips } from '../TemplateChips';
import { FieldRow, TextArea, TextInput } from '../../ui/Field';
import { useT } from '../../../i18n';

interface ShellFieldsProps {
  node: WorkflowNode;
  names: string[];
  onChange: (field: string, value: unknown) => void;
}

export function ShellFields({ node, names, onChange }: ShellFieldsProps) {
  const t = useT();
  const insertTemplate = (name: string) => {
    const current = (node.command as string) ?? '';
    onChange('command', current + `{{${name}}}`);
  };

  const timeoutSecs = typeof node.timeoutSecs === 'number' ? node.timeoutSecs : 600;

  return (
    <>
      <FieldRow label={t('shellCommand')}>
        <TextArea
          value={(node.command as string) ?? ''}
          onChange={(event) => onChange('command', event.target.value)}
          rows={2}
          className="w-full font-mono"
        />
        <TemplateChips names={names} onInsert={insertTemplate} />
      </FieldRow>
      <FieldRow label={t('shellTimeoutSecs')}>
        <TextInput
          type="number"
          value={timeoutSecs}
          onChange={(event) => onChange('timeoutSecs', Number(event.target.value))}
          className="w-full"
        />
      </FieldRow>
      <FieldRow label={t('shellSuccessCodes')}>
        <TextInput
          type="text"
          value={((node.successCodes as number[]) ?? [0]).join(',')}
          onChange={(event) => {
            const codes = event.target.value
              .split(',')
              .map((s) => Number(s.trim()))
              .filter((n) => !Number.isNaN(n));
            onChange('successCodes', codes);
          }}
          className="w-full"
        />
      </FieldRow>
    </>
  );
}
