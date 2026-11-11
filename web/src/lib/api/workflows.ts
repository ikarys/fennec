import { request } from "./request";
import type { AttachmentInput } from "./sessions";
import type {
  Origin,
  WorkflowDef,
  WorkflowIssue,
  WorkflowSummary,
} from "../workflow";
import type { RunView } from "../runView";

export class WorkflowSaveError extends Error {
  issues: WorkflowIssue[];
  constructor(message: string, issues: WorkflowIssue[]) {
    super(message);
    this.name = "WorkflowSaveError";
    this.issues = issues;
  }
}

export function listWorkflows(projectId: string): Promise<WorkflowSummary[]> {
  return request<WorkflowSummary[]>(
    `/api/projects/${encodeURIComponent(projectId)}/workflows`,
  );
}

export function getWorkflow(
  projectId: string,
  wid: string,
): Promise<{ origin: Origin; workflow: WorkflowDef }> {
  return request<{ origin: Origin; workflow: WorkflowDef }>(
    `/api/projects/${encodeURIComponent(projectId)}/workflows/${encodeURIComponent(wid)}`,
  );
}

export async function saveWorkflow(
  projectId: string,
  wid: string,
  scope: "project" | "global",
  workflow: WorkflowDef,
): Promise<{ origin: Origin; workflow: WorkflowDef }> {
  try {
    return await request<{ origin: Origin; workflow: WorkflowDef }>(
      `/api/projects/${encodeURIComponent(projectId)}/workflows/${encodeURIComponent(wid)}`,
      {
        method: "PUT",
        body: JSON.stringify({ scope, workflow }),
      },
    );
  } catch (error) {
    if (error instanceof Error) {
      const issues = (error as { issues?: WorkflowIssue[] }).issues ?? [];
      throw new WorkflowSaveError(error.message, issues);
    }
    throw error;
  }
}

export function validateWorkflow(
  projectId: string,
  workflow: WorkflowDef,
): Promise<WorkflowIssue[]> {
  return request<{ errors: WorkflowIssue[] }>("/api/workflows/validate", {
    method: "POST",
    body: JSON.stringify({ projectId, workflow }),
  }).then((result) => result.errors);
}

export function startWorkflow(
  sessionId: string,
  workflowId: string,
  task: string,
  stepByStep: boolean,
  worktree: boolean,
  attachments: AttachmentInput[] = [],
  plan?: string,
): Promise<void> {
  return request<void>(
    `/api/sessions/${encodeURIComponent(sessionId)}/workflow`,
    {
      method: "POST",
      body: JSON.stringify({
        workflowId,
        task,
        stepByStep,
        worktree,
        ...(attachments.length > 0 ? { attachments } : {}),
        ...(plan ? { plan } : {}),
      }),
    },
  );
}

export function answerWorkflow(
  sessionId: string,
  runId: string,
  choice: string,
  comment: string,
): Promise<void> {
  return request<void>(
    `/api/sessions/${encodeURIComponent(sessionId)}/workflow/answer`,
    {
      method: "POST",
      body: JSON.stringify({ runId, choice, comment }),
    },
  );
}

export function continueWorkflow(
  sessionId: string,
  runId: string,
  comment: string,
): Promise<void> {
  return request<void>(
    `/api/sessions/${encodeURIComponent(sessionId)}/workflow/continue`,
    {
      method: "POST",
      body: JSON.stringify({ runId, comment }),
    },
  );
}

export function resumeWorkflow(
  sessionId: string,
  runId: string,
): Promise<void> {
  return request<void>(
    `/api/sessions/${encodeURIComponent(sessionId)}/workflow/resume`,
    {
      method: "POST",
      body: JSON.stringify({ runId }),
    },
  );
}

export function stopWorkflow(sessionId: string, runId: string): Promise<void> {
  return request<void>(
    `/api/sessions/${encodeURIComponent(sessionId)}/workflow/stop`,
    {
      method: "POST",
      body: JSON.stringify({ runId }),
    },
  );
}

export function pauseWorkflow(sessionId: string, runId: string): Promise<void> {
  return request<void>(
    `/api/sessions/${encodeURIComponent(sessionId)}/workflow/pause`,
    {
      method: "POST",
      body: JSON.stringify({ runId }),
    },
  );
}

/**
 * The run view, pre-calculated by the server: steps, states, ports, rounds,
 * upcoming steps and completion counts; the run rail renders it as it is.
 */
export function getRunView(id: string): Promise<RunView> {
  return request<RunView>(
    `/api/sessions/${encodeURIComponent(id)}/workflow/run/view`,
  );
}

/** A workflow node run's token line, written by the server from the usage ledger. */
export interface NodeUsage {
  /** The thread item it belongs under. */
  itemId: string;
  line: string;
  detail: string;
}

export interface RunUsage {
  runId: string;
  calls: string;
  generated: string;
  read: string;
  cache: string;
}

export interface WorkflowUsage {
  nodes: NodeUsage[];
  runs: RunUsage[];
}

export function getUsageWorkflow(id: string): Promise<WorkflowUsage> {
  return request<WorkflowUsage>(
    `/api/sessions/${encodeURIComponent(id)}/usage/workflow`,
  );
}
