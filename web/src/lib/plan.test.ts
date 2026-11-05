import { describe, it, expect } from 'vitest';
import { planFromProposal, reportFromSubmission, splitContract, type Plan, type Report } from './plan';

const split = (text: string) => splitContract(text);

/** The contract the text must hold, typed by its kind: a test fails loudly when it is missing. */
function contractOf<K extends 'plan' | 'report'>(text: string, kind: K) {
  const out = split(text);
  if (!out || out.contract.kind !== kind) throw new Error(`expected a ${kind} in:\n${text}`);
  return { before: out.before, contract: out.contract as K extends 'plan' ? Plan : Report };
}
const plan = (text: string) => contractOf(text, 'plan');
const report = (text: string) => contractOf(text, 'report');

describe('splitContract', () => {
  it('reads a plan heading with a qualifier before the colon', () => {
    for (const heading of ['## Plan révisé : Ship', '## Revised Plan: Ship', '### Plan : Ship']) {
      const out = plan(`${heading}\n### Step 1: Do\n- Change: it\n### Done when\n- [ ] tests pass`);
      expect(out.contract.title).toBe('Ship');
    }
    expect(split('## Planning: later\n### Step 1: Do\n### Done when\n- [ ] x')).toBeNull();
  });

  it('parses a plan with steps, done when, and risks', () => {
    const text = [
      'Intro',
      '## Plan: Ship feature',
      'Goal: Ship it',
      '## Step 1: Add code',
      '- Files: `a.ts`, `b.ts`',
      '- Change: implement',
      '- Verify: run tests',
      '### Step 2: Update docs',
      '- Files: docs.md (new)',
      '- Change: update docs',
      '- Verify: read docs',
      '### Done when',
      '- [ ] tests pass',
      '- [x] docs updated',
      '### Risks',
      '- none',
    ].join('\n');

    const out = plan(text);
    expect(out.before).toBe('Intro');
    expect(out.contract.kind).toBe('plan');
    expect(out.contract.title).toBe('Ship feature');
    expect(out.contract.goal).toBe('Ship it');
    expect(out.contract.steps).toHaveLength(2);

    expect(out.contract.steps[0].number).toBe(1);
    expect(out.contract.steps[0].title).toBe('Add code');
    expect(out.contract.steps[0].files).toEqual(['a.ts', 'b.ts']);
    expect(out.contract.steps[0].change).toBe('implement');
    expect(out.contract.steps[0].verify).toBe('run tests');

    expect(out.contract.steps[1].number).toBe(2);
    expect(out.contract.steps[1].title).toBe('Update docs');
    expect(out.contract.steps[1].files).toEqual(['docs.md (new)']);

    expect(out.contract.doneWhen).toEqual(['tests pass', 'docs updated']);
    expect(out.contract.risks).toEqual([]);
  });

  it('parses a report with checked steps, changed files, and next actions', () => {
    const text = [
      'Before',
      '## Report: Done',
      'Summary: All good',
      '## Steps',
      '- [x] Step 1: Add feature — implemented',
      '- [ ] Step 2: Update docs - pending',
      '### Done when',
      '- [x] tests pass',
      '- [ ] docs updated',
      '### Changed',
      '- `a.ts`: added',
      '- b.ts: edited',
      '### Next',
      '- Test: run npm test',
      '- To do: follow up',
    ].join('\n');

    const out = report(text);
    expect(out.before).toBe('Before');
    expect(out.contract.kind).toBe('report');
    expect(out.contract.title).toBe('Done');
    expect(out.contract.summary).toBe('All good');

    expect(out.contract.steps).toHaveLength(2);
    expect(out.contract.steps[0]).toEqual({
      number: 1,
      checked: true,
      text: 'Add feature',
      detail: 'implemented',
    });
    expect(out.contract.steps[1]).toEqual({
      number: 2,
      checked: false,
      text: 'Update docs',
      detail: 'pending',
    });

    expect(out.contract.doneWhen).toEqual([
      { checked: true, text: 'tests pass', detail: '' },
      { checked: false, text: 'docs updated', detail: '' },
    ]);
    expect(out.contract.changed).toEqual([
      { path: 'a.ts', what: 'added' },
      { path: 'b.ts', what: 'edited' },
    ]);
    expect(out.contract.test).toBe('run npm test');
    expect(out.contract.todo).toBe('follow up');
  });

  it('returns null when there is no contract heading', () => {
    expect(split('hello')).toBeNull();
    expect(split('## Notes\n- x')).toBeNull();
  });

  it('returns null for a plan heading with no steps', () => {
    expect(split('## Plan: X\nGoal: y')).toBeNull();
  });

  it('returns null for a report heading with no steps or done when', () => {
    expect(split('## Report: X\nSummary: y')).toBeNull();
  });

  it('uses the last Plan or Report heading as the contract', () => {
    const text = [
      '## Plan: First',
      '## Step 1: A',
      '- Change: a',
      '## Plan: Second',
      '## Step 1: B',
      '- Change: b',
    ].join('\n');

    const out = plan(text);
    expect(out.before).toBe('## Plan: First\n## Step 1: A\n- Change: a');
    expect(out.contract.title).toBe('Second');
    expect(out.contract.steps[0].change).toBe('b');
  });

  it('keeps empty fields for a partial plan step', () => {
    const out = plan('## Plan: P\n## Step 1: Title');
    expect(out.contract.steps[0].files).toEqual([]);
    expect(out.contract.steps[0].change).toBe('');
    expect(out.contract.steps[0].verify).toBe('');
  });

  it('appends wrapped non-bullet lines to the previous field', () => {
    const out = plan('## Plan: P\n## Step 1: Title\n- Change: first line\nsecond line');
    expect(out.contract.steps[0].change).toBe('first line second line');
  });

  it('accepts bold labels and star bullets', () => {
    const out = plan('## Plan: P\n## Step 1: Title\n* **Files:** `a.ts`');
    expect(out.contract.steps[0].files).toEqual(['a.ts']);
  });

  it('falls back to comma splitting for files without backticks', () => {
    const out = plan('## Plan: P\n## Step 1: Title\n- Files: a.ts, , b.ts');
    expect(out.contract.steps[0].files).toEqual(['a.ts', 'b.ts']);
  });

  it('filters none-like risks', () => {
    const out = plan('## Plan: P\n## Step 1: Title\n### Risks\n- None.\n- A real risk');
    expect(out.contract.risks).toEqual(['A real risk']);
  });

  it('treats Nothing and Rien as empty next actions in reports', () => {
    const out = report('## Report: R\n## Steps\n- [x] Step 1: Done\n### Next\n- Test: Nothing.\n- To do: Rien.');
    expect(out.contract.test).toBe('');
    expect(out.contract.todo).toBe('');
  });

  it('parses report steps without checkboxes as unchecked', () => {
    const out = report('## Report: R\n## Steps\n- some text');
    expect(out.contract.steps).toEqual([
      { number: 1, checked: false, text: 'some text', detail: '' },
    ]);
  });

  it('uses the explicit step number when present in report steps', () => {
    const out = report('## Report: R\n## Steps\n- [x] Step 3: Title');
    expect(out.contract.steps[0].number).toBe(3);
  });

  it('ignores changed lines without a colon', () => {
    const out = report('## Report: R\n## Steps\n- [x] Step 1: Done\n### Changed\n- no colon here');
    expect(out.contract.changed).toEqual([]);
  });
});

describe('planFromProposal', () => {
  it('builds the recorded plan, numbering the steps and formatting the checks', () => {
    const plan = planFromProposal({
      title: 'Ship',
      goal: 'Goal',
      steps: [
        { title: 'Add code', files: ['a.ts'], change: 'implement' },
        { title: 'Docs', files: [], change: 'update' }
      ],
      doneWhen: [
        { text: 'tests pass', check: 'cargo test' },
        { text: 'docs updated' }
      ],
      risks: ['Risk one']
    });
    expect(plan).toEqual({
      kind: 'plan',
      title: 'Ship',
      goal: 'Goal',
      steps: [
        { number: 1, title: 'Add code', files: ['a.ts'], change: 'implement', verify: '' },
        { number: 2, title: 'Docs', files: [], change: 'update', verify: '' }
      ],
      doneWhen: ['tests pass — check: `cargo test`', 'docs updated'],
      risks: ['Risk one']
    });
  });

  it('keeps empty arrays when the optional fields are absent', () => {
    const plan = planFromProposal({ title: 'P', goal: '' });
    expect(plan.kind).toBe('plan');
    expect(plan.steps).toEqual([]);
    expect(plan.doneWhen).toEqual([]);
    expect(plan.risks).toEqual([]);
  });
});

describe('reportFromSubmission', () => {
  it('builds the recorded report, numbering the steps and keeping the checks', () => {
    const report = reportFromSubmission({
      title: 'Ship',
      summary: 'What changed',
      steps: [
        { title: 'Add code', done: true, note: 'implemented' },
        { title: 'Docs', done: false }
      ],
      doneWhen: [
        { text: 'tests pass', met: true, how: 'cargo test' },
        { text: 'docs updated', met: false }
      ],
      changed: [{ path: 'a.ts', what: 'the code' }],
      test: 'Open the UI',
      todo: ''
    });
    expect(report).toEqual({
      kind: 'report',
      title: 'Ship',
      summary: 'What changed',
      steps: [
        { number: 1, checked: true, text: 'Add code', detail: 'implemented' },
        { number: 2, checked: false, text: 'Docs', detail: '' }
      ],
      doneWhen: [
        { checked: true, text: 'tests pass', detail: 'cargo test' },
        { checked: false, text: 'docs updated', detail: '' }
      ],
      changed: [{ path: 'a.ts', what: 'the code' }],
      test: 'Open the UI',
      todo: ''
    });
  });

  it('keeps empty values when the optional fields are absent', () => {
    const report = reportFromSubmission({ title: 'R', summary: 's' });
    expect(report.kind).toBe('report');
    expect(report.steps).toEqual([]);
    expect(report.doneWhen).toEqual([]);
    expect(report.changed).toEqual([]);
    expect(report.test).toBe('');
    expect(report.todo).toBe('');
  });
});
