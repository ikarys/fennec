// The composer's intent picker: offered primaries and the checked value.
import { describe, expect, it } from 'vitest';
import type { Agent } from '../../lib/api';
import type { WorkflowSummary } from '../../lib/workflow';
import type { WorkflowSelection } from '../workflow/session/workflowSelection';
import { currentIntent, menuGroups, offeredPrimaries } from './IntentPicker';

const workflows: WorkflowSummary[] = ['plan-build-verify', 'quick', 'docs'].map((id) => ({
  id,
  name: id,
  description: '',
  origin: 'default',
}));

function agent(id: string, workflow: string | null): Agent {
  return {
    id,
    name: id,
    description: '',
    color: null,
    subagent: false,
    composer: true,
    phase: null,
    workflow,
    origin: 'default',
  };
}

const agents: Agent[] = [
  agent('ask', null),
  agent('plan', 'plan-build-verify'),
  agent('builder', 'quick'),
  agent('audit', null),
];

function selection(workflowId: string | null): WorkflowSelection {
  return {
    workflowId,
    stepByStep: false,
    worktree: true,
  };
}

describe('offeredPrimaries', () => {
  it('drops a primary whose agent is missing', () => {
    const offered = offeredPrimaries(agents.filter((item) => item.id !== 'builder'), workflows);
    expect(offered.map((primary) => primary.agent)).toEqual(['plan', 'ask']);
  });

  it('drops a primary whose workflow is missing', () => {
    const offered = offeredPrimaries(agents, workflows.filter((item) => item.id !== 'quick'));
    expect(offered.map((primary) => primary.agent)).toEqual(['plan', 'ask']);
  });
});

describe('menuGroups', () => {
  const primaries = offeredPrimaries(agents, workflows);

  it('excludes a primary\'s workflow from the Workflows list', () => {
    const { workflows: secondary } = menuGroups(agents, workflows, primaries);
    expect(secondary.map((w) => w.id)).not.toContain('plan-build-verify');
    expect(secondary.map((w) => w.id)).not.toContain('quick');
  });

  it('includes a project workflow that no primary offers', () => {
    const { workflows: secondary } = menuGroups(agents, workflows, primaries);
    expect(secondary.map((w) => w.id)).toContain('docs');
  });

  it('lists non-primary agents as experts', () => {
    const { experts } = menuGroups(agents, workflows, primaries);
    expect(experts.map((a) => a.id)).toEqual(['audit']);
  });
});

describe('currentIntent', () => {
  const primaries = offeredPrimaries(agents, workflows);

  it('checks the primary when the agent and workflow match', () => {
    expect(currentIntent('plan', selection('plan-build-verify'), primaries)).toBe('agent:plan');
  });

  it('checks the workflow when the agent keeps a non-primary workflow', () => {
    expect(currentIntent('plan', selection('quick'), primaries)).toBe('workflow:quick');
  });

  it('checks the agent when a primary agent has no workflow', () => {
    expect(currentIntent('ask', selection(null), primaries)).toBe('agent:ask');
  });

  it('checks a non-primary agent without a workflow', () => {
    expect(currentIntent('audit', selection(null), primaries)).toBe('agent:audit');
  });
});
