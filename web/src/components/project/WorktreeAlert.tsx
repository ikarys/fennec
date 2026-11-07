import { buttonClass } from '../ui/Button';
import { HardDriveIcon } from '../Icons';
import { getWorktreeSummary } from '../../lib/api/worktrees';
import { formatBytes } from '../../lib/format';
import { useT, useLang } from '../../i18n';
import { useLoaded } from '../../hooks/useLoaded';

export function WorktreeAlert() {
  const lang = useLang();
  const t = useT();

  // The summary is a hint: a failure shows nothing.
  const { data: summary } = useLoaded(getWorktreeSummary, []);

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
