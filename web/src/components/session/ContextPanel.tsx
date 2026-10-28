import type { ThreadView } from '../../lib/fold';
import type { SessionDetail, SessionSkills } from '../../lib/api';
import { useEffect, useState, type KeyboardEvent, type PointerEvent } from 'react';
import { useDisplayPrefs } from '../../lib/display';
import { pageBlocks, panelPages, type PanelBlockId } from '../../lib/panelBlocks';
import { Select } from '../ui/Field';
import { AgentWorkspaceSection } from './AgentWorkspaceSection';
import { CriteriaSection } from './CriteriaSection';
import { DecisionsSection } from './DecisionsSection';
import { ModifiedFilesSection } from './ModifiedFilesSection';
import { SkillsSection } from './SkillsSection';
import { TokensSection } from './TokensSection';
import { ToolCountsSection } from './ToolCountsSection';
import { VisionSection } from './VisionSection';

const WIDTH_KEY = 'fennec.contextPanelWidth';
const PAGE_KEY = 'fennec.contextPanel.page';
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

interface ContextPanelProps {
  view: ThreadView;
  /** The session's skills, folded server-side; null while the first fetch is in flight. */
  skills: SessionSkills | null;
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

export function ContextPanel({ view, skills, hasSession, partial, worktree, sessionId, onWorktreeChanged, integrationVersion }: ContextPanelProps) {
  const [width, setWidth] = useState(loadWidth);
  const [selectedPageName, setSelectedPageName] = useState(loadPageName);
  const pages = panelPages(useDisplayPrefs().panelPages);
  const page = pages.find((candidate) => candidate.name === selectedPageName) ?? pages[0] ?? { name: '', blocks: [] };

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

  function renderBlock(id: PanelBlockId) {
    switch (id) {
      case 'criteria':
        return <CriteriaSection key={id} criteria={view.criteria} />;
      case 'modifiedFiles':
        return <ModifiedFilesSection key={id} modifiedFiles={view.modifiedFiles} sessionId={sessionId} />;
      case 'workspace':
        return worktree ? (
          <AgentWorkspaceSection key={id} sessionId={sessionId} worktree={worktree} running={view.running} onDiscarded={onWorktreeChanged} integrationVersion={integrationVersion} />
        ) : null;
      case 'tokens':
        return <TokensSection key={id} usage={view.usage} partial={partial} />;
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
        className="flex min-h-0 w-full flex-col gap-2 overflow-y-auto border-l border-border bg-bg px-3 py-3 [scrollbar-width:thin]"
      >
        {pages.length > 1 && (
          <Select
            aria-label="Page du panneau"
            value={page.name}
            onChange={(event) => {
              const name = event.target.value;
              setSelectedPageName(name);
              savePageName(name);
            }}
            className="w-full"
          >
            {pages.map((candidate) => (
              <option key={candidate.name} value={candidate.name}>
                {candidate.name}
              </option>
            ))}
          </Select>
        )}
        {pageBlocks(page, page === pages[0]).map(renderBlock)}
      </aside>
    </div>
  );
}
