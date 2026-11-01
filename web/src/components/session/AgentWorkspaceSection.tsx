import { useCallback, useEffect, useRef, useState } from 'react';
import type { Blocker, IntegrationPreview, RepoState, SessionDetail } from '../../lib/api';
import { discardWorktree, getIntegration, getRepoState, openTerminal, resolveConflict } from '../../lib/api';
import { BranchIcon, CheckIcon, CopyIcon, EditIcon, FolderIcon, SpinnerIcon, TerminalIcon } from '../Icons';
import { useIntegrationInvite } from './IntegrationInvite';
import { Button, buttonClass } from '../ui/Button';
import { Badge } from '../ui/Badge';
import { SectionLabel } from '../ui/SectionLabel';
import { panelBlock } from '../../lib/panelBlocks';
import { Panel } from '../ui/Panel';
import { ConfirmDialog } from '../ui/ConfirmDialog';

function shortFolderName(path: string): string {
  const name = path.split(/[\\/]/).filter(Boolean).pop() ?? path;
  return name.length > 12 ? `${name.slice(0, 12)}…` : name;
}

function shortAgentBranch(branch: string): string {
  const prefix = 'fennec/';
  if (!branch.startsWith(prefix)) {
    return branch.length > 12 ? `${branch.slice(0, 12)}…` : branch;
  }
  const sessionPart = branch.slice(prefix.length);
  return sessionPart.length > 8 ? `${prefix}${sessionPart.slice(0, 8)}…` : branch;
}

export function AgentWorkspaceSection({ sessionId, worktree, running, onDiscarded, integrationVersion }: { sessionId: string | null; worktree: NonNullable<SessionDetail['worktree']> | null; running: boolean; onDiscarded: () => void; integrationVersion: number }) {
  const [preview, setPreview] = useState<IntegrationPreview | null>(null);
  const [repo, setRepo] = useState<RepoState | null>(null);
  const [loading, setLoading] = useState(false);
  const [copied, setCopied] = useState(false);
  const invite = useIntegrationInvite();
  const [discarding, setDiscarding] = useState(false);
  const [discardError, setDiscardError] = useState<string | null>(null);
  const [pendingDiscard, setPendingDiscard] = useState(false);
  const [resolvingConflict, setResolvingConflict] = useState(false);
  const [conflictError, setConflictError] = useState<string | null>(null);
  const previousRunning = useRef(running);
  const fetchId = useRef(0);
  const repoFetchId = useRef(0);
  const copiedTimer = useRef<number | null>(null);
  const hasWorktree = worktree !== null;

  const fetchPreview = useCallback(async () => {
    if (!sessionId || !hasWorktree) {
      fetchId.current += 1;
      setPreview(null);
      setLoading(false);
      return;
    }
    // A stale response must not overwrite a newer session's preview.
    const id = ++fetchId.current;
    setLoading(true);
    try {
      const data = await getIntegration(sessionId);
      if (id === fetchId.current) setPreview(data);
    } catch {
      // A 409 means the worktree does not exist yet: nothing to integrate.
      if (id === fetchId.current) setPreview(null);
    } finally {
      if (id === fetchId.current) setLoading(false);
    }
  }, [sessionId, hasWorktree]);

  const fetchRepo = useCallback(async () => {
    if (!sessionId) {
      repoFetchId.current += 1;
      setRepo(null);
      return;
    }
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
    // The preview is only meaningful once the agent's turn has finished.
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

  useEffect(() => {
    return () => {
      if (copiedTimer.current !== null) window.clearTimeout(copiedTimer.current);
    };
  }, []);

  const handleCopyPath = () => {
    if (worktree === null) return;
    void navigator.clipboard.writeText(worktree.path);
    setCopied(true);
    if (copiedTimer.current !== null) window.clearTimeout(copiedTimer.current);
    copiedTimer.current = window.setTimeout(() => setCopied(false), 1500);
  };

  const [terminalError, setTerminalError] = useState<string | null>(null);
  const handleOpenTerminal = () => {
    if (!sessionId) return;
    setTerminalError(null);
    openTerminal(sessionId).catch((failure) => setTerminalError(failure instanceof Error ? failure.message : 'Erreur'));
  };
  const terminalButton = (
    <Button
      variant="ghost"
      size="sm"
      iconOnly
      onClick={handleOpenTerminal}
      disabled={!sessionId}
      title="Ouvrir un terminal ici"
      aria-label="Ouvrir un terminal ici"
    >
      <TerminalIcon className="h-3.5 w-3.5" />
    </Button>
  );

  const openDiscard = () => {
    if (!sessionId || worktree === null) return;
    setPendingDiscard(true);
  };

  const confirmDiscard = async () => {
    if (!sessionId) return;
    setPendingDiscard(false);
    setDiscarding(true);
    setDiscardError(null);
    try {
      await discardWorktree(sessionId);
      onDiscarded();
    } catch (discardFailure) {
      setDiscardError(discardFailure instanceof Error ? discardFailure.message : 'Erreur');
    } finally {
      setDiscarding(false);
    }
  };

  const humanBranch = repo?.branch ?? preview?.humanBranch ?? 'ta branche';
  const hasCommits = (preview?.commits.length ?? 0) > 0;
  // The file list lives in the Fichiers modifiés block; here only what blocks the review.
  const protectedCount = preview?.files.filter((file) => file.protected).length ?? 0;
  const integrateDisabled = sessionId === null || running || invite === null;
  const conflictFiles =
    preview?.blockers.find(
      (blocker): blocker is Extract<Blocker, { kind: 'conflict' }> => blocker.kind === 'conflict',
    )?.files ?? [];

  const handleResolveConflict = async () => {
    if (!sessionId) return;
    setResolvingConflict(true);
    setConflictError(null);
    try {
      await resolveConflict(sessionId);
      void fetchPreview();
    } catch (failure) {
      setConflictError(failure instanceof Error ? failure.message : 'Erreur');
    } finally {
      setResolvingConflict(false);
    }
  };

  return (
    <Panel id="workspace" title={panelBlock('workspace').title}>
      <div className="flex flex-col gap-3">
        <div className="flex flex-col gap-1.5">
          <SectionLabel>L'agent</SectionLabel>
          {worktree !== null ? (
            <>
              <div className="flex items-center gap-2">
                <FolderIcon className="h-4 w-4 shrink-0 text-muted" />
                <span title={worktree.path} className="grow overflow-hidden text-left font-mono text-xs text-ellipsis whitespace-nowrap">
                  {shortFolderName(worktree.path)}
                </span>
                <div className="flex shrink-0 items-center gap-1">
                  <a
                    href={worktree.editorUrl}
                    title="Ouvrir dans VS Code"
                    aria-label="Ouvrir dans VS Code"
                    className={buttonClass({ variant: 'ghost', size: 'sm', iconOnly: true })}
                  >
                    <EditIcon className="h-3.5 w-3.5" />
                  </a>
                  {terminalButton}
                  <Button
                    variant="ghost"
                    size="sm"
                    iconOnly
                    onClick={handleCopyPath}
                    title="Copier le chemin"
                    aria-label="Copier le chemin"
                  >
                    {copied ? <CheckIcon className="h-3.5 w-3.5 text-ok" /> : <CopyIcon className="h-3.5 w-3.5" />}
                  </Button>
                </div>
              </div>

              <div className="flex items-center gap-2 text-xs">
                <BranchIcon className="h-4 w-4 shrink-0 text-muted" />
                <span title={worktree.branch} className="grow overflow-hidden font-mono text-muted text-ellipsis whitespace-nowrap">
                  {shortAgentBranch(worktree.branch)}
                </span>
                <div className="flex shrink-0 items-center gap-1 text-muted">
                  {loading && preview === null ? (
                    <>
                      <SpinnerIcon className="h-3.5 w-3.5" />
                      Lecture…
                    </>
                  ) : (
                    <>
                      <span>
                        {hasCommits ? `${preview?.commits.length ?? 0} commits · ${preview?.files.length ?? 0} fichiers` : "Rien à intégrer pour l'instant."}
                      </span>
                      {protectedCount > 0 && (
                        <Badge tone="err">
                          {protectedCount} protégé{protectedCount > 1 ? 's' : ''}
                        </Badge>
                      )}
                    </>
                  )}
                </div>
              </div>
            </>
          ) : (
            <div className="flex items-center gap-2 text-xs text-muted">
              <span className="grow">Travaille directement dans ton dossier.</span>
              {terminalButton}
            </div>
          )}
          {terminalError !== null && <div className="text-xs text-err">{terminalError}</div>}
        </div>

        {repo !== null && (
          <div className="flex flex-col gap-1.5">
            <SectionLabel>Ton dépôt</SectionLabel>
            <div className="flex items-center gap-2">
              <FolderIcon className="h-4 w-4 shrink-0 text-muted" />
              <span title={repo.dir} className="grow overflow-hidden font-mono text-xs text-ellipsis whitespace-nowrap">
                {repo.dir.split(/[\\/]/).filter(Boolean).pop() ?? repo.dir}
              </span>
            </div>
            <div className="flex items-center gap-2 text-xs">
              <BranchIcon className="h-4 w-4 shrink-0 text-muted" />
              <span title={repo.branch ?? undefined} className="grow overflow-hidden font-mono text-text text-ellipsis whitespace-nowrap">
                {repo.branch ?? 'HEAD détachée'}
              </span>
              {repo.dirty > 0 && (
                <span title="Modifications non commitées dans ton dossier" className="inline-flex shrink-0">
                  <Badge tone="accent">
                    {repo.dirty} non commité{repo.dirty > 1 ? 's' : ''}
                  </Badge>
                </span>
              )}
            </div>
            {repo.branchChanged && (
              <div className="text-xs text-err">
                Le travail est parti de {repo.originBranch} : tu as changé de branche depuis.
              </div>
            )}
          </div>
        )}

        {worktree !== null && (
          <div className="flex flex-col gap-1.5">
            {conflictFiles.length > 0 && (
              <div className="rounded-lg border border-err/40 bg-err/10 px-2.5 py-2 text-xs flex flex-col gap-1.5">
                <div>
                  Conflit avec {humanBranch} sur {conflictFiles.length} fichier(s) : l'intégration est bloquée.
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
                  disabled={running || resolvingConflict || sessionId === null}
                  title="L'agent met sa branche à jour et résout les conflits dans cette session"
                >
                  Résoudre avec l'agent
                </Button>
                {conflictError && <div className="text-xs text-err">{conflictError}</div>}
              </div>
            )}
            {/* A branch with no commit (a read-only agent, or already integrated) has nothing to integrate. */}
            {hasCommits && (
              <Button
                variant="primary"
                size="sm"
                onClick={() => invite?.open()}
                disabled={integrateDisabled}
                title={running ? 'Attends la fin du tour' : undefined}
                className="w-full"
              >
                Intégrer dans {humanBranch}
              </Button>
            )}

            {/* Once integrated the branch holds nothing past its base: nothing to abandon. */}
            {hasCommits && (
              <Button
                variant="danger"
                size="sm"
                onClick={openDiscard}
                disabled={sessionId === null || running || discarding}
                className="w-full"
              >
                Abandonner ce travail
              </Button>
            )}
            {discardError && <div className="text-xs text-err">{discardError}</div>}

            <details className="text-xs">
              <summary className="cursor-pointer select-none text-muted hover:text-text">Le faire à la main</summary>
              <div className="mt-1.5">
                <button
                  type="button"
                  title="Copier la commande"
                  onClick={() => void navigator.clipboard.writeText(`git cherry-pick ${worktree.base}..${worktree.branch}`)}
                  className="rounded-lg border border-border bg-code px-2 py-1.5 text-left font-mono text-xs break-all text-text hover:border-border-strong"
                >
                  <code>git cherry-pick {worktree.base}..{worktree.branch}</code>
                </button>
              </div>
            </details>
          </div>
        )}
      </div>
      <ConfirmDialog
        open={pendingDiscard}
        title="Abandonner ce travail ?"
        body={`Le dossier et la branche de l'agent seront supprimés, ses ${preview?.commits.length ?? 0} commits perdus. Ton dossier et ta branche ne sont pas touchés.`}
        confirmLabel="Abandonner"
        onConfirm={() => void confirmDiscard()}
        onCancel={() => setPendingDiscard(false)}
      />
    </Panel>
  );
}
