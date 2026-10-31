import type { WorkflowSummary } from '../lib/workflow';
import { useEffect, useState, type Dispatch, type SetStateAction } from 'react';
import type { Agent, SlashCommand } from '../lib/api';
import { listAgents, listCommands, listWorkflows } from '../lib/api';
import {
  defaultWorkflowSelection,
  type WorkflowSelection,
} from '../components/workflow/session/WorkflowPicker';

interface UseProjectOptionsArgs {
  /** The project the options belong to: the session's, or the one the new session targets. */
  projectId: string | null;
  /** A workflow opened from the editor's test button: preselected, step by step. */
  testWorkflowId: string | null;
  /** A backlog idea framed from the project page: no workflow for a framing session. */
  frameIdeaId: string | null;
  /** The session is framing a backlog idea: its messages must not start a workflow. */
  framingIdea: boolean;
}

/**
 * The composer's options for a project: the slash commands, the agents and the
 * workflows, plus which workflow the next message starts.
 */
export function useProjectOptions({
  projectId,
  testWorkflowId,
  frameIdeaId,
  framingIdea,
}: UseProjectOptionsArgs): {
  commands: SlashCommand[];
  agents: Agent[];
  workflows: WorkflowSummary[];
  workflowSelection: WorkflowSelection;
  setWorkflowSelection: Dispatch<SetStateAction<WorkflowSelection>>;
} {
  const [commands, setCommands] = useState<SlashCommand[]>([]);
  const [agents, setAgents] = useState<Agent[]>([]);
  const [workflows, setWorkflows] = useState<WorkflowSummary[]>([]);
  const [workflowSelection, setWorkflowSelection] = useState<WorkflowSelection>(
    { workflowId: null, stepByStep: false, worktree: true },
  );

  useEffect(() => {
    if (!projectId) {
      setCommands([]);
      return;
    }
    let ignore = false;
    listCommands(projectId)
      .then((data) => {
        if (!ignore) setCommands(data);
      })
      .catch(() => {
        if (!ignore) setCommands([]);
      });
    return () => {
      ignore = true;
    };
  }, [projectId]);

  useEffect(() => {
    if (!projectId) {
      setAgents([]);
      return;
    }
    let ignore = false;
    listAgents(projectId)
      .then((data) => {
        if (!ignore) setAgents(data.filter((agent) => !agent.subagent));
      })
      .catch(() => {
        if (!ignore) setAgents([]);
      });
    return () => {
      ignore = true;
    };
  }, [projectId]);

  useEffect(() => {
    if (!projectId) {
      setWorkflows([]);
      return;
    }
    let ignore = false;
    listWorkflows(projectId)
      .then((data) => {
        if (!ignore) {
          setWorkflows(data);
          if (
            testWorkflowId &&
            data.some((workflow) => workflow.id === testWorkflowId)
          ) {
            setWorkflowSelection({
              workflowId: testWorkflowId,
              stepByStep: true,
              worktree: true,
            });
          } else if (frameIdeaId) {
            setWorkflowSelection({
              ...defaultWorkflowSelection(data),
              workflowId: null,
            });
          } else {
            setWorkflowSelection(defaultWorkflowSelection(data));
          }
        }
      })
      .catch(() => {
        if (!ignore) setWorkflows([]);
      });
    return () => {
      ignore = true;
    };
  }, [projectId, testWorkflowId, frameIdeaId]);

  // A session framing a backlog idea only talks: its messages must not start
  // the default build workflow (a 'go on' there once ran a whole build).
  // Depends on `workflows` too, so a late workflow list does not reselect it.
  useEffect(() => {
    if (framingIdea) {
      setWorkflowSelection((selection) => ({ ...selection, workflowId: null }));
    }
  }, [framingIdea, workflows]);

  return {
    commands,
    agents,
    workflows,
    workflowSelection,
    setWorkflowSelection,
  };
}
