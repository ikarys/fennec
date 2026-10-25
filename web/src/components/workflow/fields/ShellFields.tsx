import type { WorkflowNode } from '../../../lib/workflow';
import { TemplateChips } from '../TemplateChips';

interface ShellFieldsProps {
  node: WorkflowNode;
  names: string[];
  onChange: (field: string, value: unknown) => void;
}

export function ShellFields({ node, names, onChange }: ShellFieldsProps) {
  const insertTemplate = (name: string) => {
    const current = (node.command as string) ?? '';
    onChange('command', current + `{{${name}}}`);
  };

  const timeoutSecs = typeof node.timeoutSecs === 'number' ? node.timeoutSecs : 600;

  return (
    <>
      <div>
        <label className="mb-1 block text-[13px] text-muted">Commande</label>
        <textarea
          value={(node.command as string) ?? ''}
          onChange={(event) => onChange('command', event.target.value)}
          rows={2}
          className="w-full rounded-md border border-border bg-panel px-2 py-1 font-mono text-[13px]"
        />
        <TemplateChips names={names} onInsert={insertTemplate} />
      </div>
      <div>
        <label className="mb-1 block text-[13px] text-muted">Délai (s)</label>
        <input
          type="number"
          value={timeoutSecs}
          onChange={(event) => onChange('timeoutSecs', Number(event.target.value))}
          className="w-full rounded-md border border-border bg-panel px-2 py-1 text-[13px]"
        />
      </div>
      <div>
        <label className="mb-1 block text-[13px] text-muted">Codes de succès</label>
        <input
          type="text"
          value={((node.successCodes as number[]) ?? [0]).join(',')}
          onChange={(event) => {
            const codes = event.target.value
              .split(',')
              .map((s) => Number(s.trim()))
              .filter((n) => !Number.isNaN(n));
            onChange('successCodes', codes);
          }}
          className="w-full rounded-md border border-border bg-panel px-2 py-1 text-[13px]"
        />
      </div>
    </>
  );
}
