// A session's changes: its branch's files in review order, the selected one's diff.
import { Fragment, useEffect, useRef, useState } from 'react';
import { getFileDiff, getIntegration, type FileDiff, type IntegrationPreview, type ReviewSignal } from '../lib/api/integration';
import { editorPath, tabPath } from '../lib/route';
import { useOpenInEditorLabel } from '../lib/editors';
import { useT } from '../i18n';

import { Badge } from '../components/ui/Badge';
import { Button, buttonClass } from '../components/ui/Button';
import { Card } from '../components/ui/Card';
import { ListCard } from '../components/ui/ListCard';
import { BranchIcon, ChevronsLeftIcon, EditIcon, ExternalIcon, FileIcon } from '../components/ui/Icons';
import { SIGNAL_KEYS } from '../components/session/IntegrationReview';

export default function ChangesPage({
  sessionId,
  initialPath,
  projectId,
}: {
  sessionId: string;
  initialPath: string | null;
  projectId: string | null;
}) {
  const t = useT();
  const openInEditor = useOpenInEditorLabel('openInEditor');
  const [integration, setIntegration] = useState<IntegrationPreview | null>(null);
  const [integrationError, setIntegrationError] = useState<string | null>(null);
  const [selectedPath, setSelectedPath] = useState<string | null>(null);
  const [diff, setDiff] = useState<FileDiff | null>(null);
  const [diffLoading, setDiffLoading] = useState(false);
  const [diffError, setDiffError] = useState<string | null>(null);
  const diffCache = useRef<Map<string, FileDiff>>(new Map());

  useEffect(() => {
    let cancelled = false;
    setIntegration(null);
    setIntegrationError(null);
    setSelectedPath(null);
    setDiff(null);
    setDiffError(null);
    setDiffLoading(false);
    getIntegration(sessionId)
      .then((data) => {
        if (cancelled) return;
        setIntegration(data);
        if (data.files.length > 0) {
          const initial =
            initialPath !== null ? data.files.find((file) => file.path === initialPath)?.path : null;
          setSelectedPath(initial ?? data.files[0].path);
        }
      })
      .catch((error) => {
        if (!cancelled) setIntegrationError(error instanceof Error ? error.message : String(error));
      });
    return () => {
      cancelled = true;
    };
  }, [sessionId, initialPath]);

  useEffect(() => {
    if (!selectedPath) return;
    const path = selectedPath;
    const cached = diffCache.current.get(path);
    if (cached) {
      setDiff(cached);
      setDiffError(null);
      setDiffLoading(false);
      return;
    }
    let cancelled = false;
    setDiff(null);
    setDiffError(null);
    setDiffLoading(true);
    getFileDiff(sessionId, path)
      .then((data) => {
        if (cancelled) return;
        diffCache.current.set(path, data);
        setDiff(data);
        setDiffLoading(false);
      })
      .catch((error) => {
        if (cancelled) return;
        setDiffError(error instanceof Error ? error.message : String(error));
        setDiffLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [sessionId, selectedPath]);

  const files = integration?.files ?? [];
  const selectedFile = files.find((file) => file.path === selectedPath);

  return (
    <div className="flex grow flex-col min-h-0 overflow-hidden p-4 gap-3">
      <div className="flex items-center gap-3">
        <Button
          variant="icon"
          onClick={() => {
            window.location.hash = tabPath(sessionId);
          }}
          title={t('backToSession')}
          aria-label={t('backToSession')}
        >
          <ChevronsLeftIcon className="h-4 w-4" />
        </Button>
        <span className="grid size-7 shrink-0 place-items-center rounded-md bg-accent-soft text-accent">
          <BranchIcon className="h-3.5 w-3.5" />
        </span>
        <h1 className="text-sm font-semibold">{t('changesTitle')}</h1>
        {integration?.branch ? <span className="font-mono text-xs text-muted">{integration.branch}</span> : null}
        {integration ? <Badge tone="ok" title={t('verifiedFromLog')}>{t('verified')}</Badge> : null}
      </div>
      <div className="flex min-h-0 grow gap-3">
        <div className="w-96 shrink-0 overflow-y-auto">
          <ListCard title={t('panelModifiedFiles')} count={files.length}>
            {/* An error is told once, in the diff card's empty state. */}
            {integrationError ? null : !integration ? (
              <div className="px-4 py-2 text-xs text-muted">{t('loading')}</div>
            ) : (
              files.map((file) => {
                const selected = file.path === selectedPath;
                // A signal every file carries says nothing about this one: the server lists those apart.
                const signals = file.signals.filter(
                  (signal): signal is Exclude<ReviewSignal, 'protected'> =>
                    signal !== 'protected' && !integration.commonSignals.includes(signal),
                );
                return (
                  <button
                    key={file.path}
                    type="button"
                    onClick={() => setSelectedPath(file.path)}
                    className={`flex w-full flex-col py-1.5 text-left font-mono text-xs hover:bg-accent-soft${
                      selected ? ' bg-accent-soft text-accent-strong' : ''
                    }`}
                  >
                    <span className="flex items-center gap-2 px-4">
                      <span dir="rtl" title={file.path} className="grow overflow-hidden text-ellipsis whitespace-nowrap">
                        <bdi>{file.path}</bdi>
                      </span>
                      <span className="text-ok tabular-nums">+{file.added}</span>
                      <span className="text-err tabular-nums">−{file.removed}</span>
                    </span>
                    {(file.protected || file.carried || signals.length > 0) && (
                      <span className="flex flex-wrap gap-1 px-4 pb-1.5">
                        {file.protected ? <Badge tone="err">{t('protectedBadge')}</Badge> : null}
                        {file.carried ? <Badge tone="accent" title={t('carriedTitle')}>{t('carriedBadge')}</Badge> : null}
                        {signals.map((signal) => {
                          const config = SIGNAL_KEYS[signal];
                          return (
                            <Badge key={signal} tone={config.tone} title={t(config.title)}>
                              {t(config.label)}
                            </Badge>
                          );
                        })}
                      </span>
                    )}
                  </button>
                );
              })
            )}
          </ListCard>
        </div>
        <Card className="flex min-w-0 grow flex-col overflow-hidden">
          {selectedPath === null ? (
            <div className="flex grow flex-col items-center justify-center gap-3 p-8 text-center">
              <span className="grid size-10 place-items-center rounded-lg bg-accent-soft text-accent">
                <FileIcon className="h-5 w-5" />
              </span>
              <p className="max-w-xs text-sm text-muted">
                {integrationError ?? (integration && files.length === 0 ? t('changesNoFiles') : t('changesPickFile'))}
              </p>
            </div>
          ) : (
            <>
              <div className="flex items-center gap-3 border-b border-border bg-linear-to-r from-accent-soft to-transparent to-75% px-4 py-2.5">
                <span className="grow truncate font-mono text-sm" title={selectedPath ?? ''}>
                  {selectedPath}
                </span>
                {selectedFile ? (
                  <>
                    <span className="text-xs text-ok tabular-nums">+{selectedFile.added}</span>
                    <span className="text-xs text-err tabular-nums">−{selectedFile.removed}</span>
                  </>
                ) : null}
                <span className="ml-auto flex items-center gap-1">
                  {projectId && selectedPath ? (
                    <a
                      href={editorPath(projectId, selectedPath, sessionId)}
                      className={buttonClass({ variant: 'ghost', size: 'sm', iconOnly: true })}
                      title={t('changesOpenFile')}
                      aria-label={t('changesOpenFile')}
                    >
                      <EditIcon className="h-3.5 w-3.5" />
                    </a>
                  ) : null}
                  {diff ? (
                    <a
                      href={diff.editorUrl}
                      className="rounded-md p-1 text-muted hover:bg-accent-soft hover:text-accent"
                      title={openInEditor}
                      aria-label={openInEditor}
                    >
                      <ExternalIcon className="h-3.5 w-3.5" />
                    </a>
                  ) : null}
                </span>
              </div>
              <div className="grow overflow-auto">
                {diffLoading ? (
                  <div className="px-4 py-2 text-xs text-muted">{t('loading')}</div>
                ) : diffError ? (
                  <div className="px-4 py-2 text-xs text-muted">{diffError}</div>
                ) : diff?.binary ? (
                  <div className="px-4 py-2 text-xs text-muted">{t('diffBinary')}</div>
                ) : diff && diff.hunks.length === 0 ? (
                  <div className="px-4 py-2 text-xs text-muted">{t('diffEmpty')}</div>
                ) : diff ? (
                  <>
                    <table className="w-full border-collapse font-mono text-xs">
                      <tbody>
                        {diff.hunks.map((hunk, hunkIndex) => (
                          <Fragment key={hunkIndex}>
                            <tr>
                              <td colSpan={3} className="bg-info-soft px-3 py-1 text-info">
                                {hunk.header ? `@@ ${hunk.header}` : '@@'}
                              </td>
                            </tr>
                            {hunk.lines.map((line, lineIndex) => (
                              <tr
                                key={lineIndex}
                                className={line.kind === 'added' ? 'bg-add-bg' : line.kind === 'removed' ? 'bg-del-bg' : ''}
                              >
                                <td className="w-12 select-none px-2 text-right text-faint tabular-nums">{line.old ?? ''}</td>
                                <td className="w-12 select-none px-2 text-right text-faint tabular-nums">{line.new ?? ''}</td>
                                <td className="whitespace-pre px-3">
                                  <span className="select-none">{line.kind === 'added' ? '+' : line.kind === 'removed' ? '-' : ' '}</span>
                                  {line.text}
                                </td>
                              </tr>
                            ))}
                          </Fragment>
                        ))}
                      </tbody>
                    </table>
                    {diff.truncated ? <div className="px-4 py-2 text-xs text-muted">{t('diffTruncated')}</div> : null}
                  </>
                ) : null}
              </div>
            </>
          )}
        </Card>
      </div>
    </div>
  );
}
