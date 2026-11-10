import { describe, expect, it } from 'vitest';
import { cueFor } from './useTurnNotices';

describe('cueFor', () => {
  it('prefers problem over yourTurn', () => {
    expect(cueFor(['error', 'question'], false)).toBe('problem');
  });

  it('prefers yourTurn over done', () => {
    expect(cueFor(['question', 'awaitingIntegration'], false)).toBe('yourTurn');
  });

  it('uses problem for a budget alone', () => {
    expect(cueFor([], true)).toBe('problem');
  });

  it('returns null when nothing is new', () => {
    expect(cueFor([], false)).toBeNull();
  });
});
