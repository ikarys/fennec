/**
 * The project's Files tab: a file tree beside the selected file. A session
 * shows the file as that session's branch has it.
 */
import { useEffect, useMemo, useState } from 'react';
import { Badge } from '../../ui/Badge';
import { buttonClass } from '../../ui/Button';
import { InfoBar } from '../../ui/InfoBar';
import { SectionLabel } from '../../ui/SectionLabel';
import { FileTree } from './FileTree';
import { FileView } from './FileView';
import { getProjectFiles, type ProjectFileListing } from '../../../lib/api';
import { buildFileTree } from '../../../lib/fileTree';
import { projectFilePath } from '../../../lib/route';
import { useT } from '../../../i18n';

export function FilesSection({
  projectId,
  path,
  sessionId,
}: {
  projectId: string;
  path: string | null;
  sessionId: string | null;
}) {
  const [listing, setListing] = useState<ProjectFileListing | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const t = useT();

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setError(null);

    getProjectFiles(projectId, sessionId)
      .then((data) => {
        if (cancelled) return;
        setListing(data);
        setLoading(false);
      })
      .catch((err: unknown) => {
        if (cancelled) return;
        setError(err instanceof Error ? err.message : t('unknownError'));
        setLoading(false);
      });

    return () => {
      cancelled = true;
    };
  }, [projectId, sessionId]);

  const nodes = useMemo(() => (listing ? buildFileTree(listing.files) : []), [listing]);

  const select = (file: string) => {
    window.location.hash = projectFilePath(projectId, file, sessionId);
  };

  return (
    <section>
      {loading ? (
        <p className="text-muted">{t('loading')}</p>
      ) : error ? (
        <p className="text-err">{error}</p>
      ) : (
        <div className="flex flex-col gap-4">
          {listing?.session ? (
            <InfoBar>
              <span className="flex flex-wrap items-center gap-2">
                {t('filesSessionVersion')}
                <button
                  type="button"
                  className={buttonClass({ variant: 'ghost', size: 'sm' })}
                  onClick={() => {
                    window.location.hash = projectFilePath(projectId, path);
                  }}
                >
                  {t('filesSeeWorkingTree')}
                </button>
              </span>
            </InfoBar>
          ) : null}

          <div className="flex flex-col gap-6 md:flex-row md:items-start">
            <nav
              aria-label={t('projectPageFiles')}
              className="flex shrink-0 flex-col md:sticky md:top-6 md:w-72 md:max-h-[calc(100vh-8rem)] md:overflow-y-auto"
            >
              <SectionLabel className="px-2.5 pb-1 pt-0.5">
                <span className="flex items-center gap-2">
                  {t('projectPageFiles')}
                  <Badge tone="neutral">{listing?.files.length ?? 0}</Badge>
                </span>
              </SectionLabel>
              <FileTree nodes={nodes} selected={path} onSelect={select} />
              {listing?.truncated ? (
                <p className="px-2.5 pt-2 text-xs text-muted">{t('filesTruncated')}</p>
              ) : null}
            </nav>

            <div className="min-w-0 flex-1">
              {path ? (
                <FileView
                  key={`${sessionId ?? ''}:${path}`}
                  projectId={projectId}
                  path={path}
                  sessionId={sessionId}
                />
              ) : (
                <p className="text-xs text-muted">{t('filesPickOne')}</p>
              )}
            </div>
          </div>
        </div>
      )}
    </section>
  );
}
