import { useState } from 'react';
import { Badge } from '../ui/Badge';
import { Button } from '../ui/Button';
import { FieldRow, Select, TextInput } from '../ui/Field';
import type { Agent } from '../../lib/api';
import type { WorkflowDef, WorkflowIssue, WorkflowNode } from '../../lib/workflow';
import { NODE_KIND_LABELS, templateNames } from '../../lib/workflow';
import { AgentFields } from './fields/AgentFields';
import { ConditionFields } from './fields/ConditionFields';
import { CriteriaFields } from './fields/CriteriaFields';
import { HumanFields } from './fields/HumanFields';
import { ShellFields } from './fields/ShellFields';

interface NodePanelProps {
  def: WorkflowDef;
  node: WorkflowNode;
  agents: Agent[];
  issues: WorkflowIssue[];
  onChange: (node: WorkflowNode) => void;
  onRename: (oldId: string, newId: string) => void;
  onDelete: () => void;
}

const ID_PATTERN = /^[a-z0-9][a-z0-9-]*$/;

export function NodePanel({ def, node, agents, issues, onChange, onRename, onDelete }: NodePanelProps) {
  const [idError, setIdError] = useState<string | null>(null);
  const nodeIssues = issues.filter((issue) => issue.nodeId === node.id);
  const names = templateNames(def, node.id);

  const updateField = (field: string, value: unknown) => {
    onChange({ ...node, [field]: value });
  };

  const handleIdChange = (newId: string) => {
    const trimmed = newId.trim();
    if (!ID_PATTERN.test(trimmed)) {
      setIdError("L'identifiant doit correspondre à ^[a-z0-9][a-z0-9-]*$");
      return;
    }
    if (trimmed !== node.id && def.nodes.some((n) => n.id === trimmed)) {
      setIdError('Identifiant déjà utilisé');
      return;
    }
    setIdError(null);
    if (trimmed !== node.id) {
      onRename(node.id, trimmed);
    }
  };

  return (
    <div className="flex flex-col gap-3 p-3">
      <div className="flex items-center gap-2 text-xs text-muted">
        Type
        <Badge tone="neutral">{NODE_KIND_LABELS[node.kind]}</Badge>
      </div>
      <FieldRow label="Identifiant">
        <TextInput
          type="text"
          value={node.id}
          onChange={(event) => handleIdChange(event.target.value)}
          className="w-full"
        />
        {idError && <div className="text-xs text-err">{idError}</div>}
      </FieldRow>

      <FieldRow label="Nom">
        <TextInput
          type="text"
          value={node.name ?? ''}
          onChange={(event) => updateField('name', event.target.value)}
          className="w-full"
        />
      </FieldRow>

      {node.kind === 'agent' && (
        <AgentFields node={node} agents={agents} names={names} onChange={updateField} />
      )}
      {node.kind === 'shell' && <ShellFields node={node} names={names} onChange={updateField} />}
      {node.kind === 'criteria' && <CriteriaFields node={node} def={def} names={names} onChange={updateField} />}
      {node.kind === 'human' && <HumanFields node={node} names={names} onChange={updateField} />}
      {node.kind === 'condition' && <ConditionFields node={node} names={names} onChange={updateField} />}
      {node.kind === 'end' && (
        <FieldRow label="Issue">
          <Select
            value={(node.outcome as string) ?? 'done'}
            onChange={(event) => updateField('outcome', event.target.value)}
            className="w-full"
          >
            <option value="done">terminé</option>
            <option value="blocked">bloqué</option>
          </Select>
        </FieldRow>
      )}

      {nodeIssues.length > 0 && (
        <div className="flex flex-col gap-1">
          <div className="text-[0.8125rem] font-medium text-err">Problèmes</div>
          {nodeIssues.map((issue, index) => (
            <div key={index} className="rounded-md bg-del-bg px-2 py-1 text-xs text-err">
              {issue.message}
            </div>
          ))}
        </div>
      )}

      <Button variant="danger" size="sm" type="button" onClick={onDelete} className="mt-2 self-start">
        Supprimer le nœud
      </Button>
    </div>
  );
}
