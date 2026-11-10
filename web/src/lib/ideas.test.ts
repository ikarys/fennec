import { describe, expect, it } from 'vitest';
import { framingPrefill } from './ideas';

describe('framingPrefill', () => {
  it('appends the task sheet', () => {
    const idea = { text: 'Do the thing', attachments: [], sheetMarkdown: '## Plan: Thing\n\nGoal: do it' };
    expect(framingPrefill(idea).text).toBe('Do the thing\n\n## Plan: Thing\n\nGoal: do it');
  });
});
