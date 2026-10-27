import { useEffect, useRef, useState } from 'react';
import type { Blocker, IntegrationPreview } from '../../lib/api';
import { getIntegration, integrate } from '../../lib/api';
import { CriterionProvenanceBadge } from './CriteriaSection';

type BlockerKind = Blocker['kind'];

function blockerSentence(blocker: Blocker): string {
  switch (blocker.kind) {
    case 'noCommits':
      return 'Aucun commit à intégrer.';
    case 'criteriaNotPassed':
      return `Critères non passés : ${blocker.criteria.join(', ')}.`;
    case 'noCriteria':
      return 'Aucun critère : rien n\'a été vérifié.';
    case 'protectedTouched':
      return `Fichiers protégés modifiés : ${blocker.paths.join(', ')}.`;
  }
}

export function IntegrationDialog({ sessionId, open, onClose }: { sessionId: string; open: boolean; onClose: () => void }) {
  const dialogRef = useRef<HTMLDialogElement>(null);
  const [preview, setPreview] = useState<IntegrationPreview | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [posting, setPosting] = useState(false);
  const [result, setResult] = useState<{ head: string; commits: string[] } | null>(null);
  const [acknowledged, setAcknowledged] = useState<Partial<Record<BlockerKind, boolean>>>({});

  useEffect(() => {
    if (!open) {
      dialogRef.current?.close();
      return;
    }
    // A fresh open re-reads the worktree: the previous result and accepted
    // blockers belong to the last preview, not to this one.
    setPreview(null);
    setLoading(true);
    setError(null);
    setResult(null);
    setAcknowledged({});
    let active = true;
    getIntegration(sessionId)
      .then((data) => {
        if (!active) return;
        setPreview(data);
        setLoading(false);
      })
      .catch((fetchError: unknown) => {
        if (!active) return;
        setError(fetchError instanceof Error ? fetchError.message : 'Erreur');
        setLoading(false);
      });
    dialogRef.current?.showModal();
    return () => {
      active = false;
    };
  }, [open, sessionId]);

  const closeDialog = () => {
    dialogRef.current?.close();
    onClose();
  };

  const hasCommits = (preview?.commits.length ?? 0) > 0;
  const hasUncheckedBlocker =
    preview?.blockers.some((blocker) => blocker.kind !== 'noCommits' && !acknowledged[blocker.kind]) ?? false;
  const integrateDisabled = loading || posting || (result === null && (!hasCommits || hasUncheckedBlocker));

  const handleIntegrate = async () => {
    if (!preview) return;
    setPosting(true);
    setError(null);
    const acknowledgedKinds = preview.blockers
      .filter((blocker) => blocker.kind !== 'noCommits' && acknowledged[blocker.kind])
      .map((blocker) => blocker.kind);
    try {
      const response = await integrate(sessionId, acknowledgedKinds);
      setResult(response);
    } catch (integrateError) {
      // A 409 keeps the dialog open: the message says what to do.
      setError(integrateError instanceof Error ? integrateError.message : 'Erreur');
    } finally {
      setPosting(false);
    }
  };

  return (
    <dialog
      ref={dialogRef}
      aria-label="Intégrer"
      onCancel={() => onClose()}
      onClick={(event) => {
        if (event.target === dialogRef.current) {
          closeDialog();
        }
      }}
      className="m-auto w-[min(92vw,720px)] rounded-xl border border-border bg-panel p-4 text-text backdrop:bg-black/60"
    >
      <div className="flex flex-col gap-3">
        <div className="flex flex-col gap-1">
          <h2 className="m-0 text-sm font-semibold">
            {preview ? `Intégrer dans ${preview.humanBranch ?? 'ta branche'}` : 'Intégrer'}
          </h2>
          {preview && (
            <p className="m-0 text-xs text-muted">
              Les commits de {preview.branch} seront rejoués par cherry-pick dans ton dépôt. Rien n'est intégré sans ton clic.
            </p>
          )}
        </div>

        {loading && <div className="text-xs text-muted">Chargement…</div>}
        {error && <div className="text-xs text-err">{error}</div>}

        {result && (
          <div className="text-xs text-ok">
            Intégré : {result.commits.length} commits, HEAD {result.head.slice(0, 8)}.
          </div>
        )}

        {preview && !loading && !result && (
          <div className="flex flex-col gap-3">
            <section className="flex flex-col gap-1.5">
              <h3 className="m-0 text-[0.6875rem] font-semibold tracking-[0.06em] text-muted uppercase">
                Commits{preview.commits.length > 0 && <span className="ml-1 tabular-nums">{preview.commits.length}</span>}
              </h3>
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
              <h3 className="m-0 text-[0.6875rem] font-semibold tracking-[0.06em] text-muted uppercase">
                Fichiers{preview.files.length > 0 && <span className="ml-1 tabular-nums">{preview.files.length}</span>}
              </h3>
              {preview.files.length === 0 ? (
                <div className="text-xs text-muted">Aucun fichier.</div>
              ) : (
                <div className="flex flex-col">
                  {preview.files.map((file) => (
                    <div key={file.path} className="flex items-center gap-2 py-1 font-mono text-xs">
                      <span className="grow overflow-hidden text-left text-ellipsis whitespace-nowrap" title={file.path}>
                        {file.path}
                      </span>
                      {file.protected && (
                        <span className="shrink-0 rounded border border-err bg-err/10 px-1 py-0.5 text-[0.625rem] text-err">
                          protégé
                        </span>
                      )}
                    </div>
                  ))}
                </div>
              )}
            </section>

            <section className="flex flex-col gap-1.5">
              <h3 className="m-0 text-[0.6875rem] font-semibold tracking-[0.06em] text-muted uppercase">Critères</h3>
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

            {preview.blockers.length > 0 && (
              <section className="flex flex-col gap-1.5">
                <h3 className="m-0 text-[0.6875rem] font-semibold tracking-[0.06em] text-muted uppercase">À accepter</h3>
                <div className="flex flex-col gap-1.5">
                  {preview.blockers.map((blocker) => (
                    <div key={blocker.kind} className="flex items-start justify-between gap-2 text-xs">
                      <span className="grow break-words text-text">{blockerSentence(blocker)}</span>
                      {blocker.kind !== 'noCommits' && (
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

        <div className="flex justify-end gap-2">
          <button
            type="button"
            onClick={closeDialog}
            className="rounded-lg border border-border bg-raised px-2.5 py-1 text-xs text-text"
          >
            Annuler
          </button>
          <button
            type="button"
            onClick={() => {
              if (result) {
                closeDialog();
              } else {
                void handleIntegrate();
              }
            }}
            disabled={integrateDisabled}
            className="rounded-lg bg-accent px-2.5 py-1 text-xs font-medium text-white disabled:opacity-60"
          >
            {result ? 'Fermer' : 'Intégrer'}
          </button>
        </div>
      </div>
    </dialog>
  );
}
