// The home page: the shared header, setup checklist, attention list, and the Home filter sections.
import { useT, useLang } from '../i18n';
import { getHome, getBacklog, getInsight, getWorktreeSummary, type AttentionItem, type Project } from '../lib/api';
import { useLoaded } from '../hooks/useLoaded';
import { formatBytes } from '../lib/format';
import { HOME_FILTERS, type HomeFilter } from '../lib/route';
import { ActivityIcon, PlusIcon, FolderIcon, CheckIcon, BacklogIcon, HardDriveIcon, InboxIcon } from '../components/ui/Icons';
import { Button } from '../components/ui/Button';
import { PageHeader } from '../components/ui/PageHeader';
import { ListCard, listRowClass } from '../components/ui/ListCard';
import { Badge } from '../components/ui/Badge';
import { Tabs } from '../components/ui/Tabs';
import { SetupChecklist } from '../components/project/SetupChecklist';
import { attentionKindLabel } from '../components/layout/AttentionBell';
import { HomeOverview, type HomeStatTile } from '../components/home/HomeOverview';
import { ProjectsSection } from '../components/home/ProjectsSection';
import { ChatsSection } from '../components/home/ChatsSection';
import { NightQueueSection } from '../components/home/NightQueueSection';

interface HomePageProps {
  filter: HomeFilter;
  projectCount: number;
  attention: AttentionItem[];
  refreshKey: number;
  projects: Project[];
  projectsLoading: boolean;
  projectsError: string | null;
  onOpenProjectDialog: () => void;
  onNewChat: (() => void) | null;
  onNewSession: (projectId: string) => void;
  onOpenSession: (sessionId: string) => void;
  onSessionsDeleted: (ids: string[]) => void;
  onToggleFavorite: (projectId: string, favorite: boolean) => void;
  onFilter: (filter: HomeFilter) => void;
}

export function HomePage({
  filter,
  projectCount,
  attention,
  refreshKey,
  projects,
  projectsLoading,
  projectsError,
  onOpenProjectDialog,
  onNewChat,
  onNewSession,
  onOpenSession,
  onSessionsDeleted,
  onToggleFavorite,
  onFilter
}: HomePageProps) {
  const t = useT();
  const lang = useLang();
  const home = useLoaded(getHome, [refreshKey], 'home');
  const backlog = useLoaded(getBacklog, [], 'backlog');
  const insight = useLoaded(() => getInsight('7', null), [], 'insight:7:');
  const worktrees = useLoaded(getWorktreeSummary, [], 'worktrees');

  // A greeting by local hour: morning, afternoon, evening.
  const hour = new Date().getHours();
  const greeting =
    hour >= 5 && hour < 12
      ? t('homeGreetingMorning')
      : hour >= 12 && hour < 18
        ? t('homeGreetingAfternoon')
        : t('homeGreetingEvening');

  const running = home.data?.running ?? 0;

  // The overview's stat tiles: the ready queue and worktree usage.
  const stats: HomeStatTile[] = [];
  if (backlog.data) {
    stats.push({
      key: 'ready',
      href: '#/backlog',
      icon: <BacklogIcon className="h-4 w-4" />,
      value: String(backlog.data.total.ready),
      label: t('homeStatReady'),
      alert: false
    });
  }
  if (worktrees.data) {
    const summary = worktrees.data;
    stats.push({
      key: 'worktrees',
      href: summary.largestProjectId ? '#/project/' + encodeURIComponent(summary.largestProjectId) : '#/projects',
      icon: <HardDriveIcon className="h-4 w-4" />,
      value: formatBytes(lang, summary.totalBytes),
      label: t('homeStatWorktrees'),
      alert: summary.alert
    });
  }

  return (
    <main className="grow overflow-y-auto px-4 py-6 md:px-6 md:py-8">
      <div className="mx-auto flex max-w-page flex-col gap-5">
        <PageHeader
          Icon={ActivityIcon}
          title={greeting}
          subtitle={
            <span className="flex items-center gap-2">
              {running > 0 && <span className="h-2 w-2 animate-pulse rounded-full bg-accent" />}
              {running > 0 ? t('homeRunningSessions', { count: running }) : t('homeNoRunningSessions')}
            </span>
          }
          actions={
            <>
              {onNewChat ? (
                <Button variant="soft" size="md" onClick={onNewChat}>
                  <PlusIcon className="h-[15px] w-[15px]" />
                  {t('paletteNewChat')}
                </Button>
              ) : null}
              <Button variant="primary" size="md" onClick={onOpenProjectDialog}>
                <FolderIcon className="h-[15px] w-[15px]" />
                {t('openProject')}
              </Button>
            </>
          }
        />

        <SetupChecklist projectCount={projectCount} onOpenProject={onOpenProjectDialog} />

        <ListCard Icon={InboxIcon} title={t('homeYourTurn')} description={t('homeYourTurnDescription')} count={attention.length}>
          {attention.length === 0 ? (
            <div className="px-4 py-6 text-center text-sm text-muted">
              <CheckIcon className="mr-1 inline h-4 w-4 text-ok" />
              {t('homeYourTurnEmpty')}
            </div>
          ) : (
            attention.map((item) => (
              <button
                key={item.sessionId}
                type="button"
                onClick={() => onOpenSession(item.sessionId)}
                className={listRowClass({}, 'w-full text-left border-t border-border first:border-t-0')}
              >
                <Badge tone={item.kind === 'error' ? 'err' : 'attention'} dot>
                  {attentionKindLabel(item.kind, t)}
                </Badge>
                <span className="min-w-0 grow">
                  <span className="block text-sm font-medium text-text truncate">{item.title}</span>
                  {item.detail ? <span className="block text-xs text-muted truncate">{item.detail}</span> : null}
                </span>
              </button>
            ))
          )}
        </ListCard>

        <Tabs<HomeFilter>
          orientation="horizontal"
          label={t('homeFilterLabel')}
          tabs={HOME_FILTERS.map((entry) => ({ id: entry.filter, label: t(entry.titleKey) }))}
          active={filter}
          onSelect={onFilter}
        />

        {filter === 'overview' ? (
          <HomeOverview
            home={home}
            insight={insight.data}
            stats={stats}
            lang={lang}
            onOpenSession={onOpenSession}
            onNewSession={onNewSession}
          />
        ) : filter === 'projects' ? (
          <ProjectsSection
            projects={projects}
            loading={projectsLoading}
            error={projectsError}
            onNewSession={onNewSession}
            onOpenSession={onOpenSession}
            onSessionsDeleted={onSessionsDeleted}
            onToggleFavorite={onToggleFavorite}
          />
        ) : filter === 'chats' ? (
          <ChatsSection
            projects={projects}
            loading={projectsLoading}
            error={projectsError}
            onOpenSession={onOpenSession}
            onSessionsDeleted={onSessionsDeleted}
          />
        ) : (
          <NightQueueSection />
        )}
      </div>
    </main>
  );
}
