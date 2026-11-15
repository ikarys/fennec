// Project page work tab, top: the server's dashboard tiles and plugins.
import { type ReactNode } from 'react';
import { Tile } from '../ui/Tile';
import {
  ClockIcon,
  CoinIcon,
  DecisionIcon,
  LayersIcon,
  ReviewIcon,
  TokensIcon,
} from '../ui/Icons';
import { getProjectDashboard, type DashboardIcon } from '../../lib/api';
import { ProjectRulesTile } from '../rules/RulesSection';
import { PluginsSection } from './PluginsSection';
import { useT } from '../../i18n';
import { useLoaded } from '../../hooks/useLoaded';

const ICONS: Record<DashboardIcon, ReactNode> = {
  sessions: <LayersIcon />,
  ideas: <ReviewIcon />,
  tokens: <TokensIcon />,
  time: <ClockIcon />,
  cost: <CoinIcon />,
  decisions: <DecisionIcon />,
};

export function DashboardSection({ projectId }: { projectId: string }) {
  const t = useT();

  const { data, error, loading } = useLoaded(() => getProjectDashboard(projectId), [projectId], `dashboard:${projectId}`);

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
      </div>

      <PluginsSection projectId={projectId} />
    </div>
  );
}
