import { useMemo } from 'react';
import type { ThreadView } from '../../lib/fold';
import { relativeToWorkdir } from '../../lib/preview';
import { usePreviewWorkdir } from '../preview/PreviewContext';
import { ChevronRightIcon, FileIcon } from '../ui/Icons';
import { Panel } from '../ui/Panel';
import { useT } from '../../i18n';
import { changesPath } from '../../lib/route';

interface ModifiedFilesSectionProps {
  modifiedFiles: ThreadView['modifiedFiles'];
  sessionId: string | null;
}

export function ModifiedFilesSection({ modifiedFiles, sessionId }: ModifiedFilesSectionProps) {
  const t = useT();
  const workdir = usePreviewWorkdir();

  const totals = useMemo(() => {
    const merged = new Map<string, { added: number; removed: number }>();
    for (const file of modifiedFiles) {
      const key = (workdir ? relativeToWorkdir(file.path, workdir) : undefined) ?? file.path;
      const entry = merged.get(key) ?? { added: 0, removed: 0 };
      entry.added += file.added;
      entry.removed += file.removed;
      merged.set(key, entry);
    }
    const added = Array.from(merged.values()).reduce((sum, entry) => sum + entry.added, 0);
    const removed = Array.from(merged.values()).reduce((sum, entry) => sum + entry.removed, 0);
    return { count: merged.size, added, removed };
  }, [modifiedFiles, workdir]);

  return (
    <Panel
      id="modifiedFiles"
      title={t('panelModifiedFiles')}
      icon={<FileIcon className="h-3.5 w-3.5" />}
      summary={totals.count > 0 ? totals.count : undefined}
    >
      {totals.count === 0 && <div className="text-xs text-muted">{t('noModifiedFiles')}</div>}
      {totals.count > 0 && sessionId && (
        <button
          type="button"
          onClick={() => {
            window.location.hash = changesPath(sessionId);
          }}
          className="group flex w-full items-center gap-2 rounded-md py-1.5 text-left text-xs hover:bg-accent-soft"
        >
          <span className="grow group-hover:text-accent-strong">{t('seeChanges')}</span>
          <span className="text-ok tabular-nums">+{totals.added}</span>
          <span className="text-err tabular-nums">−{totals.removed}</span>
          <ChevronRightIcon className="h-3.5 w-3.5 text-muted" />
        </button>
      )}
    </Panel>
  );
}
