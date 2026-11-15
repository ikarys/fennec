/** The composer's single picker for the agent, workflow, and step-by-step mode. */
import { ChoiceMenu, type Choice, type ChoiceGroup } from '../ui/ChoiceMenu';
import { Button } from '../ui/Button';
import { useT } from '../../i18n';
import type { Agent } from '../../lib/api';
import type { WorkflowSummary } from '../../lib/workflow';
import type { WorkflowSelection } from '../workflow/session/workflowSelection';

export const PRIMARY_INTENTS = [
  { agent: 'plan', workflow: 'plan-build-verify', labelKey: 'intentChange', descriptionKey: 'intentChangeDescription' },
  { agent: 'builder', workflow: 'quick', labelKey: 'intentQuick', descriptionKey: 'intentQuickDescription' },
  { agent: 'ask', workflow: null, labelKey: 'intentAsk', descriptionKey: 'intentAskDescription' },
] as const;

/** The primary intents this project offers: the agent exists and its workflow (if any) is offered. */
export function offeredPrimaries(agents: Agent[], workflows: WorkflowSummary[]): typeof PRIMARY_INTENTS[number][] {
  return PRIMARY_INTENTS.filter((primary) => {
    const agent = agents.find((item) => item.id === primary.agent);
    if (!agent) return false;
    return primary.workflow === null || workflows.some((workflow) => workflow.id === primary.workflow);
  });
}

/** The two secondary lists: project workflows not already offered by a primary, and agents that are not primaries. */
export function menuGroups(
  agents: Agent[],
  workflows: WorkflowSummary[],
  primaries: readonly typeof PRIMARY_INTENTS[number][],
): { workflows: WorkflowSummary[]; experts: Agent[] } {
  const primaryAgents = new Set<string>(primaries.map((primary) => primary.agent));
  const primaryWorkflows = new Set<string | null>(primaries.map((primary) => primary.workflow));
  return {
    workflows: workflows.filter((workflow) => !primaryWorkflows.has(workflow.id)),
    experts: agents.filter((agent) => !primaryAgents.has(agent.id)),
  };
}

/** The value the menu checks: a primary when mode and workflow match one, else `workflow:<id>` when a workflow is selected, else `agent:<mode>` (or `none` when mode is null). */
export function currentIntent(
  mode: string | null,
  selection: WorkflowSelection,
  primaries: readonly typeof PRIMARY_INTENTS[number][],
): string {
  const primary = primaries.find((item) => item.agent === mode && item.workflow === selection.workflowId);
  if (primary) return `agent:${primary.agent}`;
  if (selection.workflowId) return `workflow:${selection.workflowId}`;
  if (mode) return `agent:${mode}`;
  return 'none';
}

interface IntentPickerProps {
  agents: Agent[];
  mode: string | null;
  onModeChange: (id: string) => void;
  workflows: WorkflowSummary[];
  selection: WorkflowSelection;
  onWorkflowSelectionChange: (selection: WorkflowSelection) => void;
}

export function IntentPicker({
  agents,
  mode,
  onModeChange,
  workflows,
  selection,
  onWorkflowSelectionChange,
}: IntentPickerProps) {
  const t = useT();
  const primaries = offeredPrimaries(agents, workflows);
  const { workflows: secondaryWorkflows, experts } = menuGroups(agents, workflows, primaries);

  const primaryChoice = (primary: (typeof primaries)[number]): Choice => ({
    value: `agent:${primary.agent}`,
    label: t(primary.labelKey),
    description: t(primary.descriptionKey),
  });
  const workflowChoices: Choice[] = [
    ...primaries.filter((primary) => primary.workflow !== null).map(primaryChoice),
    ...secondaryWorkflows.map((workflow) => ({
      value: `workflow:${workflow.id}`,
      label: workflow.name,
      description: workflow.description,
    })),
  ];
  const agentChoices = primaries.filter((primary) => primary.workflow === null).map(primaryChoice);

  const groups: ChoiceGroup[] = [];
  if (workflowChoices.length > 0) groups.push({ label: t('intentWorkflows'), choices: workflowChoices });
  if (agentChoices.length > 0) groups.push({ label: t('intentAgents'), choices: agentChoices });

  const expertChoices: Choice[] = [
    ...experts.map((agent) => ({
      value: `agent:${agent.id}`,
      label: agent.name,
      description: agent.description,
    })),
    {
      value: 'none',
      label: t('workflowPickerNone'),
      description: t('workflowPickerNoneDescription'),
    },
  ];

  groups.push({
    label: t('intentMore'),
    choices: expertChoices,
    collapsed: true,
  });

  if (workflowChoices.length + agentChoices.length + expertChoices.length < 2) {
    return null;
  }

  const value = currentIntent(mode, selection, primaries);
  const agent = agents.find((item) => item.id === mode) ?? null;
  const activePrimary =
    primaries.find((primary) => primary.agent === mode && primary.workflow === selection.workflowId) ?? null;
  const selectedWorkflow = workflows.find((workflow) => workflow.id === selection.workflowId) ?? null;
  const triggerLabel = activePrimary
    ? t(activePrimary.labelKey)
    : selectedWorkflow?.name ?? agent?.name ?? t('intentLabel');

  function handleChange(next: string) {
    if (next.startsWith('agent:')) {
      onModeChange(next.slice('agent:'.length));
      return;
    }
    if (next.startsWith('workflow:')) {
      onWorkflowSelectionChange({
        ...selection,
        workflowId: next.slice('workflow:'.length),
        stepByStep: false,
      });
      return;
    }
    if (next === 'none') {
      onWorkflowSelectionChange({
        ...selection,
        workflowId: null,
        stepByStep: false,
      });
    }
  }

  return (
    <div className="flex items-center gap-1">
      <ChoiceMenu
        label={t('intentLabel')}
        groups={groups}
        value={value}
        onChange={handleChange}
        trigger={t('intentTrigger', { label: triggerLabel })}
      />
      {selection.stepByStep && (
        <Button
          variant="ghost"
          size="sm"
          title={t('workflowPickerStepByStepTitle')}
          onClick={() => onWorkflowSelectionChange({ ...selection, stepByStep: false })}
        >
          {t('workflowPickerStepByStep')}
        </Button>
      )}
    </div>
  );
}
