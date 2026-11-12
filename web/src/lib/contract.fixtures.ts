// Test fixtures: the "Ship" plan and report, as a model submits them and as fennec records them.
export const SHIP_PROPOSAL = {
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
};

export const SHIP_PLAN = {
  kind: 'plan',
  title: 'Ship',
  goal: 'Goal',
  steps: [
    { number: 1, title: 'Add code', files: ['a.ts'], change: 'implement', verify: '' },
    { number: 2, title: 'Docs', files: [], change: 'update', verify: '' }
  ],
  doneWhen: [{ text: 'tests pass', check: 'cargo test' }, { text: 'docs updated', check: '' }],
  risks: ['Risk one']
};

export const SHIP_SUBMISSION = {
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
};

export const SHIP_REPORT = {
  kind: 'report',
  title: 'Ship',
  summary: 'What changed',
  steps: [
    { number: 1, checked: true, text: 'Add code', detail: 'implemented' },
    { number: 2, checked: false, text: 'Docs', detail: '' }
  ],
  doneWhen: [
    { number: 2, checked: false, text: 'docs updated', detail: '' },
    { number: 1, checked: true, text: 'tests pass', detail: 'cargo test' }
  ],
  changed: [{ path: 'a.ts', what: 'the code' }],
  test: ['Open the UI'],
  todo: []
};
