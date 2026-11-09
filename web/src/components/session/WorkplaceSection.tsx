/**
 * The session's landing place: the human's folder or an agent worktree,
 * with the state that blocks the review.
 */
import { useCallback, useEffect, useRef, useState } from 'react';
import type { Blocker, IntegrationPreview, RepoState, SessionDetail } from '../../lib/api';
import { discardWorktree, getIntegration, getRepoState, openFileManager, openTerminal, resolveConflict } from '../../lib/api';
import { BranchIcon, ExternalIcon, FolderIcon, TerminalIcon, TrashIcon } from '../ui/Icons';
import { Button, buttonClass } from '../ui/Button';
import { Badge } from '../ui/Badge';
import { Panel } from '../ui/Panel';
import { ConfirmDialog } from '../ui/ConfirmDialog';
import { useOpenInEditorLabel } from '../../lib/editors';
import { useT } from '../../i18n';

function shortAgentBranch(branch: string): string {
  const prefix = 'fennec/';
  if (!branch.startsWith(prefix)) {
    return branch.length > 12 ? `${branch.slice(0, 12)}…` : branch;
  }
  const sessionPart = branch.slice(prefix.length);
  return sessionPart.length > 8 ? `${prefix}${sessionPart.slice(0, 8)}…` : branch;
}

function lastSegment(path: string): string {
  return path.split(/[\\/]/).filter(Boolean).pop() ?? path;
}

export function WorkplaceSection({
  sessionId,
  worktree,
  editorUrl,
  running,
  onDiscarded,
  integrationVersion,
}: {
  sessionId: string;
  worktree: NonNullable<SessionDetail['worktree']> | null;
  editorUrl: string | null;
  running: boolean;
  onDiscarded: () => void;
  integrationVersion: number;
}) {
  const t = useT();
  const openInEditor = useOpenInEditorLabel();

  const [preview, setPreview] = useState<IntegrationPreview | null>(null);
  const [repo, setRepo] = useState<RepoState | null>(null);
  const [discarding, setDiscarding] = useState(false);
  const [discardError, setDiscardError] = useState<string | null>(null);
  const [pendingDiscard, setPendingDiscard] = useState(false);
  const [resolvingConflict, setResolvingConflict] = useState(false);
  const [conflictError, setConflictError] = useState<string | null>(null);
  const [terminalError, setTerminalError] = useState<string | null>(null);
  const [filesError, setFilesError] = useState<string | null>(null);

  const previousRunning = useRef(running);
  const fetchId = useRef(0);
  const repoFetchId = useRef(0);
  const hasWorktree = worktree !== null;

  const fetchPreview = useCallback(async () => {
    if (!hasWorktree) {
      fetchId.current += 1;
      setPreview(null);
      return;
    }

    // A stale response must not overwrite a newer session's blocker.
    const id = ++fetchId.current;
    try {
      const data = await getIntegration(sessionId);
      if (id === fetchId.current) setPreview(data);
    } catch {
      if (id === fetchId.current) setPreview(null);
    }
  }, [sessionId, hasWorktree]);

  const fetchRepo = useCallback(async () => {
    // A stale response must not overwrite a newer session's repository.
    const id = ++repoFetchId.current;
    try {
      const data = await getRepoState(sessionId);
      if (id === repoFetchId.current) setRepo(data);
    } catch {
      if (id === repoFetchId.current) setRepo(null);
    }
  }, [sessionId]);

  useEffect(() => {
    void fetchPreview();
    void fetchRepo();
  }, [fetchPreview, fetchRepo, integrationVersion]);

  useEffect(() => {
    // The blocker is only meaningful once the agent's turn has finished.
    if (previousRunning.current && !running) {
      void fetchPreview();
      void fetchRepo();
    }
    previousRunning.current = running;
  }, [running, fetchPreview, fetchRepo]);

  useEffect(() => {
    const onFocus = () => void fetchRepo();
    window.addEventListener('focus', onFocus);
    return () => window.removeEventListener('focus', onFocus);
  }, [fetchRepo]);

  const handleOpenTerminal = () => {
    setTerminalError(null);
    openTerminal(sessionId).catch((failure) =>
      setTerminalError(failure instanceof Error ? failure.message : t('error')),
    );
  };

  const handleOpenFiles = () => {
    setFilesError(null);
    openFileManager(sessionId).catch((failure) =>
      setFilesError(failure instanceof Error ? failure.message : t('error')),
    );
  };

  const openDiscard = () => {
    if (worktree === null) return;
    setPendingDiscard(true);
  };

  const confirmDiscard = async () => {
    setPendingDiscard(false);
    setDiscarding(true);
    setDiscardError(null);
    try {
      await discardWorktree(sessionId);
      onDiscarded();
    } catch (discardFailure) {
      setDiscardError(discardFailure instanceof Error ? discardFailure.message : t('error'));
    } finally {
      setDiscarding(false);
    }
  };

  const conflictFiles =
    preview?.blockers.find(
      (blocker): blocker is Extract<Blocker, { kind: 'conflict' }> => blocker.kind === 'conflict',
    )?.files ?? [];

  const conflict = conflictFiles.length > 0;
  const humanBranch = repo?.branch ?? preview?.humanBranch ?? t('yourBranch');

  const handleResolveConflict = async () => {
    setResolvingConflict(true);
    setConflictError(null);
    try {
      await resolveConflict(sessionId);
      void fetchPreview();
    } catch (failure) {
      setConflictError(failure instanceof Error ? failure.message : t('error'));
    } finally {
      setResolvingConflict(false);
    }
  };

  const terminalButton = (
    <Button
      variant="ghost"
      size="sm"
      iconOnly
      onClick={handleOpenTerminal}
      title={t('openTerminalHere')}
      aria-label={t('openTerminalHere')}
    >
      <TerminalIcon className="h-3.5 w-3.5" />
    </Button>
  );

  const editorLink = (href: string) => (
    <a
      href={href}
      title={openInEditor}
      aria-label={openInEditor}
      className={buttonClass({ variant: 'ghost', size: 'sm', iconOnly: true })}
    >
      <ExternalIcon className="h-3.5 w-3.5" />
    </a>
  );

  return (
    <Panel
      id="workplace"
      title={t('panelWorkplace')}
      icon={<BranchIcon className="h-3.5 w-3.5" />}
      iconTone={conflict ? 'err' : 'neutral'}
    >
      <div className="flex flex-col gap-1.5">
        {worktree === null ? (
          <>
            <div className="flex items-center gap-1.5 text-xs">
              {repo !== null && (
                <>
                  <button
                    type="button"
                    onClick={handleOpenFiles}
                    title={t('openInExplorer', { dir: repo.dir })}
                    aria-label={t('openInExplorer', { dir: repo.dir })}
                    className="flex items-center gap-1 rounded-md px-1 font-mono hover:bg-accent-soft"
                  >
                    <FolderIcon className="h-3.5 w-3.5" />
                    {lastSegment(repo.dir)}
                  </button>
                  <span className="text-muted">·</span>
                  <BranchIcon className="h-3.5 w-3.5 shrink-0 text-muted" />
                  <span
                    title={repo.branch ?? t('detachedHeadBranch')}
                    className="min-w-0 truncate font-mono"
                  >
                    {repo.branch ?? t('detachedHeadBranch')}
                  </span>
                  {repo.dirty > 0 && (
                    <Badge tone="accent" appearance="pill" title={t('dirtyTitle')}>
                      {t('dirtyCount', { count: repo.dirty })}
                    </Badge>
                  )}
                </>
              )}
              <div className="ml-auto flex shrink-0 items-center gap-0.5">
                {terminalButton}
                {editorUrl !== null && editorLink(editorUrl)}
              </div>
            </div>
            <p className="m-0 text-xs text-muted">{t('worksInYourFolder')}</p>
          </>
        ) : (
          <>
            <div className="flex items-center gap-1.5 text-xs">
              <BranchIcon className="h-3.5 w-3.5 shrink-0 text-muted" />
              <span title={worktree.branch} className="min-w-0 truncate font-mono">
                {shortAgentBranch(worktree.branch)}
              </span>
              {worktree.commits > 0 && (
                <Badge tone="neutral" appearance="pill">
                  {t('worktreeCommits', { count: worktree.commits })}
                </Badge>
              )}
              <div className="ml-auto flex shrink-0 items-center gap-0.5">
                {terminalButton}
                {worktree.editorUrl !== null && editorLink(worktree.editorUrl)}
                <Button
                  variant="ghost"
                  size="sm"
                  iconOnly
                  onClick={openDiscard}
                  disabled={running || discarding}
                  className="hover:text-err"
                  title={t('discardWork')}
                  aria-label={t('discardWork')}
                >
                  <TrashIcon className="h-3.5 w-3.5" />
                </Button>
              </div>
            </div>

            {repo !== null && (
              <p className="m-0 text-xs text-muted">
                {t('landsOn', {
                  branch: repo.branch ?? t('detachedHeadBranch'),
                  dir: lastSegment(repo.dir),
                })}
              </p>
            )}

            {conflict && (
              <div className="rounded-lg border border-err/40 bg-err-soft px-2.5 py-2 text-xs flex flex-col gap-1.5">
                <div>
                  {t('conflictBlocked', { branch: humanBranch, count: conflictFiles.length })}
                </div>
                <div className="flex flex-col gap-0.5 font-mono">
                  {conflictFiles.map((file) => (
                    <div key={file}>{file}</div>
                  ))}
                </div>
                <Button
                  variant="primary"
                  size="sm"
                  className="w-full"
                  onClick={() => void handleResolveConflict()}
                  disabled={running || resolvingConflict}
                  title={t('resolveConflictTitle')}
                >
                  {t('resolveWithAgent')}
                </Button>
                {conflictError !== null && <div className="text-xs text-err">{conflictError}</div>}
              </div>
            )}
          </>
        )}

        {repo?.branchChanged && (
          <p className="m-0 text-xs text-err">
            {t('branchChangedSince', { branch: repo.originBranch ?? '' })}
          </p>
        )}

        {terminalError !== null && <div className="text-xs text-err">{terminalError}</div>}
        {worktree === null && filesError !== null && (
          <div className="text-xs text-err">{filesError}</div>
        )}
        {worktree !== null && discardError !== null && (
          <div className="text-xs text-err">{discardError}</div>
        )}
      </div>

      <ConfirmDialog
        open={pendingDiscard}
        title={t('discardWorkTitle')}
        body={t('discardWorkBody', { count: worktree?.commits ?? 0 })}
        confirmLabel={t('discard')}
        onConfirm={() => void confirmDiscard()}
        onCancel={() => setPendingDiscard(false)}
      />
    </Panel>
  );
}
