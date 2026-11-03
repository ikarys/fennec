import { useEffect, useState } from 'react';
import type { IntegrationConflict, IntegrationPreview, IntegrationResult } from '../../lib/api';
import { createSession, getIntegration, integrate, suggestCommitMessage } from '../../lib/api';
import { completeIdea } from '../../lib/ideas';
import { IntegrationReview } from './IntegrationReview';
import {
  acknowledgedKinds,
  blockerSentence,
  gateState,
  narrativePrompt,
  neverAcknowledged,
  type Acknowledged,
  type BlockerKind,
} from '../../lib/integration';
import { Button } from '../ui/Button';
import { Card } from '../ui/Card';
import { Modal } from '../ui/Modal';
import { FieldRow, TextArea } from '../ui/Field';
import { SectionLabel } from '../ui/SectionLabel';


export function IntegrationDialog({ sessionId, open, onClose, projectId }: { sessionId: string; open: boolean; onClose: () => void; projectId: string | null }) {
  const [narrating, setNarrating] = useState(false);
  const [preview, setPreview] = useState<IntegrationPreview | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [conflict, setConflict] = useState<IntegrationConflict | null>(null);
  const [posting, setPosting] = useState(false);
  const [result, setResult] = useState<IntegrationResult | null>(null);
  const [acknowledged, setAcknowledged] = useState<Acknowledged>({});
  const [message, setMessage] = useState('');
  const [protectTests, setProtectTests] = useState(false);
  const [removeWorktree, setRemoveWorktree] = useState(true);
  const [suggested, setSuggested] = useState<string | null>(null);

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
    setRemoveWorktree(true);
    setSuggested(null);
    let active = true;
    getIntegration(sessionId)
      .then((data) => {
        if (!active) return;
        setPreview(data);
        setMessage(data.defaultMessage);
        setLoading(false);
        // The model's English subject replaces the default only if the human has not edited it yet.
        suggestCommitMessage(sessionId)
          .then(({ message: proposal }) => {
            if (!active || !proposal) return;
            setSuggested(proposal);
            setMessage((current) => (current === data.defaultMessage ? proposal : current));
          })
          .catch(() => {});
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

  const { integrateDisabled, canFinish } = gateState({ preview, acknowledged, result, message, busy: loading || posting });

  const handleNarrate = async () => {
    if (!preview || !projectId) return;
    setNarrating(true);
    try {
      const created = await createSession(projectId, narrativePrompt(preview), 'narrate');
      window.location.hash = '#/s/' + encodeURIComponent(created.id);
      onClose();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setNarrating(false);
    }
  };

  const handleIntegrate = async () => {
    if (!preview) return;
    setPosting(true);
    setError(null);
    setConflict(null);
    try {
      const response = await integrate(sessionId, acknowledgedKinds(preview, acknowledged), message, protectTests, removeWorktree);
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
            <IntegrationReview preview={preview} projectId={projectId} narrating={narrating} onNarrate={handleNarrate} />

            <FieldRow
              label="Message du commit"
              hint={`${suggested !== null && message === suggested ? 'Proposé par le modèle (indicatif), à relire. ' : ''}Un seul commit, à ton nom, avec la session en provenance (Fennec-Session).`}
            >
              <TextArea rows={3} className="w-full font-mono" value={message} onChange={(event) => setMessage(event.target.value)} />
            </FieldRow>

            {preview.testFiles.length > 0 && (
              <label className="flex items-start gap-1.5 text-xs" title="L'agent ne pourra plus modifier ces fichiers de test : ils sont ajoutés à .agents/protected.">
                <input type="checkbox" checked={protectTests} onChange={(event) => setProtectTests(event.target.checked)} />
                <span>
                  Protéger les tests intégrés ({preview.testFiles.length})
                </span>
              </label>
            )}

            <label className="flex items-start gap-1.5 text-xs" title="Supprime le dossier et la branche de l'agent après l'intégration : la session n'aura plus de copie de travail.">
              <input type="checkbox" checked={removeWorktree} onChange={(event) => setRemoveWorktree(event.target.checked)} />
              <span>Nettoyer l'espace de l'agent</span>
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
