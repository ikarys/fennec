import type { FennecEvent } from './fold';
import type { NodeKind, WorkflowDef, WorkflowNode } from './workflow';

export type NodeRunState = 'running' | 'done' | 'ended';

export interface NodeRun {
  nodeId: string;
  label: string;
  kind: NodeKind;
  pass: number;
  state: NodeRunState;
  port?: string;
  childSessionId?: string;
}

export interface RunView {
  runId: string;
  workflow: WorkflowDef;
  task: string;
  stepByStep: boolean;
  /** Node runs in the order they started (a node run twice appears twice). */
  runs: NodeRun[];
  /** Nodes started and not finished. */
  open: string[];
  paused?: { from: string; port: string; to: string };
  finished?: { outcome: 'done' | 'blocked' | 'stopped' | 'error'; reason?: string };
  lastComment?: string;
}

export interface RunFold {
  /** Applies an event; returns true when it was a workflow event. */
  apply(event: FennecEvent): boolean;
  /** The latest run, or undefined when there is none. */
  view(): RunView | undefined;
}

function nodeById(def: WorkflowDef, nodeId: string): WorkflowNode | undefined {
  return def.nodes.find((node) => node.id === nodeId);
}

function labelOf(node: WorkflowNode | undefined, nodeId: string): string {
  return node?.name ?? nodeId;
}

function choicesOf(node: WorkflowNode | undefined): string[] {
  if (!node || !Array.isArray(node.choices)) return [];
  return node.choices.filter((choice): choice is string => typeof choice === 'string');
}

export function createRunFold(): RunFold {
  let run: RunView | undefined;

  const apply = (event: FennecEvent): boolean => {
    const data = event.data ?? {};
    switch (event.type) {
      case 'workflow.started': {
        run = {
          runId: String(data.runId ?? ''),
          workflow: data.workflow as WorkflowDef,
          task: String(data.task ?? ''),
          stepByStep: Boolean(data.stepByStep),
          runs: [],
          open: []
        };
        return true;
      }
      case 'workflow.node.started': {
        if (!run) return true;
        const nodeId = String(data.nodeId ?? '');
        const node = nodeById(run.workflow, nodeId);
        const nodeRun: NodeRun = {
          nodeId,
          label: labelOf(node, nodeId),
          kind: node?.kind ?? 'agent',
          pass: typeof data.pass === 'number' ? data.pass : 1,
          state: 'running'
        };
        if (typeof data.childSessionId === 'string') {
          nodeRun.childSessionId = data.childSessionId;
        }
        run.runs.push(nodeRun);
        run.open.push(nodeId);
        return true;
      }
      case 'workflow.node.finished': {
        if (!run) return true;
        const nodeId = String(data.nodeId ?? '');
        for (let i = run.runs.length - 1; i >= 0; i -= 1) {
          const candidate = run.runs[i];
          if (candidate.nodeId === nodeId && candidate.state === 'running') {
            candidate.state = 'done';
            candidate.port = String(data.port ?? '');
            break;
          }
        }
        const openIndex = run.open.lastIndexOf(nodeId);
        if (openIndex >= 0) {
          run.open.splice(openIndex, 1);
        }
        return true;
      }
      case 'workflow.paused': {
        if (!run) return true;
        run.paused = {
          from: String(data.from ?? ''),
          port: String(data.port ?? ''),
          to: String(data.to ?? '')
        };
        return true;
      }
      case 'workflow.resumed': {
        if (!run) return true;
        run.paused = undefined;
        run.open = [];
        run.lastComment = typeof data.comment === 'string' ? data.comment : undefined;
        return true;
      }
      case 'workflow.finished': {
        if (!run) return true;
        for (const nodeRun of run.runs) {
          if (nodeRun.state === 'running') {
            nodeRun.state = 'ended';
          }
        }
        run.open = [];
        run.paused = undefined;
        run.finished = {
          outcome: (data.outcome as 'done' | 'blocked' | 'stopped' | 'error') ?? 'error',
          reason: typeof data.reason === 'string' ? data.reason : undefined
        };
        return true;
      }
      default:
        return false;
    }
  };

  const view = (): RunView | undefined => {
    if (!run) return undefined;
    return {
      ...run,
      runs: [...run.runs],
      open: [...run.open]
    };
  };

  return { apply, view };
}

/** Where an active run waits, for the action card. Mirrors the server's `waiting`. */
export type RunWaiting =
  | { kind: 'human'; nodeId: string; question: string; choices: string[] }
  | { kind: 'pause'; to: string; toLabel: string }
  | { kind: 'interrupted' };

export function runWaiting(run: RunView | undefined, running: boolean, lastDone: string | undefined): RunWaiting | undefined {
  if (!run || run.finished || running) return undefined;
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
        question: typeof node.question === 'string' ? node.question : '',
        choices: choicesOf(node)
      };
    }
  }
  for (const nodeId of run.open) {
    const node = nodeById(run.workflow, nodeId);
    if (node?.kind === 'agent' && lastDone === 'waiting_for_user') {
      return undefined;
    }
  }
  return { kind: 'interrupted' };
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
  'minor'
]);

const NEGATIVE_PORTS = new Set([
  'fail',
  'failure',
  'findings',
  'false',
  'exhausted',
  'blocked',
  'stop',
  'revise'
]);

export function portTone(port: string): 'positive' | 'negative' | 'other' {
  const normalized = port.trim().toLowerCase();
  if (POSITIVE_PORTS.has(normalized)) return 'positive';
  if (NEGATIVE_PORTS.has(normalized)) return 'negative';
  return 'other';
}
