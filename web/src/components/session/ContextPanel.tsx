import type { ThreadView } from '../../lib/fold';
import type { SessionDetail, SessionSkills } from '../../lib/api';
import { useEffect, useMemo, useState, type KeyboardEvent, type PointerEvent } from 'react';
import { cachePercent, formatSpeed, formatTokens } from '../../lib/format';
import { previewKindOfPath, relativeToWorkdir, type PreviewKind } from '../../lib/preview';
import { usePreviewOpener, usePreviewWorkdir } from '../preview/PreviewContext';
import { AgentWorkspaceSection } from './AgentWorkspaceSection';
import { CriteriaSection } from './CriteriaSection';
import { VisionSection } from './VisionSection';
import { Button } from '../ui/Button';
import { SectionLabel } from '../ui/SectionLabel';

const WIDTH_KEY = 'fennec.contextPanelWidth';
const MIN_WIDTH = 240;
const MAX_WIDTH = 560;
const DEFAULT_WIDTH = 300;

function clampWidth(width: number): number {
  return Math.min(MAX_WIDTH, Math.max(MIN_WIDTH, Math.round(width)));
}

function loadWidth(): number {
  try {
    const stored = Number(localStorage.getItem(WIDTH_KEY));
    return stored ? clampWidth(stored) : DEFAULT_WIDTH;
  } catch {
    return DEFAULT_WIDTH;
  }
}

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
  /** Re-fetch the session detail after the worktree changed (discard). */
  onWorktreeChanged: () => void;
}

export function ContextPanel({ view, currentTokens, contextPercent, skills, hasSession, compactDisabled, onCompact, compactError, partial, worktree, sessionId, onWorktreeChanged }: ContextPanelProps) {
  const opener = usePreviewOpener();
  const workdir = usePreviewWorkdir();
  const [filesExpanded, setFilesExpanded] = useState(false);
  const [width, setWidth] = useState(loadWidth);

  useEffect(() => {
    try {
      localStorage.setItem(WIDTH_KEY, String(width));
    } catch {
      // A blocked storage only loses the width on reload.
    }
  }, [width]);

  const startResize = (event: PointerEvent<HTMLDivElement>) => {
    event.preventDefault();
    const startX = event.clientX;
    const startWidth = width;
    // The panel is on the right: dragging left widens it.
    const move = (moveEvent: globalThis.PointerEvent) => setWidth(clampWidth(startWidth + startX - moveEvent.clientX));
    window.addEventListener('pointermove', move);
    window.addEventListener('pointerup', () => window.removeEventListener('pointermove', move), { once: true });
  };

  const resizeWithKeys = (event: KeyboardEvent<HTMLDivElement>) => {
    const step = event.key === 'ArrowLeft' ? 16 : event.key === 'ArrowRight' ? -16 : 0;
    if (step !== 0) {
      event.preventDefault();
      setWidth((current) => clampWidth(current + step));
    }
  };

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
    <div className="relative hidden min-h-0 shrink-0 min-[1100px]:flex" style={{ width }}>
      <div
        role="separator"
        aria-orientation="vertical"
        aria-label="Largeur du panneau"
        aria-valuemin={MIN_WIDTH}
        aria-valuemax={MAX_WIDTH}
        aria-valuenow={width}
        tabIndex={0}
        title="Glisser pour élargir · double-clic : largeur par défaut"
        onPointerDown={startResize}
        onDoubleClick={() => setWidth(DEFAULT_WIDTH)}
        onKeyDown={resizeWithKeys}
        className="absolute inset-y-0 -left-1 z-10 w-2 cursor-col-resize touch-none hover:bg-accent/40 focus-visible:bg-accent/40 focus-visible:outline-none"
      />
      <aside
        aria-label="Contexte de la session"
        className="flex min-h-0 w-full flex-col gap-5 overflow-y-auto border-l border-border bg-panel px-4 py-4.5 [scrollbar-width:thin]"
      >
        <section className="flex flex-col gap-2">
          <SectionLabel>Contexte</SectionLabel>
          <div className="h-1.5 overflow-hidden rounded-full bg-raised">
            <div className="h-full bg-accent" style={{ width: `${contextPercent}%` }} />
          </div>
          <div className="flex justify-between text-xs text-muted tabular-nums">
            <span>{formatTokens(currentTokens)} tokens</span>
            <span>{contextPercent} %</span>
          </div>
          <Button
            variant="secondary"
            size="sm"
            onClick={onCompact}
            disabled={compactDisabled}
            className="self-start"
          >
            Compacter
          </Button>
          {compactError && <div className="text-xs text-err">{compactError}</div>}
        </section>

        <section className="flex flex-col gap-1.5">
          <SectionLabel>{partial ? 'Tokens des messages chargés' : 'Tokens de la session'}</SectionLabel>
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
          <SectionLabel>Fichiers modifiés</SectionLabel>
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
                <span className="text-ok tabular-nums">+{file.added}</span>
                <span className="text-muted tabular-nums">−{file.removed}</span>
                {kind && relative && opener && (
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={() => opener({ path: relative, kind })}
                  >
                    Aperçu
                  </Button>
                )}
              </div>
            );
          })}
          {hiddenModifiedCount > 0 && (
            <Button
              variant="ghost"
              size="sm"
              onClick={() => setFilesExpanded((value) => !value)}
              className="self-start"
            >
              {filesExpanded ? 'Réduire' : `+ ${hiddenModifiedCount} autres`}
            </Button>
          )}
        </section>

        <CriteriaSection criteria={view.criteria} />

        <VisionSection items={view.items} partial={partial} />

        <section className="flex flex-col gap-1.5">
          <SectionLabel>Skills</SectionLabel>
          <p className="m-0 text-[0.6875rem] text-muted">Les instructions spécialisées que le modèle a chargées. Point vert : encore dans son contexte.</p>
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
          <AgentWorkspaceSection sessionId={sessionId} worktree={worktree} running={view.running} onDiscarded={onWorktreeChanged} />
        )}

        <section className="flex flex-col gap-2">
          <SectionLabel>Outils de ce tour</SectionLabel>
          {view.toolCounts.length === 0 && <div className="text-xs text-muted">Aucun outil appelé.</div>}
          {view.toolCounts.map((tool) => (
            <div key={tool.name} className="flex justify-between text-xs">
              <span className="font-mono">{tool.name}</span>
              <span className="text-muted tabular-nums">{tool.count}</span>
            </div>
          ))}
        </section>

        <div className="grow" />
        <p className="m-0 rounded-lg bg-raised px-3 py-2.5 text-xs text-muted">
          Rien n'est intégré sans ta décision.
        </p>
      </aside>
    </div>
  );
}
