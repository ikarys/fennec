import { describe, it, expect } from 'vitest';
import type { Blocker, IntegrationPreview, IntegrationResult } from './api';
import {
  acknowledgedKinds,
  blockerSentence,
  gateState,
  narrativePrompt,
  neverAcknowledged,
} from './integration';
import { t } from '../i18n/messages';

function preview(overrides: Partial<IntegrationPreview> = {}): IntegrationPreview {
  return {
    branch: 'fennec/x',
    base: 'develop',
    humanBranch: 'develop',
    originBranch: null,
    defaultMessage: 'feat: default',
    commits: [],
    files: [],
    commonSignals: [],
    blockers: [],
    criteria: [],
    decisions: [],
    testFiles: [],
    idea: null,
    ...overrides,
  };
}

function blocker<T extends Blocker>(value: T): T {
  return value;
}

describe('never acknowledged', () => {
  it('returns true for no commits', () => {
    expect(neverAcknowledged('noCommits')).toBe(true);
  });

  it('returns true for conflict', () => {
    expect(neverAcknowledged('conflict')).toBe(true);
  });

  it('returns true for local changes', () => {
    expect(neverAcknowledged('localChanges')).toBe(true);
  });

  it('returns false for no criteria', () => {
    expect(neverAcknowledged('noCriteria')).toBe(false);
  });

  it('returns false for protected touched', () => {
    expect(neverAcknowledged('protectedTouched')).toBe(false);
  });
});

describe('narrative prompt', () => {
  it('names branch and base, lists files, and ends with diff instruction', () => {
    const prompt = narrativePrompt(
      preview({
        files: [
          {
            path: 'a.rs',
            added: 3,
            removed: 1,
            protected: false,
            carried: false,
            signals: ['hot', 'untested'],
          },
          {
            path: 'b.rs',
            added: 0,
            removed: 2,
            protected: false,
            carried: false,
            signals: [],
          },
        ],
      }),
    );

    expect(prompt).toContain('`fennec/x`');
    expect(prompt).toContain('`develop`');
    expect(prompt).toContain('1. a.rs (+3 −1) — signals: hot, untested');
    expect(prompt).toContain('2. b.rs (+0 −2)');
    expect(prompt).not.toContain('2. b.rs (+0 −2) — signals');
    expect(prompt).toContain('git diff develop fennec/x -- <path>');
  });
});

describe('blocker sentence', () => {
  it('renders no commits', () => {
    const sentence = blockerSentence('fr', blocker({ kind: 'noCommits' }));

    expect(sentence).toBe(t('fr', 'noCommits'));
  });

  it('renders conflict', () => {
    const sentence = blockerSentence('fr', blocker({ kind: 'conflict', files: ['a.rs', 'b.rs'] }));

    expect(sentence).toBe(t('fr', 'conflict', { files: 'a.rs, b.rs' }));
  });

  it('renders criteria not passed', () => {
    const sentence = blockerSentence('fr', blocker({ kind: 'criteriaNotPassed', criteria: ['ci', 'lint'] }));

    expect(sentence).toBe(t('fr', 'criteriaNotPassed', { criteria: 'ci, lint' }));
  });

  it('renders no criteria', () => {
    const sentence = blockerSentence('fr', blocker({ kind: 'noCriteria' }));

    expect(sentence).toBe(t('fr', 'noCriteria'));
  });

  it('renders protected touched', () => {
    const sentence = blockerSentence('fr', blocker({ kind: 'protectedTouched', paths: ['core.rs'] }));

    expect(sentence).toBe(t('fr', 'protectedTouched', { paths: 'core.rs' }));
  });

  it('renders sensitive touched', () => {
    const sentence = blockerSentence('fr', blocker({ kind: 'sensitiveTouched', paths: ['auth.rs'] }));

    expect(sentence).toBe(t('fr', 'sensitiveTouched', { paths: 'auth.rs' }));
  });

  it('renders architecture violated', () => {
    const sentence = blockerSentence(
      'fr',
      blocker({
        kind: 'architectureViolated',
        violations: [
          { path: 'a.rs', line: 'l1', prefix: '', forbidden: 'f1' },
        ],
      }),
    );

    expect(sentence).toBe(t('fr', 'architectureViolated', { list: 'a.rs : « l1 » (interdit : f1)', more: '' }));
  });

  it('renders architecture violated with more than three violations', () => {
    const sentence = blockerSentence(
      'fr',
      blocker({
        kind: 'architectureViolated',
        violations: [
          { path: 'a.rs', line: 'l1', prefix: '', forbidden: 'f1' },
          { path: 'b.rs', line: 'l2', prefix: '', forbidden: 'f2' },
          { path: 'c.rs', line: 'l3', prefix: '', forbidden: 'f3' },
          { path: 'd.rs', line: 'l4', prefix: '', forbidden: 'f4' },
          { path: 'e.rs', line: 'l5', prefix: '', forbidden: 'f5' },
        ],
      }),
    );

    expect(sentence).toBe(
      t('fr', 'architectureViolated', {
        list: 'a.rs : « l1 » (interdit : f1) ; b.rs : « l2 » (interdit : f2) ; c.rs : « l3 » (interdit : f3)',
        more: ' (+2)'
      }),
    );
  });

  it('renders suppressions added', () => {
    const sentence = blockerSentence(
      'fr',
      blocker({
        kind: 'suppressionsAdded',
        suppressions: [
          { path: 'a.rs', line: 'l1', marker: 'm1' },
        ],
      }),
    );

    expect(sentence).toBe(t('fr', 'suppressionsAdded', { list: 'a.rs : « l1 » (m1)', more: '' }));
  });

  it('renders suppressions added with more than three suppressions', () => {
    const sentence = blockerSentence(
      'fr',
      blocker({
        kind: 'suppressionsAdded',
        suppressions: [
          { path: 'a.rs', line: 'l1', marker: 'm1' },
          { path: 'b.rs', line: 'l2', marker: 'm2' },
          { path: 'c.rs', line: 'l3', marker: 'm3' },
          { path: 'd.rs', line: 'l4', marker: 'm4' },
          { path: 'e.rs', line: 'l5', marker: 'm5' },
        ],
      }),
    );

    expect(sentence).toBe(
      t('fr', 'suppressionsAdded', {
        list: 'a.rs : « l1 » (m1) ; b.rs : « l2 » (m2) ; c.rs : « l3 » (m3)',
        more: ' (+2)'
      }),
    );
  });

  it('renders tests weakened', () => {
    const sentence = blockerSentence(
      'fr',
      blocker({
        kind: 'testsWeakened',
        testsRemoved: 2,
        assertionsRemoved: 5,
        paths: ['a.test.ts', 'b.test.ts'],
      }),
    );

    expect(sentence).toBe(t('fr', 'testsWeakened', { tests: 2, assertions: 5, paths: 'a.test.ts, b.test.ts' }));
  });

  it('renders branch changed to a detached head', () => {
    const sentence = blockerSentence('fr', blocker({ kind: 'branchChanged', from: 'abc', to: null }));

    expect(sentence).toBe(t('fr', 'branchChanged', { from: 'abc', to: t('fr', 'detachedHead') }));
  });

  it('renders local changes', () => {
    const sentence = blockerSentence('fr', blocker({ kind: 'localChanges', paths: ['a.rs', 'b.rs'] }));

    expect(sentence).toBe(t('fr', 'localChanges', { paths: 'a.rs, b.rs' }));
  });
});

describe('acknowledged kinds', () => {
  it('drops never acknowledged and unticked kinds', () => {
    const previewFixture = preview({
      blockers: [
        blocker({ kind: 'noCommits' }),
        blocker({ kind: 'noCriteria' }),
        blocker({ kind: 'protectedTouched', paths: ['p'] }),
      ],
    });

    expect(acknowledgedKinds(previewFixture, { noCommits: true, noCriteria: true })).toEqual(['noCriteria']);
  });
});

describe('gate state', () => {
  it('disables integration and finishing when preview is null', () => {
    const state = gateState({
      preview: null,
      acknowledged: {},
      result: null,
      message: 'feat: x',
      busy: false,
    });

    expect(state.integrateDisabled).toBe(true);
    expect(state.canFinish).toBe(false);
  });

  it('disables integration when busy is true', () => {
    const state = gateState({
      preview: preview({
        commits: [
          { sha: 'c1', subject: 'feat' },
        ],
      }),
      acknowledged: {},
      result: null,
      message: 'feat: x',
      busy: true,
    });

    expect(state.integrateDisabled).toBe(true);
    expect(state.canFinish).toBe(false);
  });

  it('enables integration with one commit, no blockers, and a message', () => {
    const state = gateState({
      preview: preview({
        commits: [
          { sha: 'c1', subject: 'feat' },
        ],
      }),
      acknowledged: {},
      result: null,
      message: 'feat: x',
      busy: false,
    });

    expect(state.integrateDisabled).toBe(false);
    expect(state.canFinish).toBe(false);
  });

  it('disables integration when message is blank', () => {
    const state = gateState({
      preview: preview({
        commits: [
          { sha: 'c1', subject: 'feat' },
        ],
      }),
      acknowledged: {},
      result: null,
      message: '   ',
      busy: false,
    });

    expect(state.integrateDisabled).toBe(true);
    expect(state.canFinish).toBe(false);
  });

  it('disables integration when there are no commits', () => {
    const state = gateState({
      preview: preview({
        commits: [],
        idea: null,
      }),
      acknowledged: {},
      result: null,
      message: 'feat: x',
      busy: false,
    });

    expect(state.integrateDisabled).toBe(true);
    expect(state.canFinish).toBe(false);
  });

  it('allows finishing when there are no commits and an idea', () => {
    const state = gateState({
      preview: preview({
        commits: [],
        idea: { projectId: 'p', id: 'i' },
      }),
      acknowledged: {},
      result: null,
      message: 'feat: x',
      busy: false,
    });

    expect(state.integrateDisabled).toBe(true);
    expect(state.canFinish).toBe(true);
  });

  it('does not allow finishing when there are no commits and no idea', () => {
    const state = gateState({
      preview: preview({
        commits: [],
        idea: null,
      }),
      acknowledged: {},
      result: null,
      message: 'feat: x',
      busy: false,
    });

    expect(state.integrateDisabled).toBe(true);
    expect(state.canFinish).toBe(false);
  });

  it('disables integration for a conflict blocker even when acknowledged', () => {
    const state = gateState({
      preview: preview({
        commits: [
          { sha: 'c1', subject: 'feat' },
        ],
        blockers: [
          blocker({ kind: 'conflict', files: ['a.rs'] }),
        ],
      }),
      acknowledged: { conflict: true },
      result: null,
      message: 'feat: x',
      busy: false,
    });

    expect(state.integrateDisabled).toBe(true);
    expect(state.canFinish).toBe(false);
  });

  it('disables integration for a local changes blocker even when acknowledged', () => {
    const state = gateState({
      preview: preview({
        commits: [
          { sha: 'c1', subject: 'feat' },
        ],
        blockers: [
          blocker({ kind: 'localChanges', paths: ['a.rs'] }),
        ],
      }),
      acknowledged: { localChanges: true },
      result: null,
      message: 'feat: x',
      busy: false,
    });

    expect(state.integrateDisabled).toBe(true);
    expect(state.canFinish).toBe(false);
  });

  it('disables integration for an unticked no criteria blocker', () => {
    const state = gateState({
      preview: preview({
        commits: [
          { sha: 'c1', subject: 'feat' },
        ],
        blockers: [
          blocker({ kind: 'noCriteria' }),
        ],
      }),
      acknowledged: {},
      result: null,
      message: 'feat: x',
      busy: false,
    });

    expect(state.integrateDisabled).toBe(true);
    expect(state.canFinish).toBe(false);
  });

  it('enables integration for a ticked no criteria blocker', () => {
    const state = gateState({
      preview: preview({
        commits: [
          { sha: 'c1', subject: 'feat' },
        ],
        blockers: [
          blocker({ kind: 'noCriteria' }),
        ],
      }),
      acknowledged: { noCriteria: true },
      result: null,
      message: 'feat: x',
      busy: false,
    });

    expect(state.integrateDisabled).toBe(false);
    expect(state.canFinish).toBe(false);
  });

  it('enables closing when result is set with no commits and empty message', () => {
    const result: IntegrationResult = {
      head: 'abc',
      commits: ['abc'],
      protectedTests: [],
      protectError: null,
      worktreeRemoved: true,
      removeError: null,
    };
    const state = gateState({
      preview: preview({
        commits: [],
        idea: null,
      }),
      acknowledged: {},
      result,
      message: '',
      busy: false,
    });

    expect(state.integrateDisabled).toBe(false);
    expect(state.canFinish).toBe(false);
  });
});