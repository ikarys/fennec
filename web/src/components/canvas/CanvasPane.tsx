import { useCallback, useEffect, useRef, useState, type FormEvent, type PointerEvent as ReactPointerEvent } from 'react';
import { Badge } from '../ui/Badge';
import { Button, buttonClass } from '../ui/Button';
import { ConfirmDialog } from '../ui/ConfirmDialog';
import { TextArea } from '../ui/Field';
import {
  CloseIcon,
  EditIcon,
  EraserIcon,
  LocateIcon,
  RefreshIcon,
  SpinnerIcon,
  TrashIcon,
  ChevronsLeftIcon,
  ChevronsRightIcon,
} from '../Icons';
import { DocumentView } from '../preview/DocumentView';
import { useT } from '../../i18n';
import { previewUrl, type PreviewKind } from '../../lib/preview';
import {
  canvasVersionUrl,
  deleteCanvas,
  getCanvas,
  annotateCanvas,
  type CanvasAnchor,
  type CanvasDetail,
  type CanvasFormat,
} from '../../lib/api/canvas';

const REFRESH_QUIET_MS = 500;

interface CanvasPaneProps {
  artifactId: number;
  refreshKey?: unknown;
  onClose: () => void;
  onAnnotate?: (message: string) => void;
  onDeleted?: () => void;
}

function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value));
}

function canvasFormatToKind(format: CanvasFormat): PreviewKind {
  if (format === 'html') return 'html';
  if (format === 'markdown') return 'markdown';
  if (format === 'json') return 'json';
  if (format === 'mermaid') return 'mermaid';
  if (format === 'svg') return 'svg';
  if (format === 'pdf') return 'pdf';
  return 'image';
}

function RegionOverlay({ onCommit }: { onCommit: (anchor: CanvasAnchor) => void }) {
  const [drag, setDrag] = useState<{ x0: number; y0: number; x1: number; y1: number } | null>(null);

  const pointFromEvent = (event: ReactPointerEvent<HTMLDivElement>) => {
    const rect = event.currentTarget.getBoundingClientRect();
    return {
      x: clamp(event.clientX - rect.left, 0, rect.width),
      y: clamp(event.clientY - rect.top, 0, rect.height),
      rect,
    };
  };

  const handlePointerDown = (event: ReactPointerEvent<HTMLDivElement>) => {
    event.preventDefault();
    event.currentTarget.setPointerCapture(event.pointerId);
    const { x, y } = pointFromEvent(event);
    setDrag({ x0: x, y0: y, x1: x, y1: y });
  };

  const handlePointerMove = (event: ReactPointerEvent<HTMLDivElement>) => {
    if (!drag) return;
    const { x, y } = pointFromEvent(event);
    setDrag((current) => (current ? { ...current, x1: x, y1: y } : current));
  };

  const handlePointerEnd = (event: ReactPointerEvent<HTMLDivElement>) => {
    if (!drag) return;
    if (event.currentTarget.hasPointerCapture(event.pointerId)) {
      event.currentTarget.releasePointerCapture(event.pointerId);
    }
    const { rect } = pointFromEvent(event);
    const left = Math.min(drag.x0, drag.x1);
    const top = Math.min(drag.y0, drag.y1);
    const width = Math.abs(drag.x1 - drag.x0);
    const height = Math.abs(drag.y1 - drag.y0);
    if (rect.width > 0 && rect.height > 0 && width > 4 && height > 4) {
      onCommit({
        kind: 'region',
        x: left / rect.width,
        y: top / rect.height,
        width: width / rect.width,
        height: height / rect.height,
      });
    } else {
      onCommit({ kind: 'whole' });
    }
    setDrag(null);
  };

  const left = drag ? Math.min(drag.x0, drag.x1) : 0;
  const top = drag ? Math.min(drag.y0, drag.y1) : 0;
  const width = drag ? Math.abs(drag.x1 - drag.x0) : 0;
  const height = drag ? Math.abs(drag.y1 - drag.y0) : 0;

  return (
    <div
      className="absolute inset-0 cursor-crosshair touch-none select-none"
      onPointerDown={handlePointerDown}
      onPointerMove={handlePointerMove}
      onPointerUp={handlePointerEnd}
      onPointerCancel={handlePointerEnd}
    >
      {drag ? (
        <div
          className="pointer-events-none absolute border border-accent bg-accent/10"
          style={{ left, top, width, height }}
        />
      ) : null}
    </div>
  );
}

export function CanvasPane({ artifactId, refreshKey, onClose, onAnnotate, onDeleted }: CanvasPaneProps) {
  const t = useT();
  const [detail, setDetail] = useState<CanvasDetail | null>(null);
  const [viewedVersion, setViewedVersion] = useState<number | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [reloadKey, setReloadKey] = useState(0);
  const [annotateMode, setAnnotateMode] = useState(false);
  const [regionMode, setRegionMode] = useState(false);
  const [anchor, setAnchor] = useState<CanvasAnchor | null>(null);
  const [note, setNote] = useState('');
  const [sending, setSending] = useState(false);
  const [submitError, setSubmitError] = useState<string | null>(null);
  const [confirmOpen, setConfirmOpen] = useState(false);
  const [deleteError, setDeleteError] = useState<string | null>(null);

  const tRef = useRef(t);
  tRef.current = t;
  const previousLatestRef = useRef<number | null>(null);
  const firstRefreshRef = useRef(true);

  const load = useCallback(async (id: number) => {
    setLoading(true);
    setError(null);
    try {
      const data = await getCanvas(id);
      const latest = data.artifact.latestVersion;
      setDetail(data);
      setViewedVersion((prev) => {
        if (prev == null) return latest;
        if (prev > latest) return latest;
        if (
          previousLatestRef.current != null &&
          latest > previousLatestRef.current &&
          prev === previousLatestRef.current
        ) {
          return latest;
        }
        return prev;
      });
      previousLatestRef.current = latest;
    } catch (err) {
      setError(err instanceof Error ? err.message : tRef.current('unknownError'));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    previousLatestRef.current = null;
    setDetail(null);
    setViewedVersion(null);
    setAnchor(null);
    setNote('');
    setSubmitError(null);
    setDeleteError(null);
    setLoading(true);
    setError(null);
    void load(artifactId);
  }, [artifactId, load]);

  useEffect(() => {
    if (firstRefreshRef.current) {
      firstRefreshRef.current = false;
      return;
    }
    const timer = window.setTimeout(() => {
      void load(artifactId);
    }, REFRESH_QUIET_MS);
    return () => window.clearTimeout(timer);
  }, [refreshKey, artifactId, load]);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape' && !confirmOpen) onClose();
    };
    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, [onClose, confirmOpen]);

  const artifact = detail?.artifact ?? null;
  const latest = artifact?.latestVersion ?? 0;
  const url =
    artifact && viewedVersion != null
      ? artifact.source.kind === 'file' && viewedVersion === artifact.latestVersion
        ? previewUrl(artifact.sessionId, artifact.source.path)
        : canvasVersionUrl(artifact.id, viewedVersion)
      : '';

  const canPrev = viewedVersion != null && viewedVersion > 1;
  const canNext = viewedVersion != null && viewedVersion < latest;
  const isTextFormat = artifact?.format === 'markdown' || artifact?.format === 'json';
  const showOverlay = annotateMode && (!isTextFormat || regionMode);

  const handleTextMouseUp = () => {
    if (!annotateMode || regionMode) return;
    const text = window.getSelection()?.toString();
    if (text && text.trim().length > 0) {
      setAnchor({ kind: 'quote', text: text.trim() });
    } else {
      setAnchor({ kind: 'whole' });
    }
  };

  const anchorLabel = (value: CanvasAnchor | null): string => {
    if (!value || value.kind === 'whole') return t('canvasAnchorWhole');
    if (value.kind === 'quote') {
      const text = value.text.length > 40 ? `${value.text.slice(0, 40)}…` : value.text;
      return `${t('canvasAnchorQuote')}: ${text}`;
    }
    return t('canvasAnchorRegion');
  };

  const anchorKindLabel = (value: CanvasAnchor): string => {
    if (value.kind === 'whole') return t('canvasAnchorWhole');
    if (value.kind === 'quote') return t('canvasAnchorQuote');
    return t('canvasAnchorRegion');
  };

  const submit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (!artifact || viewedVersion == null || note.trim().length === 0 || sending) return;
    setSending(true);
    setSubmitError(null);
    try {
      const result = await annotateCanvas(artifact.id, viewedVersion, anchor ?? { kind: 'whole' }, note.trim());
      onAnnotate?.(result.message);
      setNote('');
      setAnchor(null);
      await load(artifact.id);
    } catch (err) {
      setSubmitError(err instanceof Error ? err.message : t('unknownError'));
    } finally {
      setSending(false);
    }
  };

  const handleDelete = async () => {
    if (!artifact) return;
    setDeleteError(null);
    try {
      await deleteCanvas(artifact.id);
      setConfirmOpen(false);
      onDeleted?.();
      onClose();
    } catch (err) {
      setDeleteError(err instanceof Error ? err.message : t('unknownError'));
      setConfirmOpen(false);
    }
  };

  return (
    <div className="fixed inset-y-0 right-0 z-40 flex w-[min(960px,92vw)] flex-col border-l border-border bg-bg shadow-xl">
      <header className="flex flex-wrap items-center gap-2 border-b border-border px-4 py-3">
        <span className="min-w-0 grow truncate text-sm font-medium">
          {artifact?.title ?? t('canvasMenu')}
        </span>
        {artifact ? <Badge tone="neutral" pill>{artifact.format}</Badge> : null}
        {artifact?.source.kind === 'file' ? (
          <span className="hidden max-w-40 truncate font-mono text-xs text-muted xl:inline">
            {artifact.source.path}
          </span>
        ) : null}
        <div className="flex items-center gap-1">
          <Button
            variant="ghost"
            size="sm"
            iconOnly
            title={t('canvasPrevVersion')}
            aria-label={t('canvasPrevVersion')}
            disabled={!canPrev}
            onClick={() => setViewedVersion((value) => (value != null && value > 1 ? value - 1 : value))}
          >
            <ChevronsLeftIcon className="h-4 w-4" />
          </Button>
          <span className="tabular-nums text-xs text-muted">
            {t('canvasVersionOf', { n: viewedVersion ?? 0, latest })}
          </span>
          <Button
            variant="ghost"
            size="sm"
            iconOnly
            title={t('canvasNextVersion')}
            aria-label={t('canvasNextVersion')}
            disabled={!canNext}
            onClick={() =>
              setViewedVersion((value) => (value != null && value < latest ? value + 1 : value))
            }
          >
            <ChevronsRightIcon className="h-4 w-4" />
          </Button>
        </div>
        <Button
          variant="ghost"
          size="sm"
          iconOnly
          title={t('canvasReload')}
          aria-label={t('canvasReload')}
          onClick={() => setReloadKey((value) => value + 1)}
        >
          <RefreshIcon className="h-4 w-4" />
        </Button>
        {url ? (
          <a
            href={url}
            target="_blank"
            rel="noreferrer"
            className={buttonClass({ variant: 'ghost', size: 'sm' })}
          >
            {t('canvasOpenTab')}
          </a>
        ) : null}
        {onAnnotate ? (
          <Button
            variant={annotateMode ? 'soft' : 'ghost'}
            size="sm"
            iconOnly
            title={t('canvasAnnotate')}
            aria-label={t('canvasAnnotate')}
            aria-pressed={annotateMode}
            onClick={() => setAnnotateMode((value) => !value)}
          >
            <EditIcon className="h-4 w-4" />
          </Button>
        ) : null}
        <Button
          variant="danger"
          size="sm"
          iconOnly
          title={t('canvasDelete')}
          aria-label={t('canvasDelete')}
          onClick={() => setConfirmOpen(true)}
        >
          <TrashIcon className="h-4 w-4" />
        </Button>
        <Button
          variant="ghost"
          size="sm"
          iconOnly
          title={t('close')}
          aria-label={t('close')}
          onClick={onClose}
        >
          <CloseIcon className="h-4 w-4" />
        </Button>
      </header>

      {deleteError ? <div className="border-b border-border bg-err/10 px-4 py-2 text-xs text-err">{deleteError}</div> : null}

      <div className="relative flex min-h-0 grow flex-col">
        {loading && !detail ? (
          <div className="p-4 text-sm text-muted">{t('loading')}</div>
        ) : error ? (
          <div className="p-4 text-sm text-err">{error}</div>
        ) : !artifact || viewedVersion == null ? null : (
          <DocumentView
            kind={canvasFormatToKind(artifact.format)}
            url={url}
            title={artifact.title}
            reloadKey={`${artifact.id}-${viewedVersion}-${reloadKey}`}
            overlay={showOverlay ? <RegionOverlay onCommit={setAnchor} /> : null}
            onTextMouseUp={handleTextMouseUp}
          />
        )}
      </div>

      {annotateMode && onAnnotate ? (
        <form
          className="flex flex-col gap-2 border-t border-border bg-raised p-3"
          onSubmit={(event) => void submit(event)}
        >
          <div className="flex items-center gap-2">
            <span className="min-w-0 grow truncate text-xs text-muted">{anchorLabel(anchor)}</span>
            <Button
              variant="ghost"
              size="sm"
              iconOnly
              title={t('canvasClearAnchor')}
              aria-label={t('canvasClearAnchor')}
              onClick={() => setAnchor(null)}
            >
              <EraserIcon className="h-4 w-4" />
            </Button>
            {isTextFormat ? (
              <Button
                variant={regionMode ? 'soft' : 'ghost'}
                size="sm"
                iconOnly
                title={t('canvasRegionMode')}
                aria-label={t('canvasRegionMode')}
                aria-pressed={regionMode}
                onClick={() => setRegionMode((value) => !value)}
              >
                <LocateIcon className="h-4 w-4" />
              </Button>
            ) : null}
          </div>
          {submitError ? <div className="text-xs text-err">{submitError}</div> : null}
          <TextArea
            value={note}
            onChange={(event) => setNote(event.target.value)}
            placeholder={t('canvasNotePlaceholder')}
            className="min-h-20"
          />
          <div className="flex justify-end">
            <Button
              type="submit"
              variant="primary"
              size="sm"
              disabled={note.trim().length === 0 || sending}
            >
              {sending ? <SpinnerIcon className="h-4 w-4" /> : null}
              {t('canvasAnnotateSend')}
            </Button>
          </div>
        </form>
      ) : null}

      {detail && detail.annotations.length > 0 ? (
        <details className="border-t border-border">
          <summary className="cursor-pointer list-none px-4 py-2 text-xs text-muted hover:bg-raised">
            {t('canvasAnnotations', { count: detail.annotations.length })}
          </summary>
          <ul className="max-h-40 overflow-y-auto border-t border-border px-4 py-2 text-xs text-muted">
            {detail.annotations.map((annotation) => (
              <li key={annotation.id} className="break-words py-1">
                <span className="tabular-nums">v{annotation.version}</span>
                {' · '}
                <span>{anchorKindLabel(annotation.anchor)}</span>
                {' · '}
                <span className="text-text">{annotation.note}</span>
              </li>
            ))}
          </ul>
        </details>
      ) : null}

      <ConfirmDialog
        open={confirmOpen}
        title={t('canvasDeleteTitle')}
        body={t('canvasDeleteBody')}
        confirmLabel={t('canvasDelete')}
        variant="danger"
        onConfirm={() => void handleDelete()}
        onCancel={() => setConfirmOpen(false)}
      />
    </div>
  );
}
