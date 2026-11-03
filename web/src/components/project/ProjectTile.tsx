/**
 * A project as a tile on the home page: the whole tile links to its project page;
 * its sessions are on that page and in the list view.
 */
import type { Project } from '../../lib/api';
import { BulbIcon, PlusIcon } from '../Icons';
import { FavoriteButton } from './FavoriteButton';
import { Badge } from '../ui/Badge';
import { Button, buttonClass } from '../ui/Button';

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

  return (
    <article className="group relative flex flex-col gap-2 rounded-xl border border-border bg-panel p-4 transition-colors hover:border-accent hover:bg-accent-soft">
      <div className="flex min-w-0 items-center gap-2">
        <a
          href={'#/project/' + encodeURIComponent(project.id)}
          title="Voir le projet : tâches, carte des modules, doc"
          className="truncate text-sm font-semibold text-text after:absolute after:inset-0 after:content-[''] after:rounded-xl"
        >
          {project.name}
        </a>
        {runningCount > 0 && (
          <Badge tone="accent" solid dot pulse>
            {runningCount} en cours
          </Badge>
        )}
        {project.sessionWorktree && (
          <Badge
            tone="err"
            title="Ce dossier est l'espace de travail d'une session de fennec : une intégration ou un nettoyage peut le supprimer."
          >
            worktree d'une session
          </Badge>
        )}
      </div>
      <div className="truncate font-mono text-xs text-faint" title={project.workdir}>
        {project.workdir}
      </div>
      <div className="mt-auto flex items-center gap-3 pt-1 font-mono text-xs text-muted">
        <span className="min-w-0 truncate">⎇ {project.branch ?? 'sans branche'}</span>
        <span className="shrink-0 tabular-nums">
          {project.sessions.length} session{project.sessions.length > 1 ? 's' : ''}
        </span>
        <div className="relative z-10 ml-auto flex shrink-0 items-center gap-0.5">
          <FavoriteButton project={project} onToggle={onToggleFavorite} />
          <Button
            variant="ghost"
            size="sm"
            iconOnly
            title="Nouvelle session dans ce projet"
            aria-label="Nouvelle session"
            onClick={() => onNewSession(project.id)}
          >
            <PlusIcon className="h-3.5 w-3.5" />
          </Button>
          <a
            href={'#/project/' + encodeURIComponent(project.id) + '/tasks'}
            title="Voir les tâches du projet"
            aria-label="Tâches du projet"
            className={buttonClass({ variant: 'ghost', size: 'sm', iconOnly: true })}
          >
            <BulbIcon className="h-3.5 w-3.5" />
          </a>
        </div>
      </div>
    </article>
  );
}
