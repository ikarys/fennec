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

  const load = useCallback(async () => {
    setError(null);
    try {
      setArtifacts(await listSessionCanvas(sessionId));
    } catch (err) {
      setError(err instanceof Error ? err.message : tRef.current('unknownError'));
    }
  }, [sessionId]);

  useEffect(() => {
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
