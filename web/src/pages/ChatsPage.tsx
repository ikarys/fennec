// The Chats page: the scratch project's conversations, grouped by linked project and filterable.
import { useState } from 'react';
import type { Project, SessionSummary } from '../lib/api';
import { useT } from '../i18n';
import { ChatIcon, PlusIcon, SearchIcon } from '../components/ui/Icons';
import { Button } from '../components/ui/Button';
import { PageHeader } from '../components/ui/PageHeader';
import { TextInput } from '../components/ui/Field';
import { ChatSection } from '../components/session/ChatSection';
import { useSessionSelection } from '../hooks/useSessionSelection';

interface ChatsPageProps {
  projects: Project[];
  loading: boolean;
  error: string | null;
  onNewSession: (projectId: string) => void;
  onOpenSession: (id: string) => void;
  onSessionsDeleted: (ids: string[]) => void;
}

export function ChatsPage({
  projects,
  loading,
  error,
  onNewSession,
  onOpenSession,
  onSessionsDeleted
}: ChatsPageProps) {
  const t = useT();
  const { selected, toggle, clear, remove, dialog } = useSessionSelection(projects, onSessionsDeleted);
  const chatProject = projects.find((project) => project.chat);
  const [query, setQuery] = useState('');

  const groups = chatProject?.chatGroups ?? [];
  const needle = query.trim().toLowerCase();
  const visibleGroups = groups
    .map((group) => {
      const sessions = group.sessionIds
        .map((id) => chatProject?.sessions.find((session) => session.id === id))
        .filter((session): session is SessionSummary => session !== undefined);
      if (needle) {
        return {
          group,
          sessions: sessions.filter(
            (session) =>
              session.title.toLowerCase().includes(needle) ||
              (group.projectName ?? '').toLowerCase().includes(needle)
          ),
        };
      }
      return { group, sessions };
    })
    .filter((entry) => entry.sessions.length > 0);

  return (
    <main className="grow overflow-y-auto px-4 py-6 md:px-6 md:py-8">
      <div className="mx-auto flex max-w-page flex-col gap-5">
        <PageHeader
          Icon={ChatIcon}
          title={t('homeChat')}
          subtitle={t('chatsPageSubtitle')}
          actions={
            chatProject ? (
              <Button variant="primary" size="md" onClick={() => onNewSession(chatProject.id)}>
                <PlusIcon className="h-[15px] w-[15px]" />
                {t('paletteNewChat')}
              </Button>
            ) : null
          }
        />

        {chatProject && (
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
                placeholder={t('chatsFilter')}
                aria-label={t('chatsFilter')}
              />
            </div>
          </div>
        )}

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

        {error && <p className="m-0 text-err">{error}</p>}

        {loading && projects.length === 0 && <div className="text-sm text-muted">{t('loading')}</div>}

        {chatProject && chatProject.sessions.length === 0 ? (
          <ChatSection
            title={t('homeChat')}
            sessions={[]}
            onOpenSession={onOpenSession}
            selected={selected}
            selecting={selected.size > 0}
            onToggle={toggle}
            onDelete={(id) => remove([id])}
          />
        ) : (
          <>
            {visibleGroups.map(({ group, sessions }) => (
              <ChatSection
                key={group.projectId ?? 'unlinked'}
                title={group.projectName ?? t('chatsUnlinked')}
                sessions={sessions}
                onOpenSession={onOpenSession}
                selected={selected}
                selecting={selected.size > 0}
                onToggle={toggle}
                onDelete={(id) => remove([id])}
              />
            ))}
            {chatProject && query.trim() !== '' && visibleGroups.length === 0 && (
              <p className="m-0 py-6 text-center text-sm text-muted">{t('chatsNoMatch', { query })}</p>
            )}
          </>
        )}
      </div>

      {dialog}
    </main>
  );
}
