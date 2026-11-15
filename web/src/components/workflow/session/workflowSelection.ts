import type { Agent } from '../../../lib/api';
import type { WorkflowSummary } from '../../../lib/workflow';

export interface WorkflowSelection {
  workflowId: string | null;
  stepByStep: boolean;
  worktree: boolean;
}

export const DEFAULT_WORKFLOW_ID = 'plan-build-verify';

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
