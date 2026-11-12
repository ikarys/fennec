/**
 * A project as a tile on the home page: the whole tile links to its project page;
 * its sessions are on that page and in the list view.
 */
import type { Project } from '../../lib/api';
import { BulbIcon, FolderIcon, PlusIcon } from '../ui/Icons';
import { FavoriteButton } from './FavoriteButton';
import { Badge } from '../ui/Badge';
import { Button, buttonClass } from '../ui/Button';
import { useT } from '../../i18n';

export function ProjectTile({
  project,
  onNewSession,
  onToggleFavorite
}: {
  project: Project;
  onNewSession: (projectId: string) => void;
  onToggleFavorite: (projectId: string, favorite: boolean) => void;
}) {
  const runningCount = project.sessions.filter((session) => session.state === 'running').length;
  const t = useT();

  return (
    <article className="group relative flex flex-col gap-2 rounded-xl border border-border bg-panel p-4 transition-colors hover:border-accent hover:bg-accent-soft">
      {/* Only favourites are tinted: the colour points them out instead of painting every tile. */}
      <div
        className={`-mx-4 -mt-4 mb-1 flex min-w-0 items-center gap-2.5 rounded-t-xl border-b border-border px-4 py-2 ${
          project.favorite ? 'bg-linear-to-r from-accent-soft to-transparent to-75%' : ''
        }`}
      >
        <span
          className={`grid size-6 shrink-0 place-items-center rounded-md ${
            project.favorite ? 'bg-accent-soft text-accent' : 'bg-raised text-muted group-hover:text-accent'
          }`}
        >
          <FolderIcon className="h-3.5 w-3.5" />
        </span>
        <a
          href={'#/project/' + encodeURIComponent(project.id)}
          title={t('viewProject')}
          className="truncate text-sm font-semibold text-text after:absolute after:inset-0 after:content-[''] after:rounded-xl"
        >
          {project.name}
        </a>
        {runningCount > 0 && (
          <Badge tone="accent" appearance="solid" dot pulse>
            {t('runningCount', { count: runningCount })}
          </Badge>
        )}
        {project.sessionWorktree && (
          <Badge tone="err" title={t('worktreeBadgeTitle')}>
            {t('sessionWorktreeBadge')}
          </Badge>
        )}
        <div className="relative z-10 ml-auto shrink-0">
          <FavoriteButton project={project} onToggle={onToggleFavorite} />
        </div>
      </div>
      <div className="truncate font-mono text-xs text-faint" title={project.workdir}>
        {project.workdir}
      </div>
      <div className="mt-auto flex items-center gap-3 pt-1 font-mono text-xs text-muted">
        <span className="min-w-0 truncate">⎇ {project.branch ?? t('noBranch')}</span>
        <span className="shrink-0 tabular-nums">
          {t('sessionCount', { count: project.sessions.length })}
        </span>
        <div className="relative z-10 ml-auto flex shrink-0 items-center gap-0.5">
          <Button
            variant="icon"
            size="sm"
            iconOnly
            title={t('newSessionInProject')}
            aria-label={t('newSession')}
            onClick={() => onNewSession(project.id)}
          >
            <PlusIcon className="h-3.5 w-3.5" />
          </Button>
          <a
            href={'#/project/' + encodeURIComponent(project.id) + '/tasks'}
            title={t('viewProjectTasks')}
            aria-label={t('projectTasksAria')}
            className={buttonClass({ variant: 'icon', size: 'sm', iconOnly: true })}
          >
            <BulbIcon className="h-3.5 w-3.5" />
          </a>
        </div>
      </div>
    </article>
  );
}
