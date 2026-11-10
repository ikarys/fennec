// The Projects page: the project tiles/list, the filter, the view choice and bulk delete.
import { useState } from 'react';
import type { Project } from '../lib/api';
import { useT } from '../i18n';
import { FolderIcon, SearchIcon } from '../components/ui/Icons';
import { Button } from '../components/ui/Button';
import { PageHeader } from '../components/ui/PageHeader';
import { Section } from '../components/ui/Section';
import { WorktreeAlert } from '../components/project/WorktreeAlert';
import { TextInput } from '../components/ui/Field';
import { Segmented } from '../components/ui/Segmented';
import { ProjectTile } from '../components/project/ProjectTile';
import { ProjectSection } from '../components/project/ProjectSection';
import { useSessionSelection } from '../hooks/useSessionSelection';
import { readStored, writeStored } from '../lib/storage';

interface ProjectsPageProps {
  projects: Project[];
  loading: boolean;
  error: string | null;
  onOpenProjectDialog: () => void;
  onNewSession: (projectId: string) => void;
  onOpenSession: (sessionId: string) => void;
  onSessionsDeleted: (ids: string[]) => void;
  onToggleFavorite: (projectId: string, favorite: boolean) => void;
}

export function ProjectsPage({
  projects,
  loading,
  error,
  onOpenProjectDialog,
  onNewSession,
  onOpenSession,
  onSessionsDeleted,
  onToggleFavorite
}: ProjectsPageProps) {
  const t = useT();
  const [query, setQuery] = useState('');
  const [view, setView] = useState<'tiles' | 'rows'>(() => {
    return readStored('fennec.home.view') === 'rows' ? 'rows' : 'tiles';
  });
  const { selected, toggle, clear, remove, dialog } = useSessionSelection(projects, onSessionsDeleted);
  const runningCount = projects.reduce(
    (count, project) => count + project.sessions.filter((session) => session.state === 'running').length,
    0
  );
  // Chats have their own page now: only the regular projects show here.
  const regularProjects = projects.filter((project) => !project.chat);
  const needle = query.toLowerCase();
  const filteredProjects = needle
    ? regularProjects.filter((project) =>
        [project.name, project.workdir, project.branch ?? '']
          .some((value) => value.toLowerCase().includes(needle))
      )
    : regularProjects;

  return (
    <main className="grow overflow-y-auto px-4 py-6 md:px-6 md:py-8">
      <div className="mx-auto flex max-w-page flex-col gap-5">
        <PageHeader
          Icon={FolderIcon}
          title={t('paletteProjects')}
          subtitle={
            <span className="flex items-center gap-2">
              {runningCount > 0 && <span className="h-2 w-2 animate-pulse rounded-full bg-accent" />}
              {runningCount > 0
                ? t('homeRunningSessions', { count: runningCount })
                : t('homeNoRunningSessions')}
            </span>
          }
          actions={
            <Button variant="primary" size="md" onClick={onOpenProjectDialog}>
              <FolderIcon className="h-[15px] w-[15px]" />
              {t('openProject')}
            </Button>
          }
        />
        <WorktreeAlert />

        {selected.size > 0 && (
          <div className="sticky top-0 z-10 flex items-center gap-3 rounded-md border border-border bg-raised px-3 py-2">
            {t('homeSelectedSessions', { count: selected.size })}
            <Button variant="danger" size="sm" onClick={() => remove([...selected])}>
              {t('homeDeleteSelected', { count: selected.size })}
            </Button>
            <Button variant="ghost" size="sm" onClick={clear}>
              {t('cancel')}
            </Button>
          </div>
        )}

        <Section
          Icon={FolderIcon}
          title={t('homeProjectsTitle')}
          description={t('homeProjectsCount', { count: regularProjects.length })}
          actions={
            <Segmented<'tiles' | 'rows'>
              label={t('homeDisplayProjects')}
              segments={[
                { value: 'tiles', label: t('homeTiles') },
                { value: 'rows', label: t('homeList') }
              ]}
              value={view}
              onChange={(value) => {
                setView(value);
                writeStored('fennec.home.view', value);
              }}
            />
          }
        >
          <div className="relative">
            <SearchIcon className="pointer-events-none absolute left-2.5 top-1/2 h-3.5 w-3.5 -translate-y-1/2 text-faint" />
            <TextInput
              className="pl-8"
              value={query}
              onChange={(event) => setQuery(event.target.value)}
              onKeyDown={(event) => {
                if (event.key === 'Escape') setQuery('');
              }}
              placeholder={t('homeFilterProjects')}
              aria-label={t('homeFilterProjects')}
            />
          </div>
          {error && <p className="m-0 text-err">{error}</p>}

          {loading && projects.length === 0 && <div className="text-sm text-muted">{t('loading')}</div>}

          {!loading && !error && regularProjects.length === 0 && (
            <p className="m-0 py-10 text-center text-muted">
              {t('homeNoProjects')}
            </p>
          )}

          {needle && regularProjects.length > 0 && filteredProjects.length === 0 && (
            <p className="m-0 py-6 text-center text-sm text-muted">{t('homeNoMatchingProjects', { query })}</p>
          )}

          {view === 'tiles' ? (
            <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3">
              {filteredProjects.map((project) => (
                <ProjectTile key={project.id} project={project} onNewSession={onNewSession} onToggleFavorite={onToggleFavorite} />
              ))}
            </div>
          ) : (
            // The server lists favourites first, then the most recently active: the first one opens unfolded.
            filteredProjects.map((project, index) => (
              <ProjectSection
                key={project.id}
                project={project}
                defaultOpen={index === 0}
                onNewSession={onNewSession}
                onOpenSession={onOpenSession}
                onToggleFavorite={onToggleFavorite}
                selected={selected}
                selecting={selected.size > 0}
                onToggle={toggle}
                onDelete={(id) => remove([id])}
              />
            ))
          )}
        </Section>
      </div>

      {dialog}
    </main>
  );
}
