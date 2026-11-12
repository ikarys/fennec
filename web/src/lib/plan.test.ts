import { describe, it, expect } from 'vitest';
import {
  planFromProposal,
  reportFromSubmission,
  splitContract,
  pendingPlanOf,
  lastHumanMessage,
  pathParts,
  type Plan,
  type Report
} from './plan';
import type { ThreadItem } from './threadView';
import { SHIP_PROPOSAL, SHIP_PLAN, SHIP_SUBMISSION, SHIP_REPORT } from './contract.fixtures';

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

    expect(out.contract.doneWhen).toEqual([{ text: 'tests pass', check: '' }, { text: 'docs updated', check: '' }]);
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
      { number: 2, checked: false, text: 'docs updated', detail: '' },
      { number: 1, checked: true, text: 'tests pass', detail: '' },
    ]);
    expect(out.contract.changed).toEqual([
      { path: 'a.ts', what: 'added' },
      { path: 'b.ts', what: 'edited' },
    ]);
    expect(out.contract.test).toEqual(['run npm test']);
    expect(out.contract.todo).toEqual(['follow up']);
  });

  it('returns null when there is no contract heading', () => {
    expect(split('hello')).toBeNull();
    expect(split('## Notes\n- x')).toBeNull();
  });

  it('returns null for a plan heading with no steps', () => {
    expect(split('## Plan: X\nGoal: y')).toBeNull();
  });

  it('keeps a legacy criterion check apart from its text', () => {
    const out = split('## Plan: P\n### Step 1: Do\n### Done when\n- [ ] tests pass — check: `cargo test`\n- [ ] docs updated');
    expect(out?.contract.kind).toBe('plan');
    if (out?.contract.kind !== 'plan') return;
    expect(out.contract.doneWhen).toEqual([{ text: 'tests pass', check: 'cargo test' }, { text: 'docs updated', check: '' }]);
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
    expect(out.contract.test).toEqual([]);
    expect(out.contract.todo).toEqual([]);
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
  it('builds the recorded plan, numbering the steps and keeping each check apart', () => {
    const plan = planFromProposal(SHIP_PROPOSAL);
    expect(plan).toEqual(SHIP_PLAN);
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
    const report = reportFromSubmission(SHIP_SUBMISSION);
    expect(report).toEqual(SHIP_REPORT);
  });

  it('keeps empty values when the optional fields are absent', () => {
    const report = reportFromSubmission({ title: 'R', summary: 's' });
    expect(report.kind).toBe('report');
    expect(report.steps).toEqual([]);
    expect(report.doneWhen).toEqual([]);
    expect(report.changed).toEqual([]);
    expect(report.test).toEqual([]);
    expect(report.todo).toEqual([]);
  });

  it('lists the free text by line, or by sentence on one line', () => {
    const report = reportFromSubmission({ title: 'R', test: 'Open Tasks. Check the button on each tab. Create an idea.', todo: '- rerun vitest\n- update docs' });
    expect(report.test).toEqual(['Open Tasks.', 'Check the button on each tab.', 'Create an idea.']);
    expect(report.todo).toEqual(['rerun vitest', 'update docs']);
  });

  it('splits a path into its folder and file name', () => {
    expect(pathParts('web/src/a.tsx')).toEqual({ dir: 'web/src/', name: 'a.tsx' });
    expect(pathParts('README.md')).toEqual({ dir: '', name: 'README.md' });
  });
});

describe('pendingPlanOf', () => {
  it('returns null when no plan waits', () => {
    expect(pendingPlanOf(null)).toBeNull();
  });

  it('turns the served plan into the card plan and its preflight into views', () => {
    const found = pendingPlanOf({
      callId: 'call-1',
      plan: { title: 'Add feature', goal: 'Goal', steps: [{ title: 'Do' }], doneWhen: [{ text: 'tests pass', check: 'npm test' }], risks: [] },
      markdown: 'plan markdown',
      preflight: [
        { criterionId: 'c1', command: 'npm test', verdict: 'fails', exitCode: 1, output: 'fail', existenceOnly: false },
        { criterionId: 'c2', command: 'rg x', verdict: 'cannotRun', exitCode: null, output: '', existenceOnly: true }
      ]
    });
    expect(found?.id).toBe('call-1');
    expect(found?.plan.title).toBe('Add feature');
    expect(found?.plan.steps[0]?.number).toBe(1);
    expect(found?.markdown).toBe('plan markdown');
    expect(found?.preflight.map((result) => result.exitCode)).toEqual([1, undefined]);
  });
});

describe('lastHumanMessage', () => {
  it('skips system-generated user items', () => {
    const items: ThreadItem[] = [
      { kind: 'user', id: 'u1', content: 'system', isSystemGenerated: true, attachments: [] },
      { kind: 'user', id: 'u2', content: 'human', isSystemGenerated: false, attachments: [] },
    ];
    expect(lastHumanMessage(items)).toBe('human');
  });
});
