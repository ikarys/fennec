import type { ThreadView } from '../../lib/fold';
import type { SessionDetail, SessionSkills, SessionUsage } from '../../lib/api';
import { useEffect, useState, type KeyboardEvent, type PointerEvent } from 'react';
import { useDisplayPrefs } from '../../lib/display';
import { pageBlocks, panelPages, type PanelBlockId } from '../../lib/panelBlocks';
import { Button } from '../ui/Button';
import { Select } from '../ui/Field';
import { ChevronsLeftIcon, ChevronsRightIcon } from '../Icons';
import { AgentWorkspaceSection } from './AgentWorkspaceSection';
import { CriteriaSection } from './CriteriaSection';
import { DecisionsSection } from './DecisionsSection';
import { ModifiedFilesSection } from './ModifiedFilesSection';
import { RequestSection } from './RequestSection';
import { SkillsSection } from './SkillsSection';
import { TokensSection } from './TokensSection';
import { ToolCountsSection } from './ToolCountsSection';
import { VisionSection } from './VisionSection';

const WIDTH_KEY = 'fennec.contextPanelWidth';
const PAGE_KEY = 'fennec.contextPanel.page';
const COLLAPSED_KEY = 'fennec.contextPanel.collapsed';
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

function loadPageName(): string {
  try {
    return localStorage.getItem(PAGE_KEY) ?? '';
  } catch {
    return '';
  }
}

function savePageName(name: string): void {
  try {
    localStorage.setItem(PAGE_KEY, name);
  } catch {
    // A blocked storage only loses the page on reload.
  }
}

function loadCollapsed(): boolean {
  try {
    return localStorage.getItem(COLLAPSED_KEY) === '1';
  } catch {
    return false;
  }
}

function saveCollapsed(collapsed: boolean): void {
  try {
    localStorage.setItem(COLLAPSED_KEY, collapsed ? '1' : '0');
  } catch {
    // A blocked storage only loses the collapsed state on reload.
  }
}

interface ContextPanelProps {
  view: ThreadView;
  /** The session's skills, folded server-side; null while the first fetch is in flight. */
  skills: SessionSkills | null;
  /** The session's token totals, computed server-side; null while the first fetch is in flight. */
  usage: SessionUsage | null;
  /** The first message the human typed, verbatim; null while unknown or absent. */
  request: string | null;
  /** False on the « Nouvelle session » tab, where there is no log to fold. */
  hasSession: boolean;
  /** Older pages are not loaded: the totals cover the loaded messages only. */
  partial: boolean;
  /** Where the agent's work lands; null when the session runs in place. */
  worktree: SessionDetail['worktree'];
  /** The session id, for the integration dialog; null on a new-session tab. */
  sessionId: string | null;
  /** Re-fetch the session detail after the worktree changed (discard). */
  onWorktreeChanged: () => void;
  /** Bumped when the review dialog closes: the workspace block re-reads the preview. */
  integrationVersion: number;
}

export function ContextPanel({ view, skills, usage, request, hasSession, partial, worktree, sessionId, onWorktreeChanged, integrationVersion }: ContextPanelProps) {
  const [width, setWidth] = useState(loadWidth);
  const [selectedPageName, setSelectedPageName] = useState(loadPageName);
  const [collapsed, setCollapsed] = useState(loadCollapsed);
  const pages = panelPages(useDisplayPrefs().panelPages);
  const page = pages.find((candidate) => candidate.name === selectedPageName) ?? pages[0] ?? { name: '', blocks: [] };

  useEffect(() => {
    try {
      localStorage.setItem(WIDTH_KEY, String(width));
    } catch {
      // A blocked storage only loses the width on reload.
    }
  }, [width]);

  useEffect(() => {
    saveCollapsed(collapsed);
  }, [collapsed]);

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

  function renderBlock(id: PanelBlockId) {
    switch (id) {
      case 'request':
        return <RequestSection key={id} request={request} />;
      case 'criteria':
        return <CriteriaSection key={id} criteria={view.criteria} />;
      case 'modifiedFiles':
        return <ModifiedFilesSection key={id} modifiedFiles={view.modifiedFiles} sessionId={sessionId} />;
      case 'workspace':
        return sessionId ? (
          <AgentWorkspaceSection key={id} sessionId={sessionId} worktree={worktree ?? null} running={view.running} onDiscarded={onWorktreeChanged} integrationVersion={integrationVersion} />
        ) : null;
      case 'tokens':
        return <TokensSection key={id} usage={usage} />;
      case 'skills':
        return <SkillsSection key={id} skills={skills} hasSession={hasSession} />;
      case 'decisions':
        return <DecisionsSection key={id} decisions={view.decisions} />;
      case 'tools':
        return <ToolCountsSection key={id} toolCounts={view.toolCounts} />;
      case 'vision':
        return <VisionSection key={id} items={view.items} partial={partial} />;
      default: {
        const exhaustive: never = id;
        return exhaustive;
      }
    }
  }

  if (collapsed) {
    return (
      <aside
        aria-label="Contexte de la session"
        className="hidden min-h-0 w-10 shrink-0 flex-col border-l border-border bg-bg min-[1100px]:flex"
      >
        <div className="flex items-center justify-center p-1">
          <Button
            type="button"
            variant="ghost"
            size="sm"
            iconOnly
            aria-label="Déplier"
            onClick={() => setCollapsed(false)}
          >
            <ChevronsLeftIcon className="h-3.5 w-3.5" />
          </Button>
        </div>
      </aside>
    );
  }

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
        className="flex min-h-0 w-full flex-col border-l border-border bg-bg"
      >
        <div className="flex items-center gap-1 px-2 py-2">
          <span className="min-w-0 truncate text-xs font-medium text-text" title="Contexte de la session">
            Contexte
          </span>
          <Button
            type="button"
            variant="ghost"
            size="sm"
            iconOnly
            aria-label="Replier"
            onClick={() => setCollapsed(true)}
            className="ml-auto"
          >
            <ChevronsRightIcon className="h-3.5 w-3.5" />
          </Button>
        </div>
        <div className="min-h-0 grow overflow-y-auto [scrollbar-width:thin]">
          {pages.length > 1 && (
            <label className="flex items-center gap-2 px-3 py-2 text-xs text-muted">Vue
              <Select
                aria-label="Page du panneau"
                value={page.name}
                onChange={(event) => {
                  const name = event.target.value;
                  setSelectedPageName(name);
                  savePageName(name);
                }}
                className="min-w-0 grow"
              >
                {pages.map((candidate) => (
                  <option key={candidate.name} value={candidate.name}>
                    {candidate.name}
                  </option>
                ))}
              </Select>
            </label>
          )}
          {pageBlocks(page, page === pages[0]).map(renderBlock)}
        </div>
      </aside>
    </div>
  );
}
