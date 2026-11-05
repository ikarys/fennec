import { describe, expect, it } from 'vitest';
import type { ServerView } from './api';
import { serverBadge, serversPollMs } from './servers';

function server(overrides: Partial<ServerView> = {}): ServerView {
  return {
    name: 'web',
    command: 'npm run dev',
    suggested: false,
    state: 'stopped',
    running: false,
    url: null,
    exitCode: null,
    output: [],
    ...overrides,
  };
}

describe('serverBadge', () => {
  it('shows stopped as neutral without pulse', () => {
    expect(serverBadge('en', server({ state: 'stopped' }))).toEqual({
      text: 'stopped',
      tone: 'neutral',
      pulse: false,
    });
  });

  it('shows starting as info with pulse', () => {
    expect(serverBadge('en', server({ state: 'starting' }))).toEqual({
      text: 'starting…',
      tone: 'info',
      pulse: true,
    });
  });

  it('shows ready as ok with pulse', () => {
    expect(serverBadge('en', server({ state: 'ready' }))).toEqual({
      text: 'ready',
      tone: 'ok',
      pulse: true,
    });
  });

  it('shows exited with a code as err', () => {
    expect(serverBadge('en', server({ state: 'exited', exitCode: 1 }))).toEqual({
      text: 'exited (1)',
      tone: 'err',
      pulse: false,
    });
  });

  it('shows exited without a code as err', () => {
    expect(serverBadge('en', server({ state: 'exited', exitCode: null }))).toEqual({
      text: 'exited',
      tone: 'err',
      pulse: false,
    });
  });

  it('shows exit code 0 as neutral ended', () => {
    expect(serverBadge('en', server({ state: 'exited', exitCode: 0 }))).toEqual({
      text: 'ended',
      tone: 'neutral',
      pulse: false,
    });
  });

  it('uses the requested language', () => {
    expect(serverBadge('fr', server({ state: 'ready' })).text).toBe('prêt');
  });
});

describe('serversPollMs', () => {
  it('returns null when there are no servers', () => {
    expect(serversPollMs(null)).toBeNull();
  });

  it('returns null for an empty list', () => {
    expect(serversPollMs([])).toBeNull();
  });

  it('returns null when nothing is starting or ready', () => {
    expect(
      serversPollMs([
        server({ state: 'stopped' }),
        server({ state: 'exited', exitCode: 1 }),
      ]),
    ).toBeNull();
  });

  it('returns 1000 when a server is starting', () => {
    expect(serversPollMs([server({ state: 'starting' })])).toBe(1000);
  });

  it('returns 3000 when a server is ready', () => {
    expect(serversPollMs([server({ state: 'ready' })])).toBe(3000);
  });

  it('prefers starting over ready', () => {
    expect(
      serversPollMs([
        server({ state: 'ready' }),
        server({ state: 'starting' }),
      ]),
    ).toBe(1000);
  });
});
