import type { WorkflowSummary } from '../../../lib/workflow';

export interface WorkflowSelection {
  workflowId: string | null;
  stepByStep: boolean;
  worktree: boolean;
}

export const DEFAULT_WORKFLOW_ID = 'plan-build-verify';

export function defaultWorkflowSelection(workflows: WorkflowSummary[]): WorkflowSelection {
  return {
    workflowId: workflows.some((workflow) => workflow.id === DEFAULT_WORKFLOW_ID)
      ? DEFAULT_WORKFLOW_ID
      : null,
    stepByStep: false,
    worktree: true
  };
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
    <div className="flex items-center gap-1">
      <select
        value={selection.workflowId ?? ''}
        onChange={(e) => {
          const workflowId = e.target.value || null;
          onChange({
            workflowId,
            stepByStep: false,
            worktree: selection.worktree
          });
        }}
        aria-label="Workflow"
        title="Le workflow qui prend ta demande ; « Aucun » parle directement à l'agent."
        className={`h-7 rounded-md bg-transparent px-1.5 text-xs outline-none transition-colors hover:bg-raised ${
          selection.workflowId ? 'text-accent-strong' : 'text-muted hover:text-text'
        }`}
      >
        <option value="">Aucun workflow</option>
        {workflows.map((workflow) => (
          <option key={workflow.id} value={workflow.id}>
            {workflow.name}
          </option>
        ))}
      </select>
      {selection.stepByStep && (
        <button
          type="button"
          title="Le run s'arrête avant chaque nœud. Clique pour l'enlever."
          onClick={() => onChange({ ...selection, stepByStep: false })}
          className="rounded-md border border-accent px-1.5 py-0.5 text-xs text-accent"
        >
          Pas à pas ✕
        </button>
      )}
    </div>
  );
}
