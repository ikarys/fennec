// The session's canvases, a tab of the side rail: newest first, one opens in the canvas pane.
import { useCallback, useEffect, useRef, useState } from 'react';
import { Badge } from '../ui/Badge';
import { LayersIcon } from '../ui/Icons';
import { useLang, useT } from '../../i18n';
import { formatDateTime } from '../../lib/format';
import { listSessionCanvas, type CanvasArtifact } from '../../lib/api/canvas';

const REFRESH_QUIET_MS = 500;

interface CanvasSectionProps {
  sessionId: string;
  refreshKey?: unknown;
  onOpen: (id: number) => void;
  onNew: (id: number) => void;
  onCount?: (count: number) => void;
}

export function CanvasSection({ sessionId, refreshKey, onOpen, onNew, onCount }: CanvasSectionProps) {
  const lang = useLang();
  const t = useT();
  const [artifacts, setArtifacts] = useState<CanvasArtifact[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const tRef = useRef(t);
  tRef.current = t;
  const firstRefreshRef = useRef(true);
  const onOpenRef = useRef(onOpen);
  onOpenRef.current = onOpen;
  const onNewRef = useRef(onNew);
  onNewRef.current = onNew;
  const onCountRef = useRef(onCount);
  onCountRef.current = onCount;
  // The newest change seen; a later one (the agent's canvas tool) opens its canvas.
  const seenRef = useRef<number | null>(null);

  const load = useCallback(async () => {
    setError(null);
    try {
      const list = await listSessionCanvas(sessionId);
      // The server lists the most recently changed first.
      const newest = list[0];
      if (newest && seenRef.current != null && newest.updatedAt > seenRef.current) onNewRef.current(newest.id);
      seenRef.current = Math.max(seenRef.current ?? 0, newest?.updatedAt ?? 0);
      onCountRef.current?.(list.length);
      setArtifacts(list);
    } catch (err) {
      setError(err instanceof Error ? err.message : tRef.current('unknownError'));
    }
  }, [sessionId]);

  useEffect(() => {
    seenRef.current = null;
    void load();
  }, [load]);

  useEffect(() => {
    if (firstRefreshRef.current) {
      firstRefreshRef.current = false;
      return;
    }
    const timer = window.setTimeout(() => {
      void load();
    }, REFRESH_QUIET_MS);
    return () => window.clearTimeout(timer);
  }, [refreshKey, load]);

  return (
    <div className="flex flex-col gap-0.5 p-2">
      {error ? (
        <div className="px-2 py-1 text-xs text-err">{error}</div>
      ) : artifacts == null ? (
        <div className="px-2 py-1 text-xs text-muted">{t('loading')}</div>
      ) : artifacts.length === 0 ? (
        <div className="px-2 py-1 text-xs text-muted">{t('canvasEmpty')}</div>
      ) : (
        artifacts.map((artifact) => (
          <button
            key={artifact.id}
            type="button"
            onClick={() => onOpen(artifact.id)}
            className="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left hover:bg-accent-soft"
          >
            <LayersIcon className="h-3.5 w-3.5 shrink-0 text-muted" />
            <span className="min-w-0 truncate text-sm">{artifact.title}</span>
            <Badge tone="neutral" appearance="pill">{artifact.format}</Badge>
            <span className="ml-auto shrink-0 text-[11px] text-faint">{formatDateTime(lang, artifact.updatedAt)}</span>
          </button>
        ))
      )}
    </div>
  );
}
