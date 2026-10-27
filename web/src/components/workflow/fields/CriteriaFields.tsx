import type { WorkflowDef, WorkflowNode } from '../../../lib/workflow';

interface CriteriaFieldsProps {
  node: WorkflowNode;
  def: WorkflowDef;
  names: string[];
  onChange: (field: string, value: unknown) => void;
}

export function CriteriaFields({ node, def, onChange }: CriteriaFieldsProps) {
  const agentNodes = def.nodes.filter((agentNode) => agentNode.kind === 'agent');

  return (
    <>
      <div>
        <label className="mb-1 block text-[13px] text-muted">Nœud du plan</label>
        <select
          value={(node.plan as string) ?? ''}
          onChange={(event) => onChange('plan', event.target.value)}
          className="w-full rounded-md border border-border bg-panel px-2 py-1 text-[13px]"
        >
          <option value="">—</option>
          {agentNodes.map((agentNode) => (
            <option key={agentNode.id} value={agentNode.id}>
              {agentNode.name ?? agentNode.id}
            </option>
          ))}
        </select>
      </div>
      <p className="m-0 text-xs text-muted">
        Lit la liste « Done when » du dernier plan de ce nœud, l'enregistre comme contrat, puis exécute les
        commandes check: de chaque critère (vérifié). Les critères sans commande sont jugés par un agent à
        verdicts (indicatif).
      </p>
    </>
  );
}
