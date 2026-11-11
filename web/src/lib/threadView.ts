// The thread as the session page shows it: what the fold builds from the events.
import type { NodeKind } from './workflow';
import type { RunView } from './workflowRun';
import type { Plan, Report } from './plan';

export interface FennecEvent {
  type: string;
  data: any;
}

export type ToolStatus = 'running' | 'success' | 'error';

export interface ToolCallView {
  id: string;
  name: string;
  args: Record<string, unknown>;
  status: ToolStatus;
  durationMs?: number;
  stdout: string;
  stderr: string;
  output: string;
  error?: string;
  truncated?: boolean;
  exitCode?: number;
  /** The human's rule that refused this call (fennec's own check: verified). */
  blockedBy?: { pattern: string; scope: string };
  addedLines?: number;
  removedLines?: number;
  contentPreview?: string[];
  diff?: {
    removed: string[];
    added: string[];
  };
  /** The question asked by `ask_user`, shown instead of raw arguments. */
  question?: string;
  image?: { dataUrl: string; mimeType: string };
}

export type Segment =
  | { kind: 'thinking'; id: string; content: string }
  | { kind: 'text'; id: string; content: string }
  | { kind: 'tool'; id: string; tool: ToolCallView }
  | { kind: 'plan'; id: string; plan: Plan; markdown: string }
  | { kind: 'report'; id: string; report: Report; markdown: string };

export interface PreflightResultView {
  criterionId: string;
  command: string;
  verdict: 'fails' | 'passesAlready' | 'cannotRun';
  exitCode?: number;
  output: string;
  existenceOnly?: boolean;
}

export interface AssistantItem {
  kind: 'assistant';
  id: string;
  segments: Segment[];
  done: boolean;
  /** Tokens per second while the message streams, estimated by the server. */
  liveSpeed?: number;
  /** The plan's checks run by fennec before any change, from `criteria.preflight`. */
  preflight?: PreflightResultView[];
}

export interface WorkflowNodeSpan {
  offsetMs: number;
  durationMs: number;
}

export interface WorkflowNodeStat {
  nodeId: string;
  label: string;
  runs: number;
  workingMs: number;
  spans: WorkflowNodeSpan[];
}

export interface WorkflowStats {
  workingMs: number;
  /** The run's wall-clock start and end (ms); absent in older logs. */
  startedAt?: number;
  finishedAt?: number;
  nodes: WorkflowNodeStat[];
  reruns: number;
  filesChanged: number;
  criteriaTotal: number;
  criteriaPassed: number;
  criteriaVerified: number;
}

export type ThreadItem =
  | { kind: 'user'; id: string; content: string; isSystemGenerated: boolean; attachments: { id: string; filename: string; data: string }[] }
  | AssistantItem
  | { kind: 'mode'; id: string; name: string; color: string }
  | { kind: 'error'; id: string; message: string }
  | { kind: 'note'; id: string; text: string; detail?: string }
  | { kind: 'workflow-node'; id: string; nodeId: string; label: string; nodeKind: NodeKind; pass: number; port?: string; output?: string; exitCode?: number; childSessionId?: string; prompt?: string; durationMs?: number; startedAt?: number }
  | { kind: 'workflow-end'; id: string; runId: string; outcome: 'done' | 'blocked' | 'stopped' | 'error'; reason?: string; stats?: WorkflowStats };

export interface PendingQuestion {
  callId: string;
  question: string;
  type: 'text' | 'confirm' | 'choice';
  options: {
    value: string;
    label: string;
    description?: string;
    tone?: string;
  }[];
}

export interface ThreadView {
  items: ThreadItem[];
  running: boolean;
  /** Reason of the last `chat.done`: the session state once a turn ends. */
  lastDone?: string;
  /** The unanswered `ask_user` question, if any. */
  pendingQuestion?: PendingQuestion;
  /** The tool call waiting for the human's approval, if any. */
  /**
   * The last `mode.changed` value, or null when the session detail should
   * provide the server's resolved mode.
   */
  mode: string | null;
  /** The latest workflow run, if any. */
  run?: RunView;
  /** The last generated session name, if any (last one wins). */
  name?: string;
}
