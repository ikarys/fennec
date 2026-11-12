// The composer's default workflow: the agent's own, none for an agent that only talks.
import { describe, expect, it } from 'vitest';
import type { Agent } from '../../../lib/api';
import type { WorkflowSummary } from '../../../lib/workflow';
import { DEFAULT_WORKFLOW_ID, defaultWorkflowSelection } from './WorkflowPicker';

const workflows: WorkflowSummary[] = [DEFAULT_WORKFLOW_ID, 'quick'].map((id) => ({
  id,
  name: id,
  description: '',
  origin: 'default',
}));

function agent(id: string, workflow: string | null): Agent {
  return { id, name: id, description: '', color: null, subagent: false, composer: true, phase: null, workflow, origin: 'default' };
}

describe('defaultWorkflowSelection', () => {
  it('starts no workflow for an agent without one', () => {
    expect(defaultWorkflowSelection(workflows, agent('ask', null)).workflowId).toBeNull();
  });

  it('takes the agent its own workflow', () => {
    expect(defaultWorkflowSelection(workflows, agent('builder', 'quick')).workflowId).toBe('quick');
  });

  it('ignores a workflow the project does not offer', () => {
    expect(defaultWorkflowSelection(workflows, agent('x', 'gone')).workflowId).toBeNull();
  });

  it('falls back to the project default while the agent is unknown', () => {
    expect(defaultWorkflowSelection(workflows).workflowId).toBe(DEFAULT_WORKFLOW_ID);
  });
});
