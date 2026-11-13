import type { WorkflowSummary } from '../lib/workflow';
import type { LaunchWorkflow } from '../lib/route';
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
  /** A workflow opened from the editor's run or test button: preselected, step by step for a test. */
  launchWorkflow: LaunchWorkflow | null;
  /** The workflow of the session's latest run: the chip shows what the session runs. */
  runWorkflowId: string | null | undefined;
  /** The session's latest run has ended: a follow-up message is a plain turn, not a new run. */
  runFinished: boolean;
  /** A backlog idea framed from the project page: no workflow for a framing session. */
  frameIdeaId: string | null;
  /** The session is framing a backlog idea: its messages must not start a workflow. */
  framingIdea: boolean;
  /**
   * The session's agent; null for a new session (the server lists its default
   * first), undefined while the session's detail is loading.
   */
  mode: string | null | undefined;
}

/**
 * The composer's options for a project: the slash commands, the agents and the
 * workflows, plus which workflow the next message starts.
 */
export function useProjectOptions({
  projectId,
  launchWorkflow,
  runWorkflowId,
  runFinished,
  frameIdeaId,
  framingIdea,
  mode,
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
        if (!ignore) setAgents(data.filter((agent) => agent.composer));
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
        }
      })
      .catch(() => {
        if (!ignore) setWorkflows([]);
      });
    return () => {
      ignore = true;
    };
  }, [projectId]);

  // Primitives, not the route's object: a later hash change must not reset the selection.
  const launchId = launchWorkflow?.id ?? null;
  const launchStepByStep = launchWorkflow?.stepByStep ?? false;
  // The selection follows the session's agent: a session in an agent that
  // only talks (ask) must not start a workflow because the page reloaded.
  useEffect(() => {
    if (mode === undefined || workflows.length === 0 || agents.length === 0) return;
    if (launchId && workflows.some((workflow) => workflow.id === launchId)) {
      setWorkflowSelection({ workflowId: launchId, stepByStep: launchStepByStep, worktree: true });
      return;
    }
    const agent = agents.find((candidate) => candidate.id === mode) ?? agents[0];
    const selection = defaultWorkflowSelection(workflows, agent);
    // The chip shows the workflow the session runs, not the agent's default;
    // an agent that only talks still starts none.
    const ran = selection.workflowId && runWorkflowId && workflows.some((workflow) => workflow.id === runWorkflowId) ? runWorkflowId : selection.workflowId;
    // After a finished run, a one-line fix must not restart the whole workflow
    // with its gates: the human picks the workflow again to rerun it.
    setWorkflowSelection({ ...selection, workflowId: frameIdeaId || runFinished ? null : ran });
  }, [workflows, agents, mode, launchId, launchStepByStep, runWorkflowId, runFinished, frameIdeaId]);

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
