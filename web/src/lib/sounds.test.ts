import { describe, expect, it } from 'vitest';
import { playCue, previewSounds } from './sounds';

describe('playCue', () => {
  it('does not throw when no AudioContext exists', () => {
    expect(() => playCue('done')).not.toThrow();
  });
});

describe('previewSounds', () => {
  it('does not throw when no AudioContext exists', () => {
    expect(() => previewSounds()).not.toThrow();
  });
});
