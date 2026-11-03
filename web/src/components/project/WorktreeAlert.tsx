import { useEffect, useState } from 'react';
import { buttonClass } from '../ui/Button';
import { HardDriveIcon } from '../Icons';
import { getWorktreeSummary, type WorktreeSummary } from '../../lib/api/worktrees';
import { formatBytes } from '../../lib/format';

export function WorktreeAlert() {
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
      title={`dont ${formatBytes(summary.generatedBytes)} de cache de build — voir l'espace disque du projet`}
      href={'#/project/' + encodeURIComponent(largestProjectId)}
    >
      <HardDriveIcon className="h-3.5 w-3.5" />
      Worktrees : {formatBytes(summary.totalBytes)}
    </a>
  );
}
