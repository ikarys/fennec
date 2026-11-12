import { describe, it, expect } from 'vitest';
import { sessionStatus } from './sessionStatus';
import type { RunView } from './runView';

const activeRun = { runId: 'r1' } as RunView;

describe('sessionStatus', () => {
  it('says an agent question inside an active run awaits the answer', () => {
    expect(sessionStatus('en', 'waiting_for_user', activeRun, undefined)).toEqual({
      label: 'Awaiting your answer',
      tone: 'accent',
      live: false,
    });
  });

  it('says a working run is running', () => {
    expect(sessionStatus('en', 'running', activeRun, undefined).live).toBe(true);
  });
});
