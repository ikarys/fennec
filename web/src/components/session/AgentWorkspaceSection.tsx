import { useCallback, useEffect, useRef, useState } from 'react';
import type { IntegrationPreview, SessionDetail } from '../../lib/api';
import { getIntegration } from '../../lib/api';
import { BranchIcon, CheckIcon, CopyIcon, EditIcon, FolderIcon, SpinnerIcon } from '../Icons';
import { IntegrationDialog } from './IntegrationDialog';

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

export function AgentWorkspaceSection({ sessionId, worktree, running }: { sessionId: string | null; worktree: NonNullable<SessionDetail['worktree']>; running: boolean }) {
  const [preview, setPreview] = useState<IntegrationPreview | null>(null);
  const [loading, setLoading] = useState(false);
  const [dialogOpen, setDialogOpen] = useState(false);
  const [copied, setCopied] = useState(false);
  const previousRunning = useRef(running);
  const fetchId = useRef(0);
  const copiedTimer = useRef<number | null>(null);

  const fetchPreview = useCallback(async () => {
    if (!sessionId) return;
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
  }, [sessionId]);

  useEffect(() => {
    void fetchPreview();
  }, [fetchPreview]);

  useEffect(() => {
    // The preview is only meaningful once the agent's turn has finished.
    if (previousRunning.current && !running) {
      void fetchPreview();
    }
    previousRunning.current = running;
  }, [running, fetchPreview]);

  useEffect(() => {
    return () => {
      if (copiedTimer.current !== null) window.clearTimeout(copiedTimer.current);
    };
  }, []);

  const handleCopyPath = () => {
    void navigator.clipboard.writeText(worktree.path);
    setCopied(true);
    if (copiedTimer.current !== null) window.clearTimeout(copiedTimer.current);
    copiedTimer.current = window.setTimeout(() => setCopied(false), 1500);
  };

  const humanBranch = preview?.humanBranch ?? 'ta branche';
  const hasCommits = (preview?.commits.length ?? 0) > 0;
  const integrateDisabled = sessionId === null || running || !hasCommits;

  return (
    <section className="flex flex-col gap-2">
      <h2 className="text-[11px] font-semibold tracking-[0.06em] text-muted uppercase">Espace de l'agent</h2>
      <p className="m-0 text-xs text-muted">
        L'agent travaille dans sa propre copie du dépôt. Ton dossier et ta branche ne changent pas tant que tu n'intègres pas.
      </p>

      <div className="flex flex-col gap-1.5">
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
              className="rounded p-1 text-muted hover:text-text"
            >
              <EditIcon className="h-3.5 w-3.5" />
            </a>
            <button
              type="button"
              onClick={handleCopyPath}
              title="Copier le chemin"
              aria-label="Copier le chemin"
              className="rounded p-1 text-muted hover:text-text"
            >
              {copied ? <CheckIcon className="h-3.5 w-3.5 text-ok" /> : <CopyIcon className="h-3.5 w-3.5" />}
            </button>
          </div>
        </div>

        <div className="flex items-center gap-2 text-xs">
          <BranchIcon className="h-4 w-4 shrink-0 text-muted" />
          <span title={worktree.branch} className="grow overflow-hidden font-mono text-muted text-ellipsis whitespace-nowrap">
            {shortAgentBranch(worktree.branch)}
          </span>
          <span aria-hidden className="shrink-0 text-muted">→</span>
          <span title={preview?.humanBranch ?? undefined} className="grow overflow-hidden font-mono text-text text-ellipsis whitespace-nowrap">
            {humanBranch}
          </span>
        </div>

        {(loading || preview !== null) && (
          <div className="flex flex-col gap-1.5">
            {loading && preview === null ? (
              <div className="flex items-center gap-2 text-xs text-muted">
                <SpinnerIcon className="h-3.5 w-3.5" />
                Lecture des changements…
              </div>
            ) : (
              preview && (
                <>
                  <div className="text-xs text-muted">
                    {hasCommits ? `${preview.commits.length} commits · ${preview.files.length} fichiers` : "Rien à intégrer pour l'instant."}
                  </div>
                  {preview.files.length > 0 && (
                    <div className="max-h-[150px] overflow-y-auto">
                      {preview.files.map((file) => (
                        <div key={file.path} className="flex items-center gap-2 py-1 font-mono text-xs">
                          {/* Cut on the left: the file name is what tells files apart. */}
                          <span
                            dir="rtl"
                            title={file.path}
                            className="grow overflow-hidden text-left text-ellipsis whitespace-nowrap"
                          >
                            <bdi>{file.path}</bdi>
                          </span>
                          {file.protected && (
                            <span className="shrink-0 rounded border border-err bg-err/10 px-1 py-0.5 text-[10px] text-err">
                              protégé
                            </span>
                          )}
                        </div>
                      ))}
                    </div>
                  )}
                </>
              )
            )}
          </div>
        )}

        <button
          type="button"
          onClick={() => setDialogOpen(true)}
          disabled={integrateDisabled}
          title={running ? 'Attends la fin du tour' : undefined}
          className="w-full rounded-lg bg-accent px-2.5 py-1.5 text-xs font-medium text-on-accent disabled:opacity-60"
        >
          Intégrer dans {humanBranch}
        </button>

        <details className="text-xs">
          <summary className="cursor-pointer select-none text-muted hover:text-text">Le faire à la main</summary>
          <div className="mt-1.5">
            <button
              type="button"
              title="Copier la commande"
              onClick={() => void navigator.clipboard.writeText(`git cherry-pick ${worktree.base}..${worktree.branch}`)}
              className="rounded-lg border border-border bg-raised px-2 py-1.5 text-left font-mono text-xs break-all text-text"
            >
              <code>git cherry-pick {worktree.base}..{worktree.branch}</code>
            </button>
          </div>
        </details>
      </div>

      {sessionId && dialogOpen && (
        <IntegrationDialog
          sessionId={sessionId}
          open={dialogOpen}
          onClose={() => {
            setDialogOpen(false);
            // The dialog may have integrated commits; the preview is stale.
            void fetchPreview();
          }}
        />
      )}
    </section>
  );
}
