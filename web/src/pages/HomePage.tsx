import { useState } from 'react';
import type { Project } from '../lib/api';
import { DeleteSessionsDialog, type DeletionTarget } from '../components/session/DeleteSessionsDialog';
import { useT } from '../i18n';
import { FolderIcon, SearchIcon } from '../components/ui/Icons';
import { Button } from '../components/ui/Button';
import { PageHeader } from '../components/ui/PageHeader';
import { WorktreeAlert } from '../components/project/WorktreeAlert';
import { SetupChecklist } from '../components/project/SetupChecklist';
import { TextInput } from '../components/ui/Field';
import { Segmented } from '../components/ui/Segmented';
import { ProjectTile } from '../components/project/ProjectTile';
import { ProjectSection } from '../components/project/ProjectSection';
import { ChatSection } from '../components/session/ChatSection';

interface HomePageProps {
  projects: Project[];
  loading: boolean;
  error: string | null;
  onOpenProjectDialog: () => void;
  onNewSession: (projectId: string) => void;
  onOpenSession: (sessionId: string) => void;
  onSessionsDeleted: (ids: string[]) => void;
  onToggleFavorite: (projectId: string, favorite: boolean) => void;
}

export function HomePage({
  projects,
  loading,
  error,
  onOpenProjectDialog,
  onNewSession,
  onOpenSession,
  onSessionsDeleted,
  onToggleFavorite
}: HomePageProps) {
  const t = useT();
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [pendingDelete, setPendingDelete] = useState<DeletionTarget[] | null>(null);
  const [query, setQuery] = useState('');
  const [view, setView] = useState<'tiles' | 'rows'>(() => {
    try {
      return localStorage.getItem('fennec.home.view') === 'rows' ? 'rows' : 'tiles';
    } catch {
      return 'tiles';
    }
  });
  const runningCount = projects.reduce(
    (count, project) => count + project.sessions.filter((session) => session.state === 'running').length,
    0
  );
  // The chat is a scratch project: it gets its own section, not a project one.
  const chatProject = projects.find((project) => project.chat);
  const regularProjects = projects.filter((project) => !project.chat);
  const needle = query.toLowerCase();
  const filteredProjects = needle
    ? regularProjects.filter((project) =>
        [project.name, project.workdir, project.branch ?? '']
          .some((value) => value.toLowerCase().includes(needle))
      )
    : regularProjects;

  const toggleSession = (id: string) => {
    setSelected((previous) => {
      const next = new Set(previous);
      if (next.has(id)) {
        next.delete(id);
      } else {
        next.add(id);
      }
      return next;
    });
  };

  const remove = (ids: string[]) => {
    const targets = projects.flatMap((project) =>
      project.sessions
        .filter((session) => ids.includes(session.id))
        .map((session) => ({ session, projectName: project.name }))
    );
    if (targets.length > 0) setPendingDelete(targets);
  };

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

        <div className="flex items-center gap-3">
          <div className="relative min-w-0 grow">
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
          <Segmented<'tiles' | 'rows'>
            label={t('homeDisplayProjects')}
            segments={[
              { value: 'tiles', label: t('homeTiles') },
              { value: 'rows', label: t('homeList') }
            ]}
            value={view}
            onChange={(value) => {
              setView(value);
              // A blocked storage only loses the choice on reload.
              try {
                localStorage.setItem('fennec.home.view', value);
              } catch {}
            }}
          />
        </div>

        {selected.size > 0 && (
          <div className="sticky top-0 z-10 flex items-center gap-3 rounded-md border border-border bg-raised px-3 py-2">
            {t('homeSelectedSessions', { count: selected.size })}
            <Button variant="danger" size="sm" onClick={() => remove([...selected])}>
              {t('homeDeleteSelected', { count: selected.size })}
            </Button>
            <Button variant="ghost" size="sm" onClick={() => setSelected(new Set())}>
              {t('cancel')}
            </Button>
          </div>
        )}

        <SetupChecklist projectCount={regularProjects.length} onOpenProject={onOpenProjectDialog} />

        {error && <p className="m-0 text-err">{error}</p>}

        {loading && projects.length === 0 && <div className="text-sm text-muted">{t('loading')}</div>}

        {!loading && !error && regularProjects.length === 0 && (
          <p className="m-0 py-10 text-center text-muted">
            {t('homeNoProjects')}
          </p>
        )}

        {chatProject && (
          <ChatSection
            project={chatProject}
            onNewSession={() => onNewSession(chatProject.id)}
            onOpenSession={onOpenSession}
            selected={selected}
            selecting={selected.size > 0}
            onToggle={toggleSession}
            onDelete={(id) => remove([id])}
          />
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
              onToggle={toggleSession}
              onDelete={(id) => remove([id])}
            />
          ))
        )}
      </div>

      {pendingDelete && (
        <DeleteSessionsDialog
          targets={pendingDelete}
          onClose={() => setPendingDelete(null)}
          onDeleted={(ids) => {
            onSessionsDeleted(ids);
            setSelected((previous) => {
              const next = new Set(previous);
              for (const id of ids) next.delete(id);
              return next;
            });
          }}
        />
      )}
    </main>
  );
}
