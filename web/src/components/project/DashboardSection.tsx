// Project page overview tab: the server's dashboard tiles and recent sessions.
import { useEffect, useState, type ReactNode } from 'react';
import { Badge } from '../ui/Badge';
import { Section } from '../ui/Section';
import { Tile } from '../ui/Tile';
import {
  ClockIcon,
  CoinIcon,
  FileIcon,
  HardDriveIcon,
  LayersIcon,
  ProjectIcon,
  ReviewIcon,
  TokensIcon,
} from '../Icons';
import { getProjectDashboard, type DashboardIcon, type ProjectDashboard } from '../../lib/api';
import { formatRelative } from '../../lib/format';
import { DiskSection } from './DiskSection';
import { ProjectRulesLine } from '../rules/RulesSection';

const ICONS: Record<DashboardIcon, ReactNode> = {
  sessions: <LayersIcon />,
  ideas: <ReviewIcon />,
  tokens: <TokensIcon />,
  time: <ClockIcon />,
  cost: <CoinIcon />,
  disk: <HardDriveIcon />,
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
      </div>

      <ProjectRulesLine projectId={projectId} />

      <Section title="Sessions récentes" flush>
        {data.recent.length === 0 ? (
          <div className="px-4 py-3 text-sm text-muted">Aucune session.</div>
        ) : (
          data.recent.map((session, index) => (
            <div
              key={session.id}
              className={`flex items-center gap-3 px-4 py-3 text-sm ${
                index === 0 ? '' : 'border-t border-border'
              }`}
            >
              <div className="min-w-0 flex-1">
                <button
                  type="button"
                  className="block w-full truncate text-left hover:underline"
                  onClick={() => onOpenSession(session.id)}
                >
                  {session.title}
                </button>
              </div>
              <div className="flex shrink-0 items-center gap-2">
                {session.running ? <Badge tone="accent" dot pulse>en cours</Badge> : null}
                <span className="text-xs text-muted">{formatRelative(session.updatedAt)}</span>
              </div>
            </div>
          ))
        )}
      </Section>

      <DiskSection projectId={projectId} onOpenSession={onOpenSession} />
    </div>
  );
}
