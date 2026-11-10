// The home page: a welcoming dashboard of what's running, what needs the human,
// recent sessions, favorite projects and quick stats.
import type { ReactNode } from 'react';
import { useT, useLang } from '../i18n';
import { getHome, getBacklog, getInsight, getWorktreeSummary, type AttentionItem } from '../lib/api';
import { useLoaded } from '../hooks/useLoaded';
import { formatRelative, formatBytes } from '../lib/format';
import { ActivityIcon, PlusIcon, FolderIcon, CheckIcon, CoinIcon, BacklogIcon, HardDriveIcon, InboxIcon, ChatIcon, StarIcon } from '../components/ui/Icons';
import { Button } from '../components/ui/Button';
import { PageHeader } from '../components/ui/PageHeader';
import { ListCard } from '../components/ui/ListCard';
import { Badge } from '../components/ui/Badge';
import { SetupChecklist } from '../components/project/SetupChecklist';
import { SessionStateIcon } from '../components/session/SessionStateIcon';
import { attentionKindLabel } from '../components/layout/AttentionBell';

interface HomePageProps {
  projectCount: number;
  attention: AttentionItem[];
  refreshKey: number;
  onOpenProjectDialog: () => void;
  onNewChat: (() => void) | null;
  onNewSession: (projectId: string) => void;
  onOpenSession: (sessionId: string) => void;
}

export function HomePage({
  projectCount,
  attention,
  refreshKey,
  onOpenProjectDialog,
  onNewChat,
  onNewSession,
  onOpenSession
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

  // Each stat only renders when its data is loaded; a missing one is skipped.
  const stats: { key: string; href: string; icon: ReactNode; value: string; label: string; alert: boolean }[] = [];
  if (insight.data) {
    const costCard = insight.data.cards.find((card) => card.kind === 'cost');
    if (costCard) {
      stats.push({
        key: 'cost',
        href: '#/insight',
        icon: <CoinIcon className="h-4 w-4" />,
        value: costCard.value,
        label: t('homeStatCost'),
        alert: false
      });
    }
  }
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
                className="flex w-full items-center gap-3 px-4 py-2.5 text-left hover:bg-accent-soft border-t border-border first:border-t-0"
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

        {home.loading ? (
          <div className="text-sm text-muted">{t('loading')}</div>
        ) : home.error ? (
          <p className="m-0 text-err">{home.error}</p>
        ) : home.data ? (
          <div className="grid grid-cols-1 gap-5 lg:grid-cols-2">
            <ListCard Icon={ChatIcon} title={t('homeRecentSessions')} count={home.data.recent.length}>
              {home.data.recent.length === 0 ? (
                <div className="px-4 py-6 text-center text-sm text-muted">{t('homeNoRecentSessions')}</div>
              ) : (
                <>
                  {home.data.recent.map((s) => (
                    <button
                      key={s.id}
                      type="button"
                      onClick={() => onOpenSession(s.id)}
                      className="flex w-full items-center gap-3 px-4 py-2.5 text-left hover:bg-accent-soft border-t border-border first:border-t-0"
                    >
                      <SessionStateIcon state={s.state} className="h-4 w-4" />
                      <span className="min-w-0 grow truncate text-sm font-medium text-text">{s.title}</span>
                      <Badge tone={s.chat ? 'info' : 'neutral'} appearance="pill">
                        {s.chat ? t('homeChat') : s.projectName}
                      </Badge>
                      <span className="shrink-0 text-xs text-faint tabular-nums">{formatRelative(lang, s.updatedAt)}</span>
                    </button>
                  ))}
                  <div className="flex gap-4 border-t border-border px-4 py-2 text-xs">
                    <a href="#/projects" className="text-accent hover:text-accent-strong">
                      {t('homeAllProjects')}
                    </a>
                    <a href="#/chats" className="text-accent hover:text-accent-strong">
                      {t('homeAllChats')}
                    </a>
                  </div>
                </>
              )}
            </ListCard>

            <ListCard Icon={StarIcon} title={t('homeFavoriteProjects')} count={home.data.favorites.length}>
              {home.data.favorites.length === 0 ? (
                <div className="px-4 py-6 text-center text-sm text-muted">{t('homeNoFavorites')}</div>
              ) : (
                home.data.favorites.map((p) => (
                  <div
                    key={p.id}
                    className="flex items-center gap-3 px-4 py-2.5 border-t border-border first:border-t-0 hover:bg-accent-soft"
                  >
                    <a href={'#/project/' + encodeURIComponent(p.id)} className="min-w-0 grow">
                      <span className="block text-sm font-medium text-text truncate">{p.name}</span>
                      <span className="block font-mono text-xs text-faint truncate">{p.workdir}</span>
                    </a>
                    <Badge appearance="pill" tone={p.running > 0 ? 'accent' : 'neutral'}>
                      {t('homeSessionCount', { count: p.sessions })}
                    </Badge>
                    <Button
                      variant="ghost"
                      size="sm"
                      iconOnly
                      onClick={() => onNewSession(p.id)}
                      title={t('newSession')}
                      aria-label={t('newSession')}
                    >
                      <PlusIcon className="h-3.5 w-3.5" />
                    </Button>
                  </div>
                ))
              )}
            </ListCard>
          </div>
        ) : null}

        {stats.length > 0 && (
          <div className="grid grid-cols-1 gap-3 sm:grid-cols-3">
            {stats.map((stat) => (
              <a
                key={stat.key}
                href={stat.href}
                className={`flex items-center gap-3 rounded-xl border border-border bg-panel px-4 py-3 hover:border-accent/40 hover:bg-accent-soft ${
                  stat.alert ? 'border-attention/50' : ''
                }`}
              >
                <span className="flex h-9 w-9 shrink-0 items-center justify-center rounded-md bg-accent-soft text-accent">
                  {stat.icon}
                </span>
                <span className="min-w-0">
                  <span className={`block text-lg font-semibold tabular-nums ${stat.alert ? 'text-attention' : 'text-text'}`}>
                    {stat.value}
                  </span>
                  <span className="block text-xs text-muted">{stat.label}</span>
                </span>
              </a>
            ))}
          </div>
        )}
      </div>
    </main>
  );
}
