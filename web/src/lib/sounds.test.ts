import { describe, expect, it } from 'vitest';
import { playCue } from './sounds';

describe('playCue', () => {
  it('does not throw when no AudioContext exists', () => {
    expect(() => playCue('done')).not.toThrow();
  });
});
