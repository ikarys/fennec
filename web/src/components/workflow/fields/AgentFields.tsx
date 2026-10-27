import type { Agent } from '../../../lib/api';
import type { WorkflowNode } from '../../../lib/workflow';
import { PortListEditor } from '../PortListEditor';
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
      <div>
        <label className="mb-1 block text-[13px] text-muted">Agent</label>
        <select
          value={(node.agent as string) ?? ''}
          onChange={(event) => onChange('agent', event.target.value)}
          className="w-full rounded-md border border-border bg-panel px-2 py-1 text-[13px]"
        >
          {agents.map((agent) => (
            <option key={agent.id} value={agent.id}>
              {agent.subagent ? `${agent.name} (sous-agent)` : agent.name}
            </option>
          ))}
        </select>
      </div>
      <div>
        <label className="mb-1 block text-[13px] text-muted">Contexte</label>
        <select
          value={(node.context as string) ?? 'session'}
          onChange={(event) => onChange('context', event.target.value)}
          className="w-full rounded-md border border-border bg-panel px-2 py-1 text-[13px]"
        >
          <option value="session">session</option>
          <option value="fresh">nouvelle session</option>
        </select>
      </div>
      <div>
        <label className="mb-1 block text-[13px] text-muted">Prompt</label>
        <textarea
          value={(node.prompt as string) ?? ''}
          onChange={(event) => onChange('prompt', event.target.value)}
          rows={3}
          className="w-full rounded-md border border-border bg-panel px-2 py-1 text-[13px]"
        />
        <TemplateChips names={names} onInsert={(name) => insertTemplate(name, 'prompt')} />
      </div>
      <div>
        <label className="mb-1 block text-[13px] text-muted">Prompt de nouvelle tentative</label>
        <textarea
          value={(node.retryPrompt as string) ?? ''}
          onChange={(event) => onChange('retryPrompt', event.target.value)}
          rows={3}
          className="w-full rounded-md border border-border bg-panel px-2 py-1 text-[13px]"
        />
        <TemplateChips names={names} onInsert={(name) => insertTemplate(name, 'retryPrompt')} />
      </div>
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
      <div>
        <label className="flex items-center gap-2 text-[13px] text-muted">
          <input
            type="checkbox"
            checked={Boolean(node.verdicts)}
            onChange={(event) => onChange('verdicts', event.target.checked)}
            className="h-4 w-4 rounded border-border bg-panel"
          />
          Verdicts sur les critères
        </label>
        <p className="mt-1 text-xs text-muted">Ses lignes C&lt;n&gt;: pass|fail jugent les critères sans commande (indicatif).</p>
      </div>
    </>
  );
}
