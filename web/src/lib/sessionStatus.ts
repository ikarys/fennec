import type { SessionState } from './api';
import { stateLabel } from './format';
import { t, type Lang } from '../i18n/messages';
import type { RunView, RunWaiting } from './workflowRun';

export type StatusTone = 'accent' | 'ok' | 'err' | 'neutral';

export interface SessionStatus {
  label: string;
  tone: StatusTone;
  live: boolean;
}

export function sessionStatus(lang: Lang, state: SessionState, run: RunView | undefined, waiting: RunWaiting | undefined): SessionStatus {
  if (run?.finished) {
    const outcome = run.finished.outcome;
    if (outcome === 'done') return { label: t(lang, 'workflowDone'), tone: 'ok', live: false };
    if (outcome === 'blocked') return { label: t(lang, 'workflowBlocked'), tone: 'err', live: false };
    if (outcome === 'stopped') return { label: t(lang, 'workflowStopped'), tone: 'neutral', live: false };
    if (outcome === 'error') return { label: t(lang, 'workflowError'), tone: 'err', live: false };
    return { label: outcome, tone: 'neutral', live: false };
  }
  if (waiting) {
    switch (waiting.kind) {
      case 'human':
        return { label: t(lang, 'awaitDecision'), tone: 'accent', live: false };
      case 'pause':
        return { label: t(lang, 'paused'), tone: 'accent', live: false };
      case 'interrupted':
        return { label: t(lang, 'interrupted'), tone: 'err', live: false };
    }
  }
  if (run) return { label: t(lang, 'workflowRunning'), tone: 'accent', live: true };
  if (state === 'running') return { label: t(lang, 'running'), tone: 'accent', live: true };
  if (state === 'waiting_for_user') return { label: t(lang, 'awaitAnswer'), tone: 'accent', live: false };
  return { label: stateLabel(lang, state), tone: 'neutral', live: false };
}
