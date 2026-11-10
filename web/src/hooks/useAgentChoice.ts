// Choosing an agent for the session: from the composer's picker or a keyboard shortcut.
import { useEffect, useRef } from 'react';
import { AGENT_SHORTCUT_EVENT } from '../lib/shortcuts';
import type { useProjectOptions } from './useProjectOptions';

type ProjectOptions = ReturnType<typeof useProjectOptions>;

interface UseAgentChoiceArgs {
  isChat: boolean;
  agents: ProjectOptions['agents'];
  workflows: ProjectOptions['workflows'];
  setWorkflowSelection: ProjectOptions['setWorkflowSelection'];
  /** Applies the mode; resolves true when it changed. */
  handleModeChange: (agent: string) => Promise<boolean>;
}

export function useAgentChoice({
  isChat,
  agents,
  workflows,
  setWorkflowSelection,
  handleModeChange,
}: UseAgentChoiceArgs): (agent: string) => Promise<boolean> {
  // Picking an agent means talking to it: the composer's default workflow
  // would otherwise take the message and start with its planner instead —
  // unless the agent carries its own default, kept only while the project
  // offers that workflow.
  function chooseAgent(agent: string) {
    const workflow = agents.find((a) => a.id === agent)?.workflow ?? null;
    setWorkflowSelection((selection) => ({
      ...selection,
      workflowId: workflow && workflows.some((w) => w.id === workflow) ? workflow : null,
      stepByStep: false,
    }));
    return handleModeChange(agent);
  }

  // A keyboard shortcut switches the agent like the composer's picker; a chat has no picker.
  const chooseAgentRef = useRef(chooseAgent);
  chooseAgentRef.current = chooseAgent;
  useEffect(() => {
    if (isChat) return;
    const onAgent = (event: Event) => {
      const agent = (event as CustomEvent<string>).detail;
      if (agents.some((candidate) => candidate.id === agent)) void chooseAgentRef.current(agent);
    };
    window.addEventListener(AGENT_SHORTCUT_EVENT, onAgent);
    return () => window.removeEventListener(AGENT_SHORTCUT_EVENT, onAgent);
  }, [isChat, agents]);

  return chooseAgent;
}
