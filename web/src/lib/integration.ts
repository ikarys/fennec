// The integration dialog's words and its gate as the UI applies it: what a blocker says, what the
// narrate agent is asked, when the Integrate button is enabled. Pure, so it is tested without a browser.
import type { Blocker, IntegrationPreview, IntegrationResult } from './api';
import { t, type Lang } from '../i18n/messages';

export type BlockerKind = Blocker['kind'];

/** What the human has accepted, blocker by blocker. */
export type Acknowledged = Partial<Record<BlockerKind, boolean>>;

// These blockers cannot be fixed by an acceptance: the merge has nothing to
// do, it cannot succeed, or it would overwrite the human's uncommitted work.
export const neverAcknowledged = (kind: string) => kind === 'noCommits' || kind === 'conflict' || kind === 'localChanges';

/** The narrate agent's prompt: the branch, its base and the files in the server's review order. */
export function narrativePrompt(preview: IntegrationPreview): string {
  const lines = preview.files.map((file, index) => {
    const signals = file.signals.length ? ` — signals: ${file.signals.join(', ')}` : '';
    return `${index + 1}. ${file.path} (+${file.added} −${file.removed})${signals}`;
  });

  return `Narrate the change of branch \`${preview.branch}\` since base \`${preview.base}\`, file by file in this review order:\n${lines.join('\n')}\nUse \`git diff ${preview.base} ${preview.branch} -- <path>\` for each file.`;
}

export function blockerSentence(lang: Lang, blocker: Blocker): string {
  switch (blocker.kind) {
    case 'noCommits':
      return t(lang, 'noCommits');
    case 'conflict':
      return t(lang, 'conflict', { files: blocker.files.join(', ') });
    case 'criteriaNotPassed':
      return t(lang, 'criteriaNotPassed', { criteria: blocker.criteria.join(', ') });
    case 'noCriteria':
      return t(lang, 'noCriteria');
    case 'protectedTouched':
      return t(lang, 'protectedTouched', { paths: blocker.paths.join(', ') });
    case 'sensitiveTouched':
      return t(lang, 'sensitiveTouched', { paths: blocker.paths.join(', ') });
    case 'architectureViolated': {
      const shown = blocker.violations
        .slice(0, 3)
        .map((v) => `${v.path} : « ${v.line} » (${t(lang, 'architectureForbidden', { what: v.forbidden })})`);
      const more = blocker.violations.length - shown.length;
      return t(lang, 'architectureViolated', { list: shown.join(' ; '), more: more > 0 ? ` (+${more})` : '' });
    }
    case 'suppressionsAdded': {
      const shown = blocker.suppressions.slice(0, 3).map((s) => `${s.path} : « ${s.line} » (${s.marker})`);
      const more = blocker.suppressions.length - shown.length;
      return t(lang, 'suppressionsAdded', { list: shown.join(' ; '), more: more > 0 ? ` (+${more})` : '' });
    }
    case 'testsWeakened':
      return t(lang, 'testsWeakened', {
        tests: blocker.testsRemoved,
        assertions: blocker.assertionsRemoved,
        paths: blocker.paths.join(', ')
      });
    case 'branchChanged':
      return t(lang, 'branchChanged', {
        from: blocker.from,
        to: blocker.to ?? t(lang, 'detachedHead')
      });
    case 'localChanges':
      return t(lang, 'localChanges', { paths: blocker.paths.join(', ') });
  }
}

/** The blocker kinds the human accepted, as the server expects them. */
export function acknowledgedKinds(preview: IntegrationPreview, acknowledged: Acknowledged): BlockerKind[] {
  return preview.blockers
    .filter((blocker) => !neverAcknowledged(blocker.kind) && acknowledged[blocker.kind])
    .map((blocker) => blocker.kind);
}

interface GateInput {
  preview: IntegrationPreview | null;
  acknowledged: Acknowledged;
  result: IntegrationResult | null;
  message: string;
  busy: boolean;
}

/** What the dialog's primary button may do: integrate, or finish an idea whose work is already in the branch. */
export function gateState({ preview, acknowledged, result, message, busy }: GateInput) {
  const hasCommits = (preview?.commits.length ?? 0) > 0;
  const hasUncheckedBlocker =
    preview?.blockers.some(
      (blocker) => blocker.kind === 'conflict' || blocker.kind === 'localChanges' || (!neverAcknowledged(blocker.kind) && !acknowledged[blocker.kind]),
    ) ?? false;
  const integrateDisabled = busy || (result === null && (!hasCommits || hasUncheckedBlocker || message.trim() === ''));
  const canFinish = preview !== null && result === null && !hasCommits && preview.idea !== null;
  return { integrateDisabled, canFinish };
}
