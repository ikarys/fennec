/**
 * The files the agent touched, with their line deltas; each opens where it can be read (the diff while the
 * session's branch has commits, else the working tree), and a jump to the whole diff when there is one.
 */
import type { ModifiedFiles } from '../../lib/api';
import { ChevronRightIcon, FileIcon } from '../ui/Icons';
import { useFileLink } from './FileLinkContext';
import { Panel } from '../ui/Panel';
import { useT } from '../../i18n';
import { changesPath } from '../../lib/route';

interface ModifiedFilesSectionProps {
  totals: ModifiedFiles;
  sessionId: string | null;
  /** The session's branch has commits, so its diff page has something to show. */
  hasDiff: boolean;
}

export function ModifiedFilesSection({ totals, sessionId, hasDiff }: ModifiedFilesSectionProps) {
  const t = useT();
  const fileLink = useFileLink();

  const navigate = () => {
    if (sessionId !== null) {
      window.location.hash = changesPath(sessionId);
    }
  };

  return (
    <Panel
      id="modifiedFiles"
      title={t('panelModifiedFiles')}
      icon={<FileIcon className="h-3.5 w-3.5" />}
      summary={
        totals.count > 0 ? (
          <>
            {totals.count} <span className="text-ok">+{totals.added}</span>{' '}
            <span className="text-err">−{totals.removed}</span>
          </>
        ) : undefined
      }
    >
      {totals.count === 0 && <div className="text-xs text-muted">{t('noModifiedFiles')}</div>}

      {totals.count > 0 && sessionId !== null && (
        <div className="flex flex-col gap-0.5">
          {totals.files.map((file) => {
            const separator = file.path.lastIndexOf('/');
            const dir = separator >= 0 ? file.path.slice(0, separator + 1) : '';
            const name = separator >= 0 ? file.path.slice(separator + 1) : file.path;

            return (
              <a
                key={file.path}
                href={fileLink ? fileLink(file.path) : undefined}
                className="group flex min-w-0 items-center gap-2 rounded-md px-1 py-1 text-xs hover:bg-accent-soft"
                title={file.path}
              >
                <FileIcon className="h-3.5 w-3.5 shrink-0 text-muted" />
                <span className="min-w-0 grow truncate font-mono">
                  <span className="text-muted">{dir}</span>
                  <span className="text-text group-hover:text-accent-strong">{name}</span>
                </span>
                <span className="text-ok tabular-nums">+{file.added}</span>
                <span className="text-err tabular-nums">−{file.removed}</span>
              </a>
            );
          })}

          {totals.count > totals.files.length && (
            <div className="px-1 text-xs text-muted">
              {t('moreFiles', { count: totals.count - totals.files.length })}
            </div>
          )}

          {hasDiff && (
            <button
              type="button"
              onClick={navigate}
              className="self-start text-xs text-accent hover:text-accent-strong flex items-center gap-1"
            >
              {t('seeChanges')}
              <ChevronRightIcon className="h-3 w-3" />
            </button>
          )}
        </div>
      )}
    </Panel>
  );
}
