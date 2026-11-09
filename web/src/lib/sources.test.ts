import { describe, it, expect } from 'vitest';
import { splitSources } from './sources';

describe('splitSources', () => {
  it('returns the text unchanged when there is no Sources section', () => {
    expect(splitSources('plain answer')).toEqual({ body: 'plain answer', sources: null });
  });

  it('splits a trailing Sources section and trims the body', () => {
    const text = 'Intro\n\n## Sources\n- a\n- b';
    expect(splitSources(text)).toEqual({ body: 'Intro', sources: '- a\n- b' });
  });

  it('does not split when a level-2 heading follows ## Sources', () => {
    const text = '## Sources\n- a\n## Next\ncontent';
    expect(splitSources(text)).toEqual({ body: text, sources: null });
  });

  it('uses the last ## Sources when two exist', () => {
    const text = '## Sources\nfirst\n## Notes\n## Sources\nsecond';
    expect(splitSources(text)).toEqual({ body: '## Sources\nfirst\n## Notes', sources: 'second' });
  });

  it('returns an empty sources string for a heading alone', () => {
    expect(splitSources('## Sources')).toEqual({ body: '', sources: '' });
  });

  it('does not split on ### Sources', () => {
    const text = '### Sources\n- a';
    expect(splitSources(text)).toEqual({ body: text, sources: null });
  });
});
