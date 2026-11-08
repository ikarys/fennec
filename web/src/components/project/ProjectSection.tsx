import { useState } from 'react';
import type { Project } from '../../lib/api';
import { useT } from '../../i18n';
import { BulbIcon, ChevronRightIcon, GlobeIcon, PlusIcon } from '../ui/Icons';
import { Button, buttonClass } from '../ui/Button';
import { Badge } from '../ui/Badge';
import { FavoriteButton } from './FavoriteButton';
import { SessionList } from '../session/SessionList';

export function ProjectSection({
  project,
  defaultOpen,
  onNewSession,
  onOpenSession,
  onToggleFavorite,
  selected,
  selecting,
  onToggle,
  onDelete
}: {
  project: Project;
  defaultOpen: boolean;
  onNewSession: (projectId: string) => void;
  onOpenSession: (sessionId: string) => void;
  onToggleFavorite: (projectId: string, favorite: boolean) => void;
  selected: Set<string>;
  selecting: boolean;
  onToggle: (id: string) => void;
  onDelete: (id: string) => void;
}) {
  const t = useT();
  const [open, setOpen] = useState(defaultOpen);
  // The expand state lives here, not in SessionList, so it survives collapsing
  // and reopening the section.
  const [expanded, setExpanded] = useState(false);
  // The live badge only appears while this project has running sessions.
  const runningCount = project.sessions.filter((session) => session.state === 'running').length;

  return (
    <section className="overflow-hidden rounded-md border border-border bg-panel">
      <div className="flex min-h-12 items-center gap-2.5 px-3 py-2">
        <button
          type="button"
          aria-expanded={open}
          onClick={() => setOpen((value) => !value)}
          className="flex min-w-0 grow items-center gap-2.5 text-left"
        >
          <ChevronRightIcon
            className={`h-3 w-3 shrink-0 text-muted transition-transform ${open ? 'rotate-90' : ''}`}
          />
          <span className="shrink-0 text-sm font-semibold">{project.name}</span>
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
          <span className="ml-1 flex min-w-0 gap-3 font-mono text-xs text-faint">
            <span className="truncate whitespace-nowrap">{project.workdir}</span>
            <span className="truncate whitespace-nowrap">⎇ {project.branch ?? t('noBranch')}</span>
            <span className="truncate whitespace-nowrap tabular-nums">
              {t('sessionCount', { count: project.sessions.length })}
            </span>
          </span>
        </button>
        <div className="ml-auto flex shrink-0 items-center gap-0.5">
          <FavoriteButton project={project} onToggle={onToggleFavorite} />
          <a
            href={'#/project/' + encodeURIComponent(project.id)}
            title={t('viewProject')}
            aria-label={t('homeViewProjectAria')}
            className={buttonClass({ variant: 'ghost', size: 'sm' })}
          >
            <GlobeIcon className="h-3.5 w-3.5" />
            {t('homeProject')}
          </a>
          <Button
            variant="ghost"
            size="sm"
            className="text-accent hover:bg-accent-soft hover:text-accent-strong"
            onClick={() => onNewSession(project.id)}
            title={t('newSessionInProject')}
            aria-label={t('newSession')}
          >
            <PlusIcon className="h-3.5 w-3.5" />
            {t('session')}
          </Button>
          <a
            href={'#/project/' + encodeURIComponent(project.id) + '/tasks'}
            title={t('viewProjectTasks')}
            aria-label={t('projectTasksAria')}
            className={buttonClass({ variant: 'ghost', size: 'sm' })}
          >
            <BulbIcon className="h-3.5 w-3.5" />
            {t('tasks')}
          </a>
        </div>
      </div>

      {open && (
        <SessionList
          sessions={project.sessions}
          selected={selected}
          selecting={selecting}
          onToggle={onToggle}
          onDelete={onDelete}
          onOpenSession={onOpenSession}
          empty={t('homeNoProjectSessions')}
          noun={t('homeSessionsNoun')}
          expanded={expanded}
          onExpandedChange={setExpanded}
        />
      )}
    </section>
  );
}

/** The chat: conversations with no project, folder or git. */
