import type { ThreadView } from '../../lib/fold';
import type { SeenImage, ServedThread, SessionDetail, SessionSkills, SessionUsage } from '../../lib/api';
import { useEffect, useState, type KeyboardEvent, type PointerEvent, type ReactNode } from 'react';
import { useDisplayPrefs } from '../../lib/display';
import { pageBlocks, panelBlock, panelPages, type PanelBlockId } from '../../lib/panelBlocks';
import { Button } from '../ui/Button';
import { ChoiceMenu } from '../ui/ChoiceMenu';
import { Segmented } from '../ui/Segmented';
import {
  BranchIcon,
  CheckIcon,
  ChevronDownIcon,
  ChevronsLeftIcon,
  ChevronsRightIcon,
  CloseIcon,
  DecisionIcon,
  EyeIcon,
  FileIcon,
} from '../Icons';
import { AgentWorkspaceSection } from './AgentWorkspaceSection';
import { CriteriaSection } from './CriteriaSection';
import { DecisionsSection } from './DecisionsSection';
import { ModifiedFilesSection } from './ModifiedFilesSection';
import { RequestSection } from './RequestSection';
import { ServersSection } from './ServersSection';
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

const RAIL_BLOCKS: readonly { id: PanelBlockId; icon: ReactNode }[] = [
  { id: 'criteria', icon: <CheckIcon className="h-4 w-4" /> },
  { id: 'modifiedFiles', icon: <FileIcon className="h-4 w-4" /> },
  { id: 'workspace', icon: <BranchIcon className="h-4 w-4" /> },
  { id: 'decisions', icon: <DecisionIcon className="h-4 w-4" /> },
  { id: 'vision', icon: <EyeIcon className="h-4 w-4" /> },
];

function railCount(id: PanelBlockId, view: ThreadView, served: ServedThread, vision: SeenImage[]): number | null {
  switch (id) {
    case 'criteria':
      return served.criteria.length;
    case 'modifiedFiles':
      return view.modifiedFiles.length;
    case 'decisions':
      return served.decisions.length;
    case 'vision':
      return vision.length;
    default:
      return null;
  }
}

/** The rail's count badge is tinted only when it signals a state worth seeing. */
function railCountClass(id: PanelBlockId, served: ServedThread): string {
  if (id === 'criteria' && served.criteria.some((criterion) => criterion.status.type === 'failed')) {
    return 'text-err';
  }
  return 'text-muted';
}

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
  /** Where the agent's work lands; null when the session runs in place. */
  worktree: SessionDetail['worktree'];
  /** The session id, for the integration dialog; null on a new-session tab. */
  sessionId: string | null;
  /** Re-fetch the session detail after the worktree changed (discard). */
  onWorktreeChanged: () => void;
  /** Bumped when the review dialog closes: the workspace block re-reads the preview. */
  integrationVersion: number;
  /** The images the model received, computed server-side. */
  vision: SeenImage[];
  /** The criteria and decisions, folded server-side. */
  served: ServedThread;
}

export function ContextPanel({ view, skills, usage, request, hasSession, worktree, sessionId, onWorktreeChanged, integrationVersion, vision, served }: ContextPanelProps) {
  const [width, setWidth] = useState(loadWidth);
  const [selectedPageName, setSelectedPageName] = useState(loadPageName);
  const [collapsed, setCollapsed] = useState(loadCollapsed);
  const [openBlock, setOpenBlock] = useState<PanelBlockId | null>(null);
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

  useEffect(() => {
    if (!openBlock) return;
    const onKeyDown = (event: globalThis.KeyboardEvent) => {
      if (event.key === 'Escape') setOpenBlock(null);
    };
    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, [openBlock]);

  useEffect(() => {
    setOpenBlock(null);
  }, [sessionId]);

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

  function selectPage(name: string) {
    setSelectedPageName(name);
    savePageName(name);
  }

  /**
   * The panel's page, themed like the composer's agent pick: a segmented
   * control up to three pages, a menu beyond (docs/ui.md: no native select).
   */
  function renderPageSwitch() {
    if (pages.length > 3) {
      return (
        <ChoiceMenu
          label="Page du panneau"
          groups={[{ choices: pages.map((candidate) => ({ value: candidate.name, label: candidate.name })) }]}
          value={page.name}
          onChange={selectPage}
          direction="down"
          className="w-full"
          trigger={
            // The trigger fills the panel width so a long page name truncates instead of overflowing.
            <span className="flex w-full items-center justify-between gap-1.5">
              <span className="min-w-0 truncate" title={page.name}>
                {page.name}
              </span>
              <ChevronDownIcon className="h-3.5 w-3.5 shrink-0" />
            </span>
          }
        />
      );
    }
    return (
      <Segmented
        label="Page du panneau"
        value={page.name}
        onChange={selectPage}
        segments={pages.map((candidate) => ({ value: candidate.name, label: candidate.name, title: candidate.name }))}
      />
    );
  }

  function renderBlock(id: PanelBlockId) {
    switch (id) {
      case 'request':
        return <RequestSection key={id} request={request} />;
      case 'criteria':
        return <CriteriaSection key={id} criteria={served.criteria} />;
      case 'modifiedFiles':
        return <ModifiedFilesSection key={id} modifiedFiles={view.modifiedFiles} sessionId={sessionId} />;
      case 'workspace':
        return sessionId ? (
          <AgentWorkspaceSection key={id} sessionId={sessionId} worktree={worktree ?? null} running={view.running} onDiscarded={onWorktreeChanged} integrationVersion={integrationVersion} />
        ) : null;
      case 'servers':
        return <ServersSection key={id} sessionId={sessionId} />;
      case 'tokens':
        return <TokensSection key={id} usage={usage} />;
      case 'skills':
        return <SkillsSection key={id} skills={skills} hasSession={hasSession} />;
      case 'decisions':
        return <DecisionsSection key={id} decisions={served.decisions} />;
      case 'tools':
        return <ToolCountsSection key={id} toolCounts={view.toolCounts} />;
      case 'vision':
        return <VisionSection key={id} images={vision} />;
      default: {
        const exhaustive: never = id;
        return exhaustive;
      }
    }
  }

  function renderRail(className: string) {
    const openCount = openBlock ? railCount(openBlock, view, served, vision) : null;
    return (
      <aside
        aria-label="Contexte de la session"
        className={`${openBlock ? 'relative ' : ''}w-10 shrink-0 flex-col items-center gap-1 border-l border-border bg-bg py-1 ${className}`}
      >
        {openBlock ? (
          <div
            className="absolute inset-y-0 right-full z-20 flex w-[300px] flex-col border border-border bg-bg shadow-xl"
            role="dialog"
            aria-label={panelBlock(openBlock).title}
          >
            <div className="flex items-center gap-1 px-2 py-2">
              <span className="min-w-0 truncate text-xs font-medium text-text">{panelBlock(openBlock).title}</span>
              {openCount !== null ? <span className="font-mono text-xs text-muted tabular-nums">{openCount}</span> : null}
              <Button
                type="button"
                variant="ghost"
                size="sm"
                iconOnly
                aria-label="Fermer"
                onClick={() => setOpenBlock(null)}
                className="ml-auto"
              >
                <CloseIcon className="h-3.5 w-3.5" />
              </Button>
            </div>
            <div className="min-h-0 grow overflow-y-auto [scrollbar-width:thin]">
              {renderBlock(openBlock)}
            </div>
          </div>
        ) : null}
        <div className="hidden min-[1100px]:flex">
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
        {RAIL_BLOCKS.filter((block) => block.id !== 'workspace' || sessionId).map((block) => {
          const title = panelBlock(block.id).title;
          const count = railCount(block.id, view, served, vision);
          return (
            <Button
              key={block.id}
              type="button"
              variant={openBlock === block.id ? 'secondary' : 'ghost'}
              size="sm"
              iconOnly
              aria-label={title}
              title={title}
              aria-pressed={openBlock === block.id}
              onClick={() => setOpenBlock((current) => (current === block.id ? null : block.id))}
              className="relative"
            >
              {block.icon}
              {count ? (
                <span className={`absolute -top-1 -right-1 rounded-full border border-border bg-raised px-1 font-mono text-[9px] leading-3 tabular-nums ${railCountClass(block.id, served)}`}>
                  {count}
                </span>
              ) : null}
            </Button>
          );
        })}
      </aside>
    );
  }

  if (collapsed) {
    return renderRail('flex');
  }

  return (
    <>
      {renderRail('flex min-[1100px]:hidden')}
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
        {pages.length > 1 && (
          <div className="px-2 pb-2">{renderPageSwitch()}</div>
        )}
        <div className="min-h-0 grow overflow-y-auto [scrollbar-width:thin]">
          {hasSession ? pageBlocks(page, page === pages[0]).map(renderBlock) : <p className="m-0 px-3 py-2 text-xs text-muted">Le contexte apparaît avec le premier message.</p>}
        </div>
      </aside>
      </div>
    </>
  );
}
