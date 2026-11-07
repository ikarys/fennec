// Project page overview tab: the server's dashboard tiles and recent sessions.
import { type ReactNode } from 'react';
import { Badge } from '../ui/Badge';
import { Button } from '../ui/Button';
import { Card } from '../ui/Card';
import { Section } from '../ui/Section';
import { Tile } from '../ui/Tile';
import {
  ClockIcon,
  CoinIcon,
  FileIcon,
  LayersIcon,
  ProjectIcon,
  ReviewIcon,
  TokensIcon,
} from '../Icons';
import { getProjectDashboard, type DashboardIcon } from '../../lib/api';
import { formatDateTime, formatRelative } from '../../lib/format';
import { BudgetTile } from './BudgetTile';
import { DiskSection } from './DiskSection';
import { ProjectRulesTile } from '../rules/RulesSection';
import { PluginsSection } from './PluginsSection';
import { useT, useLang } from '../../i18n';
import { TableHeader } from '../ui/TableHeader';
import { useLoaded } from '../../hooks/useLoaded';

const ICONS: Record<DashboardIcon, ReactNode> = {
  sessions: <LayersIcon />,
  ideas: <ReviewIcon />,
  tokens: <TokensIcon />,
  time: <ClockIcon />,
  cost: <CoinIcon />,
  map: <ProjectIcon />,
  doc: <FileIcon />,
};

export function DashboardSection({
  projectId,
  onOpenSession,
}: {
  projectId: string;
  onOpenSession: (sessionId: string) => void;
}) {
  const lang = useLang();
  const t = useT();

  const { data, error, loading } = useLoaded(() => getProjectDashboard(projectId), [projectId]);

  if (loading) return <p className="text-muted">{t('loading')}</p>;
  if (error) return <p className="text-err">{error}</p>;
  if (!data) return <p className="text-muted">{t('loading')}</p>;

  return (
    <div className="flex flex-col gap-4">
      <div className="grid grid-cols-2 gap-3 lg:grid-cols-4">
        {data.tiles.map((tile) => {
          const href = tile.href;
          const handleClick = href ? () => {
            window.location.hash = href;
          } : undefined;
          return (
            <Tile
              key={tile.id}
              icon={ICONS[tile.icon]}
              figure={tile.figure}
              label={tile.label}
              detail={tile.detail}
              tone={tile.tone}
              active={tile.active}
              title={tile.title ?? undefined}
              onClick={handleClick}
            />
          );
        })}
        <ProjectRulesTile projectId={projectId} />
        <BudgetTile projectId={projectId} />
      </div>

      <PluginsSection projectId={projectId} />

      <Section
        title={t('recentSessions')}
        flush
        actions={
          <Button
            variant="ghost"
            size="sm"
            onClick={() => {
              window.location.hash = '#/project/' + encodeURIComponent(projectId) + '/sessions';
            }}
          >
            {t('allSessions')}
          </Button>
        }
      >
        {data.recent.length === 0 ? (
          <Card className="px-4 py-6 text-center text-sm text-muted">{t('noSessions')}</Card>
        ) : (
          <Card className="overflow-hidden">
            <TableHeader>
              <span className="min-w-0 grow">{t('titleCol')}</span>
              <span className="w-28 shrink-0">{t('statusCol')}</span>
              <span className="w-24 shrink-0 text-right">{t('modifiedCol')}</span>
            </TableHeader>
            <ul className="divide-y divide-border">
              {data.recent.map((session) => (
                <li key={session.id}>
                  <button
                    type="button"
                    className="w-full text-left group relative flex cursor-pointer items-center gap-3 px-4 py-2.5 transition-colors before:absolute before:inset-y-0 before:left-0 before:w-0.5 before:bg-accent before:transition-opacity hover:bg-accent-soft hover:before:opacity-100 before:opacity-0"
                    onClick={() => onOpenSession(session.id)}
                  >
                    <span
                      className="min-w-0 grow truncate text-sm text-text group-hover:text-accent-strong"
                      title={session.title}
                    >
                      {session.title}
                    </span>
                    <span className="w-28 shrink-0">
                      {session.running ? (
                        <Badge tone="accent" dot pulse>{t('running')}</Badge>
                      ) : session.awaiting ? (
                        <Badge tone="accent" appearance="solid">{t('toIntegrateBadge')}</Badge>
                      ) : null}
                    </span>
                    <span
                      className="w-24 shrink-0 text-right font-mono text-xs text-faint tabular-nums"
                      title={formatDateTime(lang, session.updatedAt)}
                    >
                      {formatRelative(lang, session.updatedAt)}
                    </span>
                  </button>
                </li>
              ))}
            </ul>
          </Card>
        )}
      </Section>

      <DiskSection projectId={projectId} onOpenSession={onOpenSession} />
    </div>
  );
}
