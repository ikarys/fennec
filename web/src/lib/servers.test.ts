import { describe, expect, it } from 'vitest';
import type { ServerView } from './api';
import { portLabel, serverBadge, serversPollMs } from './servers';

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
    port: null,
    extraUrls: [],
    portIgnored: false,
    exitSignal: null,
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

  it('shows exited by a signal as attention', () => {
    expect(serverBadge('en', server({ state: 'exited', exitSignal: 15 }))).toEqual({
      text: 'stopped by signal 15',
      tone: 'attention',
      pulse: false,
    });
  });

  it('uses the requested language', () => {
    expect(serverBadge('fr', server({ state: 'ready' })).text).toBe('prêt');
  });
});

describe('portLabel', () => {
  it('returns the port when the URL has one', () => {
    expect(portLabel('http://127.0.0.1:5180/')).toBe(':5180');
  });

  it('returns the URL when it has no explicit port', () => {
    expect(portLabel('http://example.test/')).toBe('http://example.test/');
  });

  it('returns the input when it does not parse', () => {
    expect(portLabel('not a url')).toBe('not a url');
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
