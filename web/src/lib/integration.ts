// The integration dialog's words and its gate as the UI applies it: what a blocker says, what the
// narrate agent is asked, when the Intégrer button is enabled. Pure, so it is tested without a browser.
import type { Blocker, IntegrationPreview, IntegrationResult } from './api';

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

export function blockerSentence(blocker: Blocker): string {
  switch (blocker.kind) {
    case 'noCommits':
      return 'Aucun commit à intégrer.';
    case 'conflict':
      return `Conflit avec ta branche : ${blocker.files.join(', ')}.`;
    case 'criteriaNotPassed':
      return `Critères non passés : ${blocker.criteria.join(', ')}.`;
    case 'noCriteria':
      return 'Aucun critère : rien n\'a été vérifié.';
    case 'protectedTouched':
      return `Fichiers protégés modifiés : ${blocker.paths.join(', ')}.`;
    case 'sensitiveTouched':
      return `Zones sensibles modifiées, ta revue est obligatoire : ${blocker.paths.join(', ')}.`;
    case 'architectureViolated': {
      const shown = blocker.violations.slice(0, 3).map((v) => `${v.path} : « ${v.line} » (interdit : ${v.forbidden})`);
      const more = blocker.violations.length - shown.length;
      return `Règles d'architecture violées : ${shown.join(' ; ')}${more > 0 ? ` (+${more})` : ''}.`;
    }
    case 'suppressionsAdded': {
      const shown = blocker.suppressions.slice(0, 3).map((s) => `${s.path} : « ${s.line} » (${s.marker})`);
      const more = blocker.suppressions.length - shown.length;
      return `Vérifications désactivées dans les lignes ajoutées : ${shown.join(' ; ')}${more > 0 ? ` (+${more})` : ''}.`;
    }
    case 'testsWeakened':
      return `Tests affaiblis : ${blocker.testsRemoved} test(s) et ${blocker.assertionsRemoved} assertion(s) en moins (${blocker.paths.join(', ')}).`;
    case 'branchChanged':
      return `Le run est parti de ${blocker.from}, tu es maintenant sur ${blocker.to ?? 'un HEAD détaché'} : les commits iraient ailleurs.`;
    case 'localChanges':
      return `Changements non commités seraient écrasés : ${blocker.paths.join(', ')} — committe-les ou stash-les avant d'intégrer.`;
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
