import { useEffect, useRef, useState, type PointerEvent, type KeyboardEvent } from 'react';
import { useT } from '../i18n';
import type { Project } from '../lib/api';
import { SessionPage } from './SessionPage';
import { Palette } from '../components/layout/Palette';
import type { PageId } from '../components/layout/TopBar';
import { Button } from '../components/ui/Button';
import { PlusIcon, CloseIcon } from '../components/ui/Icons';
import { SessionTabs, type TabInfo } from '../components/layout/SessionTabs';
import { SideRailFloat } from '../components/ui/SideRail';

const RATIO_KEY = 'fennec.split.ratio';
const MIN_PCT = 25;
const MAX_PCT = 75;

function clampRatio(pct: number): number {
  return Math.min(MAX_PCT, Math.max(MIN_PCT, Math.round(pct)));
}

function loadRatio(): number {
  try {
    const stored = Number(localStorage.getItem(RATIO_KEY));
    return stored ? clampRatio(stored) : 50;
  } catch {
    return 50;
  }
}

function saveRatio(pct: number): void {
  try {
    localStorage.setItem(RATIO_KEY, String(clampRatio(pct)));
  } catch {
    // A blocked storage only loses the ratio on reload.
  }
}

interface SplitPageProps {
  /** A pane's value: a session id, `new:<projectId>`, or null for an empty pane. */
  left: string | null;
  right: string | null;
  projects: Project[];
  /** Each pane's own open tabs: closing one in a pane leaves the other pane's list alone. */
  paneTabs: { left: TabInfo[]; right: TabInfo[] };
  /** The projects the strips' "+" menu offers, most recently used first. */
  newSessionProjects: { id: string; name: string }[];
  /** A tab picked in a pane's strip: a session opens in that pane, a page leaves the split. */
  onPickTabInPane: (side: 'left' | 'right', id: string) => void;
  onCloseTabInPane: (side: 'left' | 'right', id: string) => void;
  /** Closes a pane: the other one stays, alone. */
  onClosePane: (side: 'left' | 'right') => void;
  onChooseSession: (side: 'left' | 'right', sessionId: string) => void;
  onNewSessionInPane: (side: 'left' | 'right', projectId: string) => void;
  onSessionCreated: (side: 'left' | 'right', newId: string, pseudoId: string | null) => void;
  onRunningChanged: () => void;
  onRenamed: () => void;
  onOpenProjectDialog: () => void;
  onHome: () => void;
  onOpenPage: (page: PageId) => void;
  onOpenProject: (projectId: string) => void;
}

/**
 * Two sessions side by side, like two browser windows in one page. Each pane is
 * a full, independent SessionPage (its own thread, composer and SSE stream); the
 * panes' ids live in the URL, so the split survives a refresh. Each pane keeps
 * its own tab strip and tabs while the split lasts.
 */
export function SplitPage({
  left,
  right,
  projects,
  paneTabs,
  newSessionProjects,
  onPickTabInPane,
  onCloseTabInPane,
  onClosePane,
  onChooseSession,
  onNewSessionInPane,
  onSessionCreated,
  onRunningChanged,
  onRenamed,
  onOpenProjectDialog,
  onHome,
  onOpenPage,
  onOpenProject,
}: SplitPageProps) {
  const t = useT();
  const [leftPct, setLeftPct] = useState(() => loadRatio());
  const [paletteFor, setPaletteFor] = useState<'left' | 'right' | null>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const draggingRef = useRef(false);
  const pctRef = useRef(leftPct);

  useEffect(() => {
    pctRef.current = leftPct;
  }, [leftPct]);

  const updateFromClientX = (clientX: number) => {
    const rect = containerRef.current?.getBoundingClientRect();
    if (!rect || rect.width === 0) return;
    setLeftPct(clampRatio(((clientX - rect.left) / rect.width) * 100));
  };

  const handleDividerDown = (event: PointerEvent<HTMLDivElement>) => {
    draggingRef.current = true;
    event.currentTarget.setPointerCapture(event.pointerId);
    event.preventDefault();
  };
  const handleDividerMove = (event: PointerEvent<HTMLDivElement>) => {
    if (draggingRef.current) updateFromClientX(event.clientX);
  };
  const handleDividerUp = (event: PointerEvent<HTMLDivElement>) => {
    if (!draggingRef.current) return;
    draggingRef.current = false;
    event.currentTarget.releasePointerCapture(event.pointerId);
    saveRatio(pctRef.current);
  };
  const handleDividerKey = (event: KeyboardEvent<HTMLDivElement>) => {
    if (event.key === 'ArrowLeft' || event.key === 'ArrowRight') {
      event.preventDefault();
      const next = clampRatio(leftPct + (event.key === 'ArrowLeft' ? -5 : 5));
      setLeftPct(next);
      saveRatio(next);
    }
  };

  const renderPane = (side: 'left' | 'right', value: string | null) => {
    if (value === null) {
      return (
        <div className="relative flex min-h-0 min-w-0 grow flex-col items-center justify-center gap-3 p-6 text-center">
          <Button
            variant="ghost"
            size="sm"
            iconOnly
            className="absolute right-2 top-2"
            title={t('splitClosePane')}
            aria-label={t('splitClosePane')}
            onClick={() => onClosePane(side)}
          >
            <CloseIcon className="h-3.5 w-3.5" />
          </Button>
          <p className="text-sm text-muted">{t('splitEmptyPane')}</p>
          <Button onClick={() => setPaletteFor(side)}>
            <PlusIcon className="h-4 w-4" />
            {t('splitChooseSession')}
          </Button>
        </div>
      );
    }
    const isNew = value.startsWith('new:');
    return (
      <SessionPage
        sessionId={isNew ? null : value}
        projectId={isNew ? value.slice(4) : null}
        testWorkflowId={null}
        frameIdeaId={null}
        fromSessionId={null}
        pseudoTabId={isNew ? value : null}
        projects={projects}
        onSessionCreated={(newId, pseudoId) => onSessionCreated(side, newId, pseudoId)}
        onRunningChanged={onRunningChanged}
        onRenamed={onRenamed}
        onClose={() => onClosePane(side)}
      />
    );
  };

  const renderColumn = (side: 'left' | 'right', value: string | null) => (
    <div
      className={side === 'left' ? 'flex min-h-0 min-w-0 flex-col' : 'flex min-h-0 min-w-0 grow flex-col'}
      style={side === 'left' ? { width: `${leftPct}%` } : undefined}
    >
      <div className="flex h-11 shrink-0 border-b border-border bg-panel px-2">
        <SessionTabs
          tabs={paneTabs[side]}
          activeTabId={value}
          onPickTab={(id) => onPickTabInPane(side, id)}
          onCloseTab={(id) => onCloseTabInPane(side, id)}
          currentProject={null}
          projects={newSessionProjects}
          onNewSession={(projectId) => onNewSessionInPane(side, projectId)}
        />
      </div>
      <div className="flex min-h-0 min-w-0 grow">
        <SideRailFloat.Provider value={true}>{renderPane(side, value)}</SideRailFloat.Provider>
      </div>
    </div>
  );

  return (
    <div ref={containerRef} className="flex min-h-0 min-w-0 grow">
      {renderColumn('left', left)}

      <div
        role="separator"
        aria-orientation="vertical"
        aria-label={t('splitDividerAria')}
        aria-valuemin={MIN_PCT}
        aria-valuemax={MAX_PCT}
        aria-valuenow={leftPct}
        tabIndex={0}
        title={t('splitDividerTitle')}
        onPointerDown={handleDividerDown}
        onPointerMove={handleDividerMove}
        onPointerUp={handleDividerUp}
        onDoubleClick={() => {
          setLeftPct(50);
          saveRatio(50);
        }}
        onKeyDown={handleDividerKey}
        className="z-10 w-1 shrink-0 cursor-col-resize touch-none bg-border hover:bg-accent/50 focus-visible:bg-accent/50 focus-visible:outline-none"
      />

      {renderColumn('right', right)}

      <Palette
        open={paletteFor !== null}
        projects={projects}
        onClose={() => setPaletteFor(null)}
        onOpenSession={(sessionId) => {
          if (paletteFor) onChooseSession(paletteFor, sessionId);
          setPaletteFor(null);
        }}
        onNewSession={(projectId) => {
          if (paletteFor) onNewSessionInPane(paletteFor, projectId);
          setPaletteFor(null);
        }}
        onOpenProjectDialog={onOpenProjectDialog}
        onHome={onHome}
        onOpenPage={onOpenPage}
        onOpenProject={onOpenProject}
      />
    </div>
  );
}
