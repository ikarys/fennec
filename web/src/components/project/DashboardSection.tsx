// Project page overview tab: the server's dashboard tiles and recent sessions.
import { useEffect, useState, type ReactNode } from 'react';
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
import { getProjectDashboard, type DashboardIcon, type ProjectDashboard } from '../../lib/api';
import { formatDateTime, formatRelative } from '../../lib/format';
import { DiskSection } from './DiskSection';
import { ProjectRulesTile } from '../rules/RulesSection';
import { useLang } from '../../i18n';

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
  const [data, setData] = useState<ProjectDashboard | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setError(null);
    setData(null);
    getProjectDashboard(projectId)
      .then((dashboard) => {
        if (!cancelled) {
          setData(dashboard);
          setLoading(false);
        }
      })
      .catch((err: unknown) => {
        if (!cancelled) {
          setError(err instanceof Error ? err.message : 'Erreur inconnue');
          setLoading(false);
        }
      });
    return () => {
      cancelled = true;
    };
  }, [projectId]);

  if (loading) return <p className="text-muted">Chargement…</p>;
  if (error) return <p className="text-err">{error}</p>;
  if (!data) return <p className="text-muted">Chargement…</p>;

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
      </div>

      <Section
        title="Sessions récentes"
        flush
        actions={
          <Button
            variant="ghost"
            size="sm"
            onClick={() => {
              window.location.hash = '#/project/' + encodeURIComponent(projectId) + '/sessions';
            }}
          >
            Toutes les sessions
          </Button>
        }
      >
        {data.recent.length === 0 ? (
          <Card className="px-4 py-6 text-center text-sm text-muted">Aucune session.</Card>
        ) : (
          <Card className="overflow-hidden">
            <div className="flex items-center gap-3 border-b border-border bg-raised px-4 py-2 text-xs uppercase tracking-wide text-faint">
              <span className="min-w-0 grow">Titre</span>
              <span className="w-28 shrink-0">Statut</span>
              <span className="w-24 shrink-0 text-right">Modifié</span>
            </div>
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
                        <Badge tone="accent" dot pulse>en cours</Badge>
                      ) : session.awaiting ? (
                        <Badge tone="accent" solid>à intégrer</Badge>
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
