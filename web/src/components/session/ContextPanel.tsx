import type { ThreadView } from '../../lib/fold';
import type { SessionDetail, SessionSkills } from '../../lib/api';
import { useEffect, useMemo, useState } from 'react';
import { cachePercent, formatSpeed, formatTokens } from '../../lib/format';
import { previewKindOfPath, relativeToWorkdir, type PreviewKind } from '../../lib/preview';
import { usePreviewOpener, usePreviewWorkdir } from '../preview/PreviewContext';
import { AgentWorkspaceSection } from './AgentWorkspaceSection';
import { CriteriaSection } from './CriteriaSection';
import { VisionSection } from './VisionSection';

interface ContextPanelProps {
  view: ThreadView;
  currentTokens: number;
  contextPercent: number;
  /** The session's skills, folded server-side; null while the first fetch is in flight. */
  skills: SessionSkills | null;
  /** False on the « Nouvelle session » tab, where there is no log to fold. */
  hasSession: boolean;
  compactDisabled: boolean;
  onCompact: () => void;
  compactError: string | null;
  /** Older pages are not loaded: the totals cover the loaded messages only. */
  partial: boolean;
  /** Where the agent's work lands; null when the session runs in place. */
  worktree: SessionDetail['worktree'];
  /** The session id, for the integration dialog; null on a new-session tab. */
  sessionId: string | null;
}

export function ContextPanel({ view, currentTokens, contextPercent, skills, hasSession, compactDisabled, onCompact, compactError, partial, worktree, sessionId }: ContextPanelProps) {
  const opener = usePreviewOpener();
  const workdir = usePreviewWorkdir();
  const [filesExpanded, setFilesExpanded] = useState(false);

  useEffect(() => {
    // A tab change collapses the file list; the workspace section owns its dialog.
    setFilesExpanded(false);
  }, [sessionId]);

  const modifiedFiles = useMemo(() => {
    const merged = new Map<string, { key: string; added: number; removed: number; kind?: PreviewKind }>();
    for (const file of view.modifiedFiles) {
      // The same file arrives absolute and relative: one entry per project path.
      const key = (workdir ? relativeToWorkdir(file.path, workdir) : undefined) ?? file.path;
      const entry: { key: string; added: number; removed: number; kind?: PreviewKind } = merged.get(key) ?? { key, added: 0, removed: 0 };
      entry.added += file.added;
      entry.removed += file.removed;
      if (!entry.kind) {
        entry.kind = previewKindOfPath(file.path);
      }
      merged.set(key, entry);
    }
    return Array.from(merged.values()).sort((a, b) => a.key.localeCompare(b.key));
  }, [view.modifiedFiles, workdir]);

  const visibleModifiedFiles = filesExpanded ? modifiedFiles : modifiedFiles.slice(0, 6);
  const hiddenModifiedCount = modifiedFiles.length - 6;
  return (
      <aside
        aria-label="Contexte de la session"
        className="hidden min-[1100px]:flex flex-1 flex-col gap-5 border-l border-border bg-panel px-4 py-4.5 max-w-[300px] min-w-[240px]"
      >
        <section className="flex flex-col gap-2">
          <h2 className="text-[11px] font-semibold tracking-[0.06em] text-muted uppercase">Contexte</h2>
          <div className="h-1.5 overflow-hidden rounded-full bg-raised">
            <div className="h-full bg-accent" style={{ width: `${contextPercent}%` }} />
          </div>
          <div className="flex justify-between text-xs text-muted">
            <span>{formatTokens(currentTokens)} tokens</span>
            <span>{contextPercent} %</span>
          </div>
          <button
            type="button"
            onClick={onCompact}
            disabled={compactDisabled}
            className="self-start rounded-lg border border-border bg-raised px-2.5 py-1 text-xs text-text disabled:opacity-60"
          >
            Compacter
          </button>
          {compactError && <div className="text-xs text-err">{compactError}</div>}
        </section>

        <section className="flex flex-col gap-1.5">
          <h2 className="text-[11px] font-semibold tracking-[0.06em] text-muted uppercase">{partial ? 'Tokens des messages chargés' : 'Tokens de la session'}</h2>
          {view.usage.calls === 0 && <div className="text-xs text-muted">Aucun appel mesuré.</div>}
          {view.usage.calls > 0 && (
            <dl className="m-0 grid grid-cols-[1fr_auto] gap-x-3 gap-y-1 text-xs tabular-nums">
              <dt className="text-muted">Entrée</dt>
              <dd className="m-0 text-right">{formatTokens(view.usage.prefillTokens)}</dd>
              <dt className="text-muted">dont cache</dt>
              <dd className="m-0 text-right">
                {formatTokens(view.usage.cachedPromptTokens)} · {cachePercent(view.usage.cachedPromptTokens, view.usage.prefillTokens)} %
              </dd>
              <dt className="text-muted">Sortie</dt>
              <dd className="m-0 text-right">{formatTokens(view.usage.generationTokens)}</dd>
              {view.usage.last && (
                <>
                  <dt className="text-muted">Dernier appel</dt>
                  <dd className="m-0 text-right">{formatSpeed(view.usage.last.generationSpeed)}</dd>
                </>
              )}
              <dt className="text-muted">Appels</dt>
              <dd className="m-0 text-right">{view.usage.calls}</dd>
            </dl>
          )}
        </section>

        <section className="flex flex-col gap-1.5">
          <h2 className="text-[11px] font-semibold tracking-[0.06em] text-muted uppercase">Fichiers modifiés</h2>
          {modifiedFiles.length === 0 && <div className="text-xs text-muted">Aucun fichier modifié.</div>}
          {visibleModifiedFiles.map((file) => {
            const relative = workdir ? file.key : undefined;
            const kind = file.kind;
            return (
              <div key={file.key} className="flex items-center gap-2 py-1.5 font-mono text-xs">
                {/* Cut on the left: the file name is what tells files apart. */}
                <span
                  dir="rtl"
                  title={file.key}
                  className="grow overflow-hidden text-left text-ellipsis whitespace-nowrap"
                >
                  <bdi>{file.key}</bdi>
                </span>
                <span className="text-ok">+{file.added}</span>
                <span className="text-muted">−{file.removed}</span>
                {kind && relative && opener && (
                  <button
                    type="button"
                    onClick={() => opener({ path: relative, kind })}
                    className="shrink-0 text-xs text-muted hover:text-text"
                  >
                    Aperçu
                  </button>
                )}
              </div>
            );
          })}
          {hiddenModifiedCount > 0 && (
            <button
              type="button"
              onClick={() => setFilesExpanded((value) => !value)}
              className="self-start text-xs text-muted hover:text-text"
            >
              {filesExpanded ? 'Réduire' : `+ ${hiddenModifiedCount} autres`}
            </button>
          )}
        </section>

        <CriteriaSection criteria={view.criteria} />

        <VisionSection items={view.items} partial={partial} />

        <section className="flex flex-col gap-1.5">
          <h2 className="text-[11px] font-semibold tracking-[0.06em] text-muted uppercase">Skills</h2>
          <p className="m-0 text-[11px] text-muted">Les instructions spécialisées que le modèle a chargées. Point vert : encore dans son contexte.</p>
          {(!hasSession || (skills !== null && skills.loaded.length === 0)) && (
            <div className="text-xs text-muted">Aucun skill chargé.</div>
          )}
          {hasSession && skills === null && <div className="text-xs text-muted">Chargement…</div>}
          {hasSession && skills !== null && skills.loaded.length > 0 && skills.loaded.map((id) => {
            const active = skills.active.includes(id);
            return (
              <div
                key={id}
                title={active ? id : 'Plus dans le contexte du modèle'}
                className="flex items-center gap-2 py-1.5 font-mono text-xs"
              >
                <span className={active ? undefined : 'text-muted'}>{id}</span>
                {active && <span className="h-1.5 w-1.5 shrink-0 rounded-full bg-ok" />}
              </div>
            );
          })}
        </section>

        {worktree && (
          <AgentWorkspaceSection sessionId={sessionId} worktree={worktree} running={view.running} />
        )}

        <section className="flex flex-col gap-2">
          <h2 className="text-[11px] font-semibold tracking-[0.06em] text-muted uppercase">Outils de ce tour</h2>
          {view.toolCounts.length === 0 && <div className="text-xs text-muted">Aucun outil appelé.</div>}
          {view.toolCounts.map((tool) => (
            <div key={tool.name} className="flex justify-between text-xs">
              <span className="font-mono">{tool.name}</span>
              <span className="text-muted">{tool.count}</span>
            </div>
          ))}
        </section>

        <div className="grow" />
        <p className="m-0 rounded-lg bg-raised px-3 py-2.5 text-xs text-muted">
          Rien n'est intégré sans ta décision.
        </p>
      </aside>
  );
}
