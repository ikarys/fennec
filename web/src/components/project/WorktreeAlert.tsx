import { useEffect, useState } from 'react';
import { buttonClass } from '../ui/Button';
import { HardDriveIcon } from '../Icons';
import { getWorktreeSummary, type WorktreeSummary } from '../../lib/api/worktrees';
import { formatBytes } from '../../lib/format';
import { useT, useLang } from '../../i18n';

export function WorktreeAlert() {
  const lang = useLang();
  const t = useT();
  const [summary, setSummary] = useState<WorktreeSummary | null>(null);

  useEffect(() => {
    let cancelled = false;

    getWorktreeSummary()
      .then((result) => {
        if (!cancelled) setSummary(result);
      })
      .catch(() => {
        // The summary is a hint; ignore failures.
      });

    return () => {
      cancelled = true;
    };
  }, []);

  const largestProjectId = summary?.largestProjectId ?? null;
  if (!summary?.alert || !largestProjectId) return null;

  return (
    <a
      className={buttonClass({ variant: 'ghost', size: 'sm' })}
      title={t('worktreeCacheHint', { bytes: formatBytes(lang, summary.generatedBytes) })}
      href={'#/project/' + encodeURIComponent(largestProjectId)}
    >
      <HardDriveIcon className="h-3.5 w-3.5" />
      {t('worktreesLabel')}: {formatBytes(lang, summary.totalBytes)}
    </a>
  );
}
