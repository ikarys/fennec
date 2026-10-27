import type { Agent } from '../../../lib/api';
import type { WorkflowNode } from '../../../lib/workflow';
import { PortListEditor } from '../PortListEditor';
import { FieldRow, Select, TextArea } from '../../ui/Field';
import { TemplateChips } from '../TemplateChips';

interface AgentFieldsProps {
  node: WorkflowNode;
  agents: Agent[];
  names: string[];
  onChange: (field: string, value: unknown) => void;
}

export function AgentFields({ node, agents, names, onChange }: AgentFieldsProps) {
  const insertTemplate = (name: string, field: string) => {
    const current = (node[field] as string) ?? '';
    onChange(field, current + `{{${name}}}`);
  };

  return (
    <>
      <FieldRow label="Agent">
        <Select
          value={(node.agent as string) ?? ''}
          onChange={(event) => onChange('agent', event.target.value)}
          className="w-full"
        >
          {agents.map((agent) => (
            <option key={agent.id} value={agent.id}>
              {agent.subagent ? `${agent.name} (sous-agent)` : agent.name}
            </option>
          ))}
        </Select>
      </FieldRow>
      <FieldRow label="Contexte">
        <Select
          value={(node.context as string) ?? 'session'}
          onChange={(event) => onChange('context', event.target.value)}
          className="w-full"
        >
          <option value="session">session</option>
          <option value="fresh">nouvelle session</option>
        </Select>
      </FieldRow>
      <FieldRow label="Prompt">
        <TextArea
          value={(node.prompt as string) ?? ''}
          onChange={(event) => onChange('prompt', event.target.value)}
          rows={3}
          className="w-full"
        />
        <TemplateChips names={names} onInsert={(name) => insertTemplate(name, 'prompt')} />
      </FieldRow>
      <FieldRow label="Prompt de nouvelle tentative">
        <TextArea
          value={(node.retryPrompt as string) ?? ''}
          onChange={(event) => onChange('retryPrompt', event.target.value)}
          rows={3}
          className="w-full"
        />
        <TemplateChips names={names} onInsert={(name) => insertTemplate(name, 'retryPrompt')} />
      </FieldRow>
      <PortListEditor
        label="Sorties"
        values={((node.outputs as string[]) ?? []).length > 0 ? (node.outputs as string[]) : ['done']}
        onChange={(values) => onChange('outputs', values)}
        onRename={(oldValue, newValue) => {
          const next = ((node.outputs as string[]) ?? []).map((v) => (v === oldValue ? newValue : v));
          onChange('outputs', next);
        }}
        onRemove={(value) => {
          const next = ((node.outputs as string[]) ?? []).filter((v) => v !== value);
          onChange('outputs', next);
        }}
      />
      <div className="flex flex-col gap-1">
        <label className="flex items-center gap-2 text-xs text-muted">
          <input
            type="checkbox"
            checked={Boolean(node.verdicts)}
            onChange={(event) => onChange('verdicts', event.target.checked)}
            className="h-4 w-4 rounded border-border bg-panel"
          />
          Verdicts sur les critères
        </label>
        <p className="m-0 text-xs text-muted">Ses lignes C&lt;n&gt;: pass|fail jugent les critères sans commande (indicatif).</p>
      </div>
    </>
  );
}
