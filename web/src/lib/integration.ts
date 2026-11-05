// The integration dialog's words and its gate as the UI applies it: what a blocker says, what the
// narrate and understand agents are asked, when the Integrate button is enabled. Pure, so it is tested without a browser.
import type { BadgeTone } from '../components/ui/Badge';
import type { Blocker, IntegrationPreview, IntegrationResult, ScanStatus } from './api';
import { t, type Lang } from '../i18n/messages';

export type BlockerKind = Blocker['kind'];

/** What the human has accepted, blocker by blocker. */
export type Acknowledged = Partial<Record<BlockerKind, boolean>>;

// These blockers cannot be fixed by an acceptance: the merge has nothing to
// do, it cannot succeed, or it would overwrite the human's uncommitted work.
export const neverAcknowledged = (kind: string) => kind === 'noCommits' || kind === 'conflict' || kind === 'localChanges' || kind === 'securityScanRunning';

/** The narrate agent's prompt: the branch, its base and the files in the server's review order. */
export function narrativePrompt(preview: IntegrationPreview): string {
  const lines = preview.files.map((file, index) => {
    const signals = file.signals.length ? ` — signals: ${file.signals.join(', ')}` : '';
    return `${index + 1}. ${file.path} (+${file.added} −${file.removed})${signals}`;
  });

  return `Narrate the change of branch \`${preview.branch}\` since base \`${preview.base}\`, file by file in this review order:\n${lines.join('\n')}\nUse \`git diff ${preview.base} ${preview.branch} -- <path>\` for each file.`;
}

/** The understand agent's prompt: the branch, its base, the files in the server's review order, then questions. */
export function understandingPrompt(preview: IntegrationPreview): string {
  const lines = preview.files.map((file, index) => {
    const signals = file.signals.length ? ` — signals: ${file.signals.join(', ')}` : '';
    return `${index + 1}. ${file.path} (+${file.added} −${file.removed})${signals}`;
  });

  return `Hold a comprehension dialogue with me on the change of branch \`${preview.branch}\` since base \`${preview.base}\`. The files, in review order:\n${lines.join('\n')}\nUse \`git diff ${preview.base} ${preview.branch} -- <path>\` for each file, then ask me your questions.`;
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
    case 'understandingMissing':
      return t(lang, 'understandingMissing');
    case 'securityScanRunning':
      return t(lang, 'securityScanRunning');
    case 'securityFindings': {
      const shown = blocker.findings
        .slice(0, 3)
        .map((f) => `${f.path}${f.line != null ? `:${f.line}` : ''} (${f.scanner} ${f.rule}: ${f.detail})`);
      const more = blocker.findings.length - shown.length;
      return t(lang, 'securityFindings', { list: shown.join(' ; '), more: more > 0 ? ` (+${more})` : '' });
    }
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
      (blocker) => blocker.kind !== 'noCommits' && (neverAcknowledged(blocker.kind) || !acknowledged[blocker.kind]),
    ) ?? false;
  const integrateDisabled = busy || (result === null && (!hasCommits || hasUncheckedBlocker || message.trim() === ''));
  const canFinish = preview !== null && result === null && !hasCommits && preview.idea !== null;
  return { integrateDisabled, canFinish };
}

/** One badge per gate scanner: its state in words and the tone that carries it. */
export function scanBadge(lang: Lang, scan: ScanStatus): { text: string; tone: BadgeTone; running: boolean } {
  const name = scan.scanner;

  switch (scan.state) {
    case 'ran':
      return {
        text: t(lang, 'scanRan', { name, count: scan.findings }),
        tone: scan.findings === 0 ? 'ok' : 'err',
        running: false,
      };
    case 'notInstalled':
      return {
        text: t(lang, 'scanNotInstalled', { name }),
        tone: 'neutral',
        running: false,
      };
    case 'notApplicable':
      return {
        text: t(lang, 'scanNotApplicable', { name }),
        tone: 'neutral',
        running: false,
      };
    case 'failed':
      return {
        text: t(lang, 'scanFailed', { name, reason: scan.reason }),
        tone: 'attention',
        running: false,
      };
    case 'running':
      return {
        text: t(lang, 'scanRunning', { name, seconds: scan.seconds }),
        tone: 'info',
        running: true,
      };
  }
}
