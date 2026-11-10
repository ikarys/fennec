// The Chats page: the scratch project's conversations, with no folder or git.
import type { Project } from '../lib/api';
import { useT } from '../i18n';
import { ChatIcon, PlusIcon } from '../components/ui/Icons';
import { Button } from '../components/ui/Button';
import { PageHeader } from '../components/ui/PageHeader';
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

        {chatProject && (
          <ChatSection
            project={chatProject}
            onOpenSession={onOpenSession}
            selected={selected}
            selecting={selected.size > 0}
            onToggle={toggle}
            onDelete={(id) => remove([id])}
          />
        )}
      </div>

      {dialog}
    </main>
  );
}
