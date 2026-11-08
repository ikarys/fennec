import type { ModifiedFiles } from '../../lib/api';
import { ChevronRightIcon, FileIcon } from '../ui/Icons';
import { Panel } from '../ui/Panel';
import { useT } from '../../i18n';
import { changesPath } from '../../lib/route';

interface ModifiedFilesSectionProps {
  totals: ModifiedFiles;
  sessionId: string | null;
}

export function ModifiedFilesSection({ totals, sessionId }: ModifiedFilesSectionProps) {
  const t = useT();
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
