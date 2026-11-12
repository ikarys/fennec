import { describe, expect, it } from 'vitest';
import { splitTitle } from './titleLine';

describe('splitTitle', () => {
  it('takes the first non-empty line without Markdown marks', () => {
    expect(splitTitle('\n## Keep the **sort**?\n\nIt sorts by date.')).toEqual({
      title: 'Keep the sort?',
      rest: 'It sorts by date.'
    });
  });

  it('leaves no rest for a one-line text', () => {
    expect(splitTitle('Go on?')).toEqual({ title: 'Go on?', rest: '' });
  });
});
