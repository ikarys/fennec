// Shared tone helpers for the workflow run rail and its timeline.
import { portTone, type NodeRun, type UpcomingStep } from '../../../lib/workflowRun';

const PLAIN_PORTS = new Set(['next', 'done']);

export type RailStep =
  | { kind: 'run'; run: NodeRun }
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

  const { state, port, kind } = step.run;
  if (state === 'running') return kind === 'human' ? 'waiting' : 'running';
  if (state === 'interrupted') return 'interrupted';
  if (state === 'ended') return port === 'done' ? 'ok' : 'err';
  if (state === 'done') {
    const tone = portTone(port ?? '');
    return tone === 'negative' ? 'err' : 'ok';
  }

  return 'upcoming';
}

export function nodeRunTone(nodeRun: NodeRun): StepTone {
  return stepTone({ kind: 'run', run: nodeRun });
}
