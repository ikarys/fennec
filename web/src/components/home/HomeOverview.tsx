// The Home overview section: recent sessions, favorite projects, and the remaining stat tiles.
import type { ReactNode } from 'react';
import { useT } from '../../i18n';
import type { Lang } from '../../i18n/messages';
import type { HomeView, InsightView } from '../../lib/api';
import { formatRelative } from '../../lib/format';
import { ChatIcon, PlusIcon, StarIcon } from '../ui/Icons';
import { Button } from '../ui/Button';
import { ListCard, listRowClass } from '../ui/ListCard';
import { Badge } from '../ui/Badge';
import { SessionStateIcon } from '../session/SessionStateIcon';
import { InsightWeekCard } from './InsightWeekCard';


export interface HomeStatTile {
  key: string;
  href: string;
  icon: ReactNode;
  value: string;
  label: string;
  alert: boolean;
}

interface HomeOverviewProps {
  home: { data: HomeView | null; loading: boolean; error: string | null };
  insight: InsightView | null;
  stats: HomeStatTile[];
  lang: Lang;
  onOpenSession: (sessionId: string) => void;
  onNewSession: (projectId: string) => void;
}

export function HomeOverview({
  home,
  insight,
  stats,
  lang,
  onOpenSession,
  onNewSession
}: HomeOverviewProps) {
  const t = useT();

  return (
    <>
      <InsightWeekCard insight={insight} />

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
                    className={listRowClass({}, 'w-full text-left border-t border-border first:border-t-0')}
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
                  className={listRowClass({}, 'border-t border-border first:border-t-0')}
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
    </>
  );
}
