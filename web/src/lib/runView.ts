import type { NodeKind, WorkflowDef } from './workflow';

export type NodeRunState = 'running' | 'done' | 'ended' | 'interrupted';

/** One node run, in the order it started (a node run twice appears twice). */
export interface NodeRun {
  nodeId: string;
  label: string;
  kind: NodeKind;
  pass: number;
  state: NodeRunState;
  port?: string;
  childSessionId?: string;
  /** Unix ms; absent in older logs. */
  startedAt?: number;
  /** The node's working time in ms; none for a human node or a session-context agent node. */
  durationMs?: number;
  /** What the node returned (its report, a check's output); shown when it failed. */
  output?: string;
}

/** A step the run has not reached yet, on its nominal path. */
export interface UpcomingStep {
  nodeId: string;
  label: string;
}

/** The completion counts the rail's status line reads. */
export interface RunCounts {
  done: number;
  open: number;
  total: number;
}

/** The run, pre-calculated by the server: the UI renders it as it is. */
export interface RunView {
  runId: string;
  workflow: WorkflowDef;
  task: string;
  stepByStep: boolean;
  /** The human asked to pause at the next edge; cleared once paused. */
  pauseRequested: boolean;
  runs: NodeRun[];
  /** Nodes started and not finished. */
  open: string[];
  paused?: { from: string; port: string; to: string };
  /** Human nodes' questions with their {{…}} names filled in by the server. */
  questions: Record<string, string>;
  finished?: {
    outcome: 'done' | 'blocked' | 'stopped' | 'error';
    reason?: string;
  };
  lastComment?: string;
  /** Node runs split into rounds; a repeated node opens a new round. */
  rounds: NodeRun[][];
  /** The steps still to come, on the run's nominal path. */
  upcoming: UpcomingStep[];
  counts: RunCounts;
}

const POSITIVE_PORTS = new Set([
  'next',
  'done',
  'pass',
  'success',
  'clean',
  'true',
  'go',
  'ok',
  'accept',
  'minor',
]);

const NEGATIVE_PORTS = new Set([
  'fail',
  'failure',
  'findings',
  'false',
  'exhausted',
  'blocked',
  'stop',
  'revise',
]);

/** A port name's tone: the badge and marker colours read it. */
export function portTone(port: string): 'positive' | 'negative' | 'other' {
  const normalized = port.trim().toLowerCase();
  if (POSITIVE_PORTS.has(normalized)) return 'positive';
  if (NEGATIVE_PORTS.has(normalized)) return 'negative';
  return 'other';
}

/** Where an active run waits, for the action card. Mirrors the server's `waiting`. */
export type RunWaiting =
  | { kind: 'human'; nodeId: string; question: string; choices: string[] }
  | { kind: 'pause'; to: string; toLabel: string }
  | { kind: 'interrupted' }
  | { kind: 'child'; label: string; childSessionId: string }
  | { kind: 'relaunch' };

function nodeById(def: WorkflowDef, nodeId: string) {
  return def.nodes.find((node) => node.id === nodeId);
}

/**
 * The real kind of a node from the run's definition. The served `NodeRun.kind`
 * is a constant (the server does not re-serialize each node's kind), so the UI
 * resolves the kind here to tell human/criteria/shell nodes apart.
 */
export function nodeKindOf(workflow: WorkflowDef, nodeId: string): NodeKind | undefined {
  return nodeById(workflow, nodeId)?.kind;
}

function labelOf(node: WorkflowDef['nodes'][number] | undefined, nodeId: string): string {
  return node?.name ?? nodeId;
}

function choicesOf(node: WorkflowDef['nodes'][number] | undefined): string[] {
  if (!node || !Array.isArray(node.choices)) return [];
  return node.choices.filter((choice): choice is string => typeof choice === 'string');
}

export function runWaiting(
  run: RunView | undefined,
  running: boolean,
  lastDone: string | undefined,
): RunWaiting | undefined {
  if (!run || running) return undefined;
  if (run.finished) {
    // A run that ended in error can be relaunched on its failed node.
    return run.finished.outcome === 'error' ? { kind: 'relaunch' } : undefined;
  }
  if (run.paused) {
    const toLabel = labelOf(nodeById(run.workflow, run.paused.to), run.paused.to);
    return { kind: 'pause', to: run.paused.to, toLabel };
  }
  for (const nodeId of run.open) {
    const node = nodeById(run.workflow, nodeId);
    if (node?.kind === 'human') {
      return {
        kind: 'human',
        nodeId,
        // Runs from before workflow.human.asked show the raw question.
        question:
          run.questions[nodeId] ??
          (typeof node.question === 'string' ? node.question : ''),
        choices: choicesOf(node),
      };
    }
  }
  for (const nodeId of run.open) {
    const node = nodeById(run.workflow, nodeId);
    if (node?.kind === 'agent' && lastDone === 'waiting_for_user') {
      // A fresh step may have asked in its own session; point the parent there.
      const latest = [...run.runs].reverse().find((r) => r.nodeId === nodeId);
      if (latest?.childSessionId) {
        return { kind: 'child', label: labelOf(node, nodeId), childSessionId: latest.childSessionId };
      }
      return undefined;
    }
  }
  return { kind: 'interrupted' };
}
