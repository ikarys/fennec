import { useEffect, useState } from 'react';
import type { IntegrationConflict, IntegrationPreview, IntegrationResult } from '../../lib/api';
import { createSession, getIntegration, integrate, suggestCommitMessage } from '../../lib/api';
import { completeIdea } from '../../lib/ideas';
import { IntegrationReview } from './IntegrationReview';
import { ServerList } from './ServerList';
import {
  acknowledgedKinds,
  blockerSentence,
  gateState,
  narrativePrompt,
  scanBadge,
  neverAcknowledged,
  type Acknowledged,
  type BlockerKind,
} from '../../lib/integration';
import { AlertIcon, BranchIcon, SpinnerIcon } from '../Icons';
import { Badge } from '../ui/Badge';
import { Button } from '../ui/Button';
import { Card } from '../ui/Card';
import { Modal } from '../ui/Modal';
import { FieldRow, TextArea } from '../ui/Field';
import { SectionLabel } from '../ui/SectionLabel';
import { useLang, useT } from '../../i18n';


export function IntegrationDialog({ sessionId, open, onClose, projectId }: { sessionId: string; open: boolean; onClose: () => void; projectId: string | null }) {
  const t = useT();
  const lang = useLang();
  const [tryOpen, setTryOpen] = useState(false);
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
        setError(fetchError instanceof Error ? fetchError.message : t('error'));
        setLoading(false);
      });
    return () => {
      active = false;
    };
  }, [open, sessionId]);

  // The scanners run in the background: re-read the gate each second until they end.
  useEffect(() => {
    if (!open || !preview) return;
    if (!preview.securityScans.some((scan) => scan.state === 'running')) return;

    let active = true;
    const timer = window.setTimeout(() => {
      getIntegration(sessionId)
        .then((data) => {
          if (active) setPreview(data);
        })
        .catch(() => {});
    }, 1000);

    return () => {
      active = false;
      window.clearTimeout(timer);
    };
  }, [open, preview, sessionId]);

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
        setError(integrateError instanceof Error ? integrateError.message : t('error'));
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
      setError(finishError instanceof Error ? finishError.message : t('error'));
    } finally {
      setPosting(false);
    }
  };

  return (
    <Modal open={open} onClose={onClose} busy={posting} label={t('integrate')} width="w-[min(92vw,720px)]">
      <Card className="overflow-hidden">
        {preview ? (
          <div className="border-b border-border bg-accent-soft px-4 py-3">
            <div className="flex items-center gap-2.5">
              <span className="flex shrink-0 items-center gap-1.5 rounded border border-border bg-raised px-2 py-0.5 font-mono text-xs">
                <BranchIcon className="h-3 w-3 text-accent" />
                {preview.humanBranch ?? t('yourBranch')}
              </span>
              <h2 className="m-0 min-w-0 truncate text-sm font-semibold">
                {t('integrateInto', { branch: preview.humanBranch ?? t('yourBranch') })}
              </h2>
              <span className="ml-auto shrink-0 rounded border border-border px-1.5 py-0.5 font-mono text-xs text-muted">
                {preview.commits.length} {t(preview.commits.length === 1 ? 'commit' : 'commits')}
              </span>
            </div>
            <div className="mt-1 pl-[33px] text-xs text-muted">
              {t('filesCount', { count: preview.files.length })} ·{' '}
              <span className="text-ok">+{preview.files.reduce((sum, file) => sum + file.added, 0)}</span>{' '}
              <span className="text-err">−{preview.files.reduce((sum, file) => sum + file.removed, 0)}</span> ·{' '}
              {t('becomesCommit')}
            </div>
          </div>
        ) : (
          <div className="border-b border-border px-4 py-3">
            <h2 className="m-0 text-sm font-semibold">{t('integrate')}</h2>
          </div>
        )}
        <div className="flex flex-col gap-3 p-4">

          {loading && <div className="text-xs text-muted">{t('loading')}</div>}
          {conflict ? (
            <div className="rounded-lg border border-err/40 bg-err/10 p-3 text-xs flex flex-col gap-2">
              <div className="font-semibold text-err">{t('conflictNothingIntegrated')}</div>
              <div>{t('conflictNothingIntegratedBody')}</div>
              <div className="flex flex-col gap-1">
                {conflict.files.map((file) => (
                  <div key={file} className="font-mono">{file}</div>
                ))}
              </div>
              <div className="flex flex-col gap-1">
                <div>{t('resolveByHand')}</div>
                <button
                  type="button"
                  title={t('copyCommand')}
                  onClick={() => void navigator.clipboard.writeText(conflict.command)}
                  className="rounded-lg border border-border bg-code px-2 py-1.5 text-left font-mono text-xs break-all text-text hover:border-border-strong"
                >
                  <code>{conflict.command}</code>
                </button>
                <div className="text-muted">{t('orDiscardWork')}</div>
              </div>
            </div>
          ) : error ? (
            <div className="text-xs text-err">{error}</div>
          ) : null}

          {result && (
            <div className="text-xs text-ok">
              {t('integratedResult', { commits: result.commits.length, head: result.head.slice(0, 8) })}
              {result.protectedTests.length > 0 && ` ${t('testsProtectedResult', { count: result.protectedTests.length })}`}
              {result.protectError && <span className="text-err"> {t('testsNotProtected', { error: result.protectError })}</span>}
              {result.worktreeRemoved && ` ${t('worktreeRemovedResult')}`}
              {result.removeError && <span className="text-err"> {t('worktreeNotRemoved', { error: result.removeError })}</span>}
            </div>
          )}

          {preview && !loading && !result && (
            <div className="flex flex-col gap-3">
              <IntegrationReview preview={preview} projectId={projectId} narrating={narrating} onNarrate={handleNarrate} />

              <FieldRow
                label={t('commitMessageLabel')}
                hint={
                  <>
                    {suggested !== null && message === suggested && (
                      <Badge tone="accent" title={t('commitMessageIndicativeTitle')}>
                        {t('indicative')}
                      </Badge>
                    )}
                    <span>
                      {suggested !== null && message === suggested ? ` ${t('commitMessageSuggested')} ` : ''}
                      {t('commitMessageSingle')}
                    </span>
                  </>
                }
              >
                <TextArea rows={3} className="w-full font-mono" value={message} onChange={(event) => setMessage(event.target.value)} />
              </FieldRow>

              {preview.testFiles.length > 0 && (
                <label
                  className="flex cursor-pointer items-center gap-2 rounded-md px-1.5 py-1 text-xs hover:bg-accent-soft"
                  title={t('protectTestsTitle')}
                >
                  <input type="checkbox" className="accent-accent" checked={protectTests} onChange={(event) => setProtectTests(event.target.checked)} />
                  {t('protectTests', { count: preview.testFiles.length })}
                </label>
              )}

              <label
                className="flex cursor-pointer items-center gap-2 rounded-md px-1.5 py-1 text-xs hover:bg-accent-soft"
                title={t('removeWorktreeTitle')}
              >
                <input type="checkbox" className="accent-accent" checked={removeWorktree} onChange={(event) => setRemoveWorktree(event.target.checked)} />
                {t('removeWorktree')}
              </label>

              {preview.securityScans.length > 0 && (
                <div className="flex flex-wrap items-center gap-1.5 px-1.5">
                  <span className="text-xs text-muted">{t('securityScansLabel')}</span>
                  {preview.securityScans.map((scan) => {
                    const b = scanBadge(lang, scan);
                    return (
                      <Badge key={scan.scanner} tone={b.tone} pill>
                        {b.running && <SpinnerIcon className="h-3 w-3" />}
                        {b.text}
                      </Badge>
                    );
                  })}
                </div>
              )}


              {preview.blockers.length > 0 && (
                <section className="flex flex-col gap-1.5 rounded-lg border border-attention/35 bg-attention/10 p-3">
                  <SectionLabel className="flex items-center">
                    <span className="flex items-center gap-1.5 text-attention">
                      <AlertIcon className="h-3.5 w-3.5" />
                      {t('toAccept')}
                    </span>
                  </SectionLabel>
                  <div className="flex flex-col gap-1.5">
                    {preview.blockers.map((blocker) => (
                      <div key={blocker.kind} className="flex items-start justify-between gap-2 text-xs">
                        <span className="grow break-words text-text">{blockerSentence(lang, blocker)}</span>
                        {!neverAcknowledged(blocker.kind) && (
                          <label className="flex shrink-0 cursor-pointer items-center gap-1.5 text-xs">
                            <input
                              type="checkbox"
                              className="accent-accent"
                              checked={Boolean(acknowledged[blocker.kind])}
                              onChange={(event) => {
                                setAcknowledged((current) => ({ ...current, [blocker.kind]: event.target.checked } as Record<BlockerKind, boolean>));
                              }}
                            />
                            {t('acceptBlocker')}
                          </label>
                        )}
                      </div>
                    ))}
                  </div>
                </section>
              )}
            </div>
          )}

          {/* The change runs where it lives, its worktree: tried before it reaches the human's branch. */}
          <section className="flex flex-col gap-1.5">
            <SectionLabel>{t('tryBeforeIntegrating')}</SectionLabel>
            <details onToggle={(event) => setTryOpen(event.currentTarget.open)}>
              <summary className="cursor-pointer text-xs text-muted">{t('tryBeforeIntegratingHint')}</summary>
              {tryOpen && (
                <div className="mt-1.5">
                  <ServerList sessionId={sessionId} />
                </div>
              )}
            </details>
          </section>

          {canFinish && (
            <p className="text-xs text-muted">
              {t('nothingToFinishHint')}
            </p>
          )}
          <div className="flex justify-end gap-2">
            <Button variant="secondary" size="md" onClick={closeDialog}>
              {t('cancel')}
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
              {canFinish ? t('finishTask') : result ? t('close') : t('integrate')}
            </Button>
          </div>
        </div>
      </Card>
    </Modal>
  );
}
