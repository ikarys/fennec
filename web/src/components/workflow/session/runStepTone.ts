// Shared tone helpers for the workflow run rail and its timeline.
import { portTone, type NodeRun, type UpcomingStep } from '../../../lib/runView';
import type { NodeKind } from '../../../lib/workflow';

const PLAIN_PORTS = new Set(['next', 'done']);

export type RailStep =
  | { kind: 'run'; run: NodeRun; nodeKind?: NodeKind }
  | { kind: 'upcoming'; step: UpcomingStep };

export type StepTone = 'ok' | 'err' | 'running' | 'waiting' | 'interrupted' | 'upcoming';

export const STEP_MARKER_CLASS: Record<StepTone, string> = {
  ok: 'bg-ok-soft text-ok',
  err: 'bg-err-soft text-err',
  running: 'bg-accent-soft text-accent',
  waiting: 'bg-attention-soft text-attention',
  interrupted: 'bg-raised text-muted',
  upcoming: 'border border-dashed border-border-strong',
};

export function isPlainPort(port: string): boolean {
  return PLAIN_PORTS.has(port.trim().toLowerCase());
}

export function stepTone(step: RailStep): StepTone {
  if (step.kind === 'upcoming') return 'upcoming';

  const { state, port } = step.run;
  // The served kind is a constant; the resolved kind from the definition wins.
  const kind = step.nodeKind ?? step.run.kind;
  if (state === 'running') return kind === 'human' ? 'waiting' : 'running';
  if (state === 'interrupted') return 'interrupted';
  if (state === 'ended') return port === 'done' ? 'ok' : 'err';
  if (state === 'done') {
    const tone = portTone(port ?? '');
    return tone === 'negative' ? 'err' : 'ok';
  }

  return 'upcoming';
}

export function nodeRunTone(nodeRun: NodeRun, nodeKind?: NodeKind): StepTone {
  return stepTone({ kind: 'run', run: nodeRun, nodeKind });
}
