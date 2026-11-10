import { useCallback, useEffect, useRef, useState, type KeyboardEvent as ReactKeyboardEvent } from 'react';
import { Badge } from '../ui/Badge';
import { Button } from '../ui/Button';
import { ConfirmDialog } from '../ui/ConfirmDialog';
import { Section } from '../ui/Section';
import { TrashIcon, WandIcon } from '../ui/Icons';
import { useT } from '../../i18n';
import { CanvasPane } from '../canvas/CanvasPane';
import { deleteCanvas, listProjectCanvas, type CanvasArtifact } from '../../lib/api/canvas';

export function CanvasSection({ projectId }: { projectId: string }) {
  const t = useT();
  const [artifacts, setArtifacts] = useState<CanvasArtifact[] | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [openArtifactId, setOpenArtifactId] = useState<number | null>(null);
  const [deleteId, setDeleteId] = useState<number | null>(null);
  const [deleteError, setDeleteError] = useState<string | null>(null);
  const tRef = useRef(t);
  tRef.current = t;

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      setArtifacts(await listProjectCanvas(projectId));
    } catch (err) {
      setError(err instanceof Error ? err.message : tRef.current('unknownError'));
    } finally {
      setLoading(false);
    }
  }, [projectId]);

  useEffect(() => {
    void load();
  }, [load]);

  const handleDelete = async () => {
    if (deleteId == null) return;
    setDeleteError(null);
    try {
      await deleteCanvas(deleteId);
      setDeleteId(null);
      void load();
    } catch (err) {
      setDeleteError(err instanceof Error ? err.message : tRef.current('unknownError'));
      setDeleteId(null);
    } finally {
    }
  };

  return (
    <section>
      <Section Icon={WandIcon} title={t('canvasLibraryTab')} flush>
        <div className="flex items-center gap-2 border-b border-border bg-raised px-4 py-2 text-xs text-muted">
          <span className="font-medium text-text">{t('canvasLibraryTitle')}</span>
          <Badge tone="neutral">{artifacts?.length ?? 0}</Badge>
        </div>
        {loading ? (
          <div className="p-4 text-sm text-muted">{t('loading')}</div>
        ) : error ? (
          <div className="p-4 text-sm text-err">{error}</div>
        ) : artifacts && artifacts.length === 0 ? (
          <div className="p-6 text-center text-sm text-muted">{t('canvasLibraryEmpty')}</div>
        ) : (
          <ul className="divide-y divide-border">
            {artifacts?.map((artifact) => (
              <li key={artifact.id}>
                <div
                  role="button"
                  tabIndex={0}
                  aria-label={artifact.title}
                  onClick={() => setOpenArtifactId(artifact.id)}
                  onKeyDown={(event: ReactKeyboardEvent<HTMLDivElement>) => {
                    if (event.key === 'Enter' || event.key === ' ') {
                      event.preventDefault();
                      setOpenArtifactId(artifact.id);
                    }
                  }}
                  className="group flex w-full cursor-pointer items-center gap-3 px-4 py-3 text-left text-sm hover:bg-accent-soft focus-visible:bg-accent-soft focus-visible:outline-none"
                >
                  <span className="min-w-0 grow truncate font-medium group-hover:text-accent-strong">
                    {artifact.title}
                  </span>
                  <Badge tone="neutral" appearance="pill">{artifact.format}</Badge>
                  <span className="hidden shrink-0 font-mono text-xs text-muted md:inline">
                    {artifact.source.kind === 'file' ? artifact.source.path : t('canvasFromMessage')}
                  </span>
                  <span className="shrink-0 tabular-nums text-xs text-muted">v{artifact.latestVersion}</span>
                  <span className="shrink-0 text-xs text-muted">
                    {new Date(artifact.updatedAt).toLocaleString()}
                  </span>
                  <span className="shrink-0" onClick={(event) => event.stopPropagation()}>
                    <Button
                      variant="danger"
                      size="sm"
                      iconOnly
                      title={t('canvasDelete')}
                      aria-label={t('canvasDelete')}
                      onClick={() => setDeleteId(artifact.id)}
                    >
                      <TrashIcon className="h-4 w-4" />
                    </Button>
                  </span>
                </div>
              </li>
            ))}
          </ul>
        )}
        {deleteError ? <div className="border-t border-border px-4 py-2 text-xs text-err">{deleteError}</div> : null}
      </Section>

      {openArtifactId != null ? (
        <CanvasPane
          artifactId={openArtifactId}
          onClose={() => setOpenArtifactId(null)}
          onDeleted={() => {
            setOpenArtifactId(null);
            void load();
          }}
        />
      ) : null}

      <ConfirmDialog
        open={deleteId != null}
        title={t('canvasDeleteTitle')}
        body={t('canvasDeleteBody')}
        confirmLabel={t('canvasDelete')}
        variant="danger"
        onConfirm={() => void handleDelete()}
        onCancel={() => setDeleteId(null)}
      />
    </section>
  );
}
