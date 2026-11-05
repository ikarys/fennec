import { useCallback, useEffect, useRef, useState } from 'react';
import { Badge } from '../ui/Badge';
import { Menu, MenuItem } from '../ui/Menu';
import { LayersIcon } from '../Icons';
import { useT } from '../../i18n';
import { listSessionCanvas, type CanvasArtifact } from '../../lib/api/canvas';

const REFRESH_QUIET_MS = 500;

interface SessionCanvasMenuProps {
  sessionId: string;
  refreshKey?: unknown;
  onOpen: (artifactId: number) => void;
}

export function SessionCanvasMenu({ sessionId, refreshKey, onOpen }: SessionCanvasMenuProps) {
  const t = useT();
  const [artifacts, setArtifacts] = useState<CanvasArtifact[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const tRef = useRef(t);
  tRef.current = t;
  const firstRefreshRef = useRef(true);
  const onOpenRef = useRef(onOpen);
  onOpenRef.current = onOpen;
  // The newest change seen; a later one (the agent's canvas tool) opens its canvas.
  const seenRef = useRef<number | null>(null);

  const load = useCallback(async () => {
    setError(null);
    try {
      const list = await listSessionCanvas(sessionId);
      // The server lists the most recently changed first.
      const newest = list[0];
      if (newest && seenRef.current != null && newest.updatedAt > seenRef.current) onOpenRef.current(newest.id);
      seenRef.current = Math.max(seenRef.current ?? 0, newest?.updatedAt ?? 0);
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
    <Menu label={t('canvasMenu')} icon={<LayersIcon className="h-4 w-4" />}>
      {error ? (
        <div className="px-2 py-1 text-xs text-err">{error}</div>
      ) : artifacts == null ? (
        <div className="px-2 py-1 text-xs text-muted">{t('loading')}</div>
      ) : artifacts.length === 0 ? (
        <div className="px-2 py-1 text-xs text-muted">{t('canvasEmpty')}</div>
      ) : (
        artifacts.map((artifact) => (
          <MenuItem key={artifact.id} onSelect={() => onOpen(artifact.id)}>
            <span className="flex items-center gap-2">
              <span className="truncate">{artifact.title}</span>
              <Badge tone="neutral" pill>{artifact.format}</Badge>
            </span>
          </MenuItem>
        ))
      )}
    </Menu>
  );
}
