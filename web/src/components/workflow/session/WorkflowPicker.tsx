import { ChoiceMenu } from '../../ui/ChoiceMenu';
import type { WorkflowSummary } from '../../../lib/workflow';

export interface WorkflowSelection {
  workflowId: string | null;
  stepByStep: boolean;
  worktree: boolean;
}

export const DEFAULT_WORKFLOW_ID = 'plan-build-verify';

/** « Aucun workflow » is a value of its own, so the menu can select it. */
const NONE = '';

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

  const selected = workflows.find((workflow) => workflow.id === selection.workflowId);

  return (
    <div className="flex items-center gap-1">
      <ChoiceMenu
        label="Workflow"
        value={selection.workflowId ?? NONE}
        onChange={(value) =>
          onChange({
            workflowId: value === NONE ? null : value,
            stepByStep: false,
            worktree: selection.worktree
          })
        }
        groups={[
          {
            choices: [
              {
                value: NONE,
                label: 'Aucun workflow',
                description: "Parle directement à l'agent, sans graphe."
              }
            ]
          },
          {
            label: 'Workflows',
            choices: workflows.map((workflow) => ({
              value: workflow.id,
              label: workflow.name,
              description: workflow.description
            }))
          }
        ]}
        trigger={
          <span className={selection.workflowId ? 'text-accent-strong' : undefined}>
            {selected?.name ?? 'Aucun workflow'}
          </span>
        }
      />
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
