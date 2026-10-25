import type { WorkflowSummary } from '../../../lib/workflow';

export interface WorkflowSelection {
  workflowId: string | null;
  stepByStep: boolean;
}

export function WorkflowPicker({
  workflows,
  selection,
  onChange
}: {
  workflows: WorkflowSummary[];
  selection: WorkflowSelection;
  onChange: (selection: WorkflowSelection) => void;
}) {
  if (workflows.length === 0) {
    return null;
  }

  return (
    <div className="flex items-center gap-2">
      <select
        value={selection.workflowId ?? ''}
        onChange={(e) => {
          const workflowId = e.target.value || null;
          onChange({ workflowId, stepByStep: workflowId ? selection.stepByStep : false });
        }}
        className="rounded-lg border border-border bg-raised px-2 py-1 text-xs text-text outline-none focus:border-accent"
      >
        <option value="">Aucun workflow</option>
        {workflows.map((workflow) => (
          <option key={workflow.id} value={workflow.id}>
            {workflow.name}
          </option>
        ))}
      </select>
      {selection.workflowId && (
        <label className="flex items-center gap-1.5 text-xs text-muted">
          <input
            type="checkbox"
            checked={selection.stepByStep}
            onChange={(e) => onChange({ ...selection, stepByStep: e.target.checked })}
            className="rounded border-border"
          />
          Pas à pas
        </label>
      )}
    </div>
  );
}
