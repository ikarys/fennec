import { ChoiceMenu } from '../../ui/ChoiceMenu';
import { useT } from '../../../i18n';
import type { Agent } from '../../../lib/api';
import type { WorkflowSummary } from '../../../lib/workflow';

export interface WorkflowSelection {
  workflowId: string | null;
  stepByStep: boolean;
  worktree: boolean;
}

export const DEFAULT_WORKFLOW_ID = 'plan-build-verify';

/** 'No workflow' is a value of its own, so the menu can select it. */
const NONE = '';

/**
 * The workflow the next message starts unless the human picks another: the
 * agent's own default (none for an agent that only talks), the project's
 * default while the agent is unknown.
 */
export function defaultWorkflowSelection(
  workflows: WorkflowSummary[],
  agent?: Agent,
): WorkflowSelection {
  const wanted = agent ? agent.workflow : DEFAULT_WORKFLOW_ID;
  return {
    workflowId: wanted && workflows.some((workflow) => workflow.id === wanted) ? wanted : null,
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
  const t = useT();
  if (workflows.length === 0) {
    return null;
  }

  const selected = workflows.find((workflow) => workflow.id === selection.workflowId);

  return (
    <div className="flex items-center gap-1">
      <ChoiceMenu
        label={t('workflowPickerLabel')}
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
                label: t('workflowPickerNone'),
                description: t('workflowPickerNoneDescription')
              }
            ]
          },
          {
            label: t('workflows'),
            choices: workflows.map((workflow) => ({
              value: workflow.id,
              label: workflow.name,
              description: workflow.description
            }))
          }
        ]}
        trigger={
          <span className={selection.workflowId ? 'text-accent-strong' : undefined}>
            {selected?.name ?? t('workflowPickerNone')}
          </span>
        }
      />
      {selection.stepByStep && (
        <button
          type="button"
          title={t('workflowPickerStepByStepTitle')}
          onClick={() => onChange({ ...selection, stepByStep: false })}
          className="rounded-md border border-accent px-1.5 py-0.5 text-xs text-accent"
        >
          {t('workflowPickerStepByStep')}
        </button>
      )}
    </div>
  );
}
