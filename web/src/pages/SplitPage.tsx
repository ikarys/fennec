import { useEffect, useRef, useState, type PointerEvent, type KeyboardEvent } from 'react';
import type { Project } from '../lib/api';
import { SessionPage } from './SessionPage';
import { Palette } from '../components/Palette';
import { Button } from '../components/ui/Button';
import { PlusIcon } from '../components/Icons';

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
  onChooseSession: (side: 'left' | 'right', sessionId: string) => void;
  onNewSessionInPane: (side: 'left' | 'right', projectId: string) => void;
  onSessionCreated: (side: 'left' | 'right', newId: string, pseudoId: string | null) => void;
  onRunningChanged: () => void;
  onRenamed: () => void;
  onOpenProjectDialog: () => void;
  onHome: () => void;
}

/**
 * Two sessions side by side, like two browser windows in one page. Each pane is
 * a full, independent SessionPage (its own thread, composer and SSE stream); the
 * panes' ids live in the URL, so the split survives a refresh.
 */
export function SplitPage({
  left,
  right,
  projects,
  onChooseSession,
  onNewSessionInPane,
  onSessionCreated,
  onRunningChanged,
  onRenamed,
  onOpenProjectDialog,
  onHome,
}: SplitPageProps) {
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
        <div className="flex min-h-0 min-w-0 grow flex-col items-center justify-center gap-3 p-6 text-center">
          <p className="text-sm text-muted">Sélectionne une session pour ce volet</p>
          <Button onClick={() => setPaletteFor(side)}>
            <PlusIcon className="h-4 w-4" />
            Choisir une session
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
      />
    );
  };

  return (
    <div ref={containerRef} className="flex min-h-0 min-w-0 grow">
      <div className="flex min-h-0 min-w-0" style={{ width: `${leftPct}%` }}>
        {renderPane('left', left)}
      </div>

      <div
        role="separator"
        aria-orientation="vertical"
        aria-label="Largeur du volet de gauche"
        aria-valuemin={MIN_PCT}
        aria-valuemax={MAX_PCT}
        aria-valuenow={leftPct}
        tabIndex={0}
        title="Glisser pour redimensionner · double-clic : 50/50"
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

      <div className="flex min-h-0 min-w-0 grow">
        {renderPane('right', right)}
      </div>

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
      />
    </div>
  );
}
