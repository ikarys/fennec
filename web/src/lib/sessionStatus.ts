import type { SessionState } from './api';
import { stateLabel } from './format';
import type { RunView, RunWaiting } from './workflowRun';

export type StatusTone = 'accent' | 'ok' | 'err' | 'neutral';

export interface SessionStatus {
  label: string;
  tone: StatusTone;
  live: boolean;
}

export function sessionStatus(state: SessionState, run: RunView | undefined, waiting: RunWaiting | undefined): SessionStatus {
  if (run?.finished) {
    const outcome = run.finished.outcome;
    if (outcome === 'done') return { label: 'Workflow terminé', tone: 'ok', live: false };
    if (outcome === 'blocked') return { label: 'Workflow bloqué', tone: 'err', live: false };
    if (outcome === 'stopped') return { label: 'Workflow arrêté', tone: 'neutral', live: false };
    if (outcome === 'error') return { label: 'Workflow en erreur', tone: 'err', live: false };
    return { label: outcome, tone: 'neutral', live: false };
  }
  if (waiting) {
    switch (waiting.kind) {
      case 'human':
        return { label: 'Attend ta décision', tone: 'accent', live: false };
      case 'pause':
        return { label: 'En pause', tone: 'accent', live: false };
      case 'interrupted':
        return { label: 'Interrompu', tone: 'err', live: false };
    }
  }
  if (run) return { label: 'Workflow en cours', tone: 'accent', live: true };
  if (state === 'running') return { label: 'En cours', tone: 'accent', live: true };
  if (state === 'waiting_for_user') return { label: 'Attend ta réponse', tone: 'accent', live: false };
  return { label: stateLabel(state), tone: 'neutral', live: false };
}
