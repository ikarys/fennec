import { DecisionList } from './DecisionList';
import { useEffect, useState } from 'react';
import type { Blocker, IntegrationConflict, IntegrationPreview, ReviewSignal } from '../../lib/api';
import type { IntegrationResult } from '../../lib/api';
import { getIntegration, integrate } from '../../lib/api';
import { completeIdea } from '../../lib/ideas';
import { CriterionProvenanceBadge } from './CriteriaSection';
import { Button } from '../ui/Button';
import { Badge } from '../ui/Badge';
import type { BadgeTone } from '../ui/Badge';
import { Card } from '../ui/Card';
import { Modal } from '../ui/Modal';
import { FieldRow, TextArea } from '../ui/Field';
import { SectionLabel } from '../ui/SectionLabel';

type BlockerKind = Blocker['kind'];

// These blockers cannot be fixed by an acceptance: the merge has nothing to
// do, or it cannot succeed.
const neverAcknowledged = (kind: string) => kind === 'noCommits' || kind === 'conflict';

// `protected` has its own badge, shown before these.
const SIGNAL_LABEL: Record<Exclude<ReviewSignal, 'protected'>, { label: string; tone: BadgeTone; title: string }> = {
  bypass: {
    label: 'contournement',
    tone: 'err',
    title: 'Les lignes ajoutées font taire un lint, un contrôle de types ou un test.'
  },
  architecture: {
    label: 'architecture',
    tone: 'err',
    title: "Le fichier viole une règle d'architecture."
  },
  conflictMarker: {
    label: 'marqueurs de conflit',
    tone: 'err',
    title: 'Des marqueurs de conflit git restent dans les lignes ajoutées.'
  },
  testsWeakened: {
    label: 'tests affaiblis',
    tone: 'err',
    title: 'Des tests ou assertions ont été affaiblis dans ce fichier.'
  },
  hot: {
    label: 'instable',
    tone: 'accent',
    title: 'Modifié 5 fois ou plus ces 90 derniers jours.'
  },
  untested: {
    label: 'sans test',
    tone: 'accent',
    title: 'Aucun test en ligne et aucun fichier de test du même nom sur la branche.'
  },
  large: {
    label: 'gros changement',
    tone: 'neutral',
    title: '200 lignes ou plus modifiées.'
  }
};

function blockerSentence(blocker: Blocker): string {
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

export function IntegrationDialog({ sessionId, open, onClose }: { sessionId: string; open: boolean; onClose: () => void }) {
  const [preview, setPreview] = useState<IntegrationPreview | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [conflict, setConflict] = useState<IntegrationConflict | null>(null);
  const [posting, setPosting] = useState(false);
  const [result, setResult] = useState<IntegrationResult | null>(null);
  const [acknowledged, setAcknowledged] = useState<Partial<Record<BlockerKind, boolean>>>({});
  const [message, setMessage] = useState('');
  const [protectTests, setProtectTests] = useState(false);
  const [removeWorktree, setRemoveWorktree] = useState(false);

  useEffect(() => {
    if (!open) return;
    // A fresh open re-reads the worktree: the previous result and accepted
    // blockers belong to the last preview, not to this one.
    setPreview(null);
    setLoading(true);
    setError(null);
    setConflict(null);
    setResult(null);
    setAcknowledged({});
    setMessage('');
    setProtectTests(false);
    let active = true;
    getIntegration(sessionId)
      .then((data) => {
        if (!active) return;
        setPreview(data);
        setMessage(data.defaultMessage);
        setLoading(false);
      })
      .catch((fetchError: unknown) => {
        if (!active) return;
        setError(fetchError instanceof Error ? fetchError.message : 'Erreur');
        setLoading(false);
      });
    return () => {
      active = false;
    };
  }, [open, sessionId]);

  const closeDialog = () => {
    onClose();
  };

  const hasCommits = (preview?.commits.length ?? 0) > 0;
  const hasUncheckedBlocker =
    preview?.blockers.some(
      (blocker) => blocker.kind === 'conflict' || (!neverAcknowledged(blocker.kind) && !acknowledged[blocker.kind]),
    ) ?? false;
  const integrateDisabled =
    loading || posting || (result === null && (!hasCommits || hasUncheckedBlocker || message.trim() === ''));
  const canFinish = preview !== null && result === null && !hasCommits && preview.idea !== null;

  const handleIntegrate = async () => {
    if (!preview) return;
    setPosting(true);
    setError(null);
    setConflict(null);
    const acknowledgedKinds = preview.blockers
      .filter((blocker) => !neverAcknowledged(blocker.kind) && acknowledged[blocker.kind])
      .map((blocker) => blocker.kind);
    try {
      const response = await integrate(sessionId, acknowledgedKinds, message, protectTests, removeWorktree);
      setResult(response);
    } catch (integrateError) {
      // A 409 keeps the dialog open: the message says what to do.
      const conflictValue = integrateError instanceof Error ? (integrateError as { conflict?: IntegrationConflict }).conflict : undefined;
      if (conflictValue) {
        setConflict(conflictValue);
      } else {
        setError(integrateError instanceof Error ? integrateError.message : 'Erreur');
      }
    } finally {
      setPosting(false);
    }
  };

  const handleFinish = async () => {
    if (!preview || preview.idea === null) return;
    setPosting(true);
    setError(null);
    setConflict(null);
    try {
      await completeIdea(preview.idea.projectId, preview.idea.id);
      closeDialog();
    } catch (finishError) {
      setError(finishError instanceof Error ? finishError.message : 'Erreur');
    } finally {
      setPosting(false);
    }
  };

  return (
    <Modal open={open} onClose={onClose} busy={posting} label="Intégrer" width="w-[min(92vw,720px)]">
      <Card className="flex flex-col gap-3 p-4">
        <div className="flex flex-col gap-1">
          <h2 className="m-0 text-sm font-semibold">
            {preview ? `Intégrer dans ${preview.humanBranch ?? 'ta branche'}` : 'Intégrer'}
          </h2>
          {preview && (
            <p className="m-0 text-xs text-muted">
              Les changements de l'agent deviennent un commit sur ta branche. Rien n'est intégré sans ton clic.
            </p>
          )}
        </div>

        {loading && <div className="text-xs text-muted">Chargement…</div>}
        {conflict ? (
          <div className="rounded-lg border border-err/40 bg-err/10 p-3 text-xs flex flex-col gap-2">
            <div className="font-semibold text-err">Conflit : rien n'a été intégré</div>
            <div>Ta branche a modifié les mêmes lignes que l'agent depuis le début de son travail. Le cherry-pick a été annulé : ton dossier est tel qu'avant.</div>
            <div className="flex flex-col gap-1">
              {conflict.files.map((file) => (
                <div key={file} className="font-mono">{file}</div>
              ))}
            </div>
            <div className="flex flex-col gap-1">
              <div>Pour résoudre à la main :</div>
              <button
                type="button"
                title="Copier la commande"
                onClick={() => void navigator.clipboard.writeText(conflict.command)}
                className="rounded-lg border border-border bg-code px-2 py-1.5 text-left font-mono text-xs break-all text-text hover:border-border-strong"
              >
                <code>{conflict.command}</code>
              </button>
              <div className="text-muted">Ou abandonne ce travail s'il n'est plus utile.</div>
            </div>
          </div>
        ) : error ? (
          <div className="text-xs text-err">{error}</div>
        ) : null}

        {result && (
          <div className="text-xs text-ok">
            Intégré : {result.commits.length} commits, HEAD {result.head.slice(0, 8)}.
            {result.protectedTests.length > 0 && ` ${result.protectedTests.length} tests protégés.`}
            {result.protectError && <span className="text-err"> Tests non protégés : {result.protectError}</span>}
            {result.worktreeRemoved && ' Dossier de l\'agent supprimé.'}
            {result.removeError && <span className="text-err"> Dossier non supprimé : {result.removeError}</span>}
          </div>
        )}

        {preview && !loading && !result && (
          <div className="flex flex-col gap-3">
            <section className="flex flex-col gap-1.5">
              <SectionLabel>
                Commits{preview.commits.length > 0 && <span className="ml-1 tabular-nums">{preview.commits.length}</span>}
              </SectionLabel>
              {preview.commits.length === 0 ? (
                <div className="text-xs text-muted">Aucun commit.</div>
              ) : (
                <div className="flex flex-col">
                  {preview.commits.map((commit) => (
                    <div key={commit.sha} className="flex items-start gap-2 py-1 text-xs">
                      <span className="shrink-0 font-mono text-muted">{commit.sha.slice(0, 8)}</span>
                      <span className="grow break-words text-text">{commit.subject}</span>
                    </div>
                  ))}
                </div>
              )}
            </section>

            <section className="flex flex-col gap-1.5">
              <SectionLabel>
                Fichiers{preview.files.length > 0 && <span className="ml-1 tabular-nums">{preview.files.length}</span>}
                {preview.files.length > 0 && (
                  <span className="ml-2 text-xs text-muted">ordre de relecture · vérifié</span>
                )}
              </SectionLabel>
              {preview.files.length === 0 ? (
                <div className="text-xs text-muted">Aucun fichier.</div>
              ) : (
                <div className="flex flex-col">
                  {preview.files.map((file) => (
                    <div key={file.path} className="flex items-center gap-2 py-1 font-mono text-xs">
                      <span className="grow overflow-hidden text-left text-ellipsis whitespace-nowrap" title={file.path}>
                        {file.path}
                      </span>
                      {file.protected && <Badge tone="err">protégé</Badge>}
                      {file.carried && (
                        <Badge
                          tone="accent"
                          title="Le fichier avait des modifications non commitées sur lesquelles l'agent a travaillé : elles entrent dans ce commit."
                        >
                          ton travail inclus
                        </Badge>
                      )}
                      {file.signals.map((signal) =>
                        signal === 'protected' ? null : (
                          <Badge key={signal} tone={SIGNAL_LABEL[signal].tone} title={SIGNAL_LABEL[signal].title}>
                            {SIGNAL_LABEL[signal].label}
                          </Badge>
                        ),
                      )}
                      <span className="shrink-0 tabular-nums text-ok">+{file.added}</span>
                      <span className="shrink-0 tabular-nums text-err">−{file.removed}</span>
                    </div>
                  ))}
                </div>
              )}
            </section>

            <section className="flex flex-col gap-1.5">
              <SectionLabel>Critères</SectionLabel>
              {preview.criteria.length === 0 ? (
                <div className="text-xs text-muted">Aucun critère.</div>
              ) : (
                <div className="flex flex-col">
                  {preview.criteria.map((criterion) => {
                    const dotClass =
                      criterion.status.type === 'passed' ? 'bg-ok' :
                      criterion.status.type === 'failed' ? 'bg-err' :
                      'bg-muted';
                    const provenance = criterion.status.type === 'pending' ? undefined : criterion.status.provenance;
                    return (
                      <div key={criterion.id} className="flex items-start gap-2 py-1.5 text-xs">
                        <span aria-hidden className={`mt-1 h-2 w-2 shrink-0 rounded-full ${dotClass}`} />
                        <span className="shrink-0 font-mono text-muted">{criterion.id}</span>
                        <span className="grow break-words text-text">{criterion.description}</span>
                        <CriterionProvenanceBadge provenance={provenance} />
                      </div>
                    );
                  })}
                </div>
              )}
            </section>

            <section className="flex flex-col gap-1.5">
              <SectionLabel>Décisions</SectionLabel>
              <DecisionList decisions={preview.decisions} />
            </section>

            <FieldRow label="Message du commit" hint="Un seul commit, à ton nom, avec la session en provenance (Fennec-Session).">
              <TextArea rows={3} className="w-full font-mono" value={message} onChange={(event) => setMessage(event.target.value)} />
            </FieldRow>

            {preview.testFiles.length > 0 && (
              <label className="flex items-start gap-1.5 text-xs">
                <input type="checkbox" checked={protectTests} onChange={(event) => setProtectTests(event.target.checked)} />
                <span>
                  Protéger les tests intégrés ({preview.testFiles.length}) : l'agent ne pourra plus les modifier.
                </span>
              </label>
            )}

            <label className="flex items-start gap-1.5 text-xs">
              <input type="checkbox" checked={removeWorktree} onChange={(event) => setRemoveWorktree(event.target.checked)} />
              <span>Supprimer le dossier et la branche de l'agent après l'intégration (la session n'aura plus de copie de travail).</span>
            </label>

            {preview.blockers.length > 0 && (
              <section className="flex flex-col gap-1.5">
                <SectionLabel>À accepter</SectionLabel>
                <div className="flex flex-col gap-1.5">
                  {preview.blockers.map((blocker) => (
                    <div key={blocker.kind} className="flex items-start justify-between gap-2 text-xs">
                      <span className="grow break-words text-text">{blockerSentence(blocker)}</span>
                      {!neverAcknowledged(blocker.kind) && (
                        <label className="flex shrink-0 items-center gap-1.5 text-xs">
                          <input
                            type="checkbox"
                            checked={Boolean(acknowledged[blocker.kind])}
                            onChange={(event) => {
                              setAcknowledged((current) => ({ ...current, [blocker.kind]: event.target.checked } as Record<BlockerKind, boolean>));
                            }}
                          />
                          Je l'accepte
                        </label>
                      )}
                    </div>
                  ))}
                </div>
              </section>
            )}
          </div>
        )}

        {canFinish && (
          <p className="text-xs text-muted">
            Rien à intégrer : si le travail est déjà dans ta branche (intégré à la main), termine la tâche.
          </p>
        )}
        <div className="flex justify-end gap-2">
          <Button variant="secondary" size="md" onClick={closeDialog}>
            Annuler
          </Button>
          <Button
            variant="primary"
            size="md"
            onClick={() => {
              if (canFinish) {
                void handleFinish();
              } else if (result) {
                closeDialog();
              } else {
                void handleIntegrate();
              }
            }}
            disabled={canFinish ? posting : integrateDisabled}
          >
            {canFinish ? 'Terminer la tâche' : result ? 'Fermer' : 'Intégrer'}
          </Button>
        </div>
      </Card>
    </Modal>
  );
}
