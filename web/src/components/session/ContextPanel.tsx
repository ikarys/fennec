/**
 * The session's right context rail: a SideRail of work, servers, session, and plugin panels.
 */
import { useState } from 'react';
import type {
  ContextView,
  SeenImage,
  ServedThread,
  SessionDetail,
  SessionSkills,
  SessionUsage,
  ServerView,
  ServedPluginBlock,
} from '../../lib/api';
import type { ThreadView } from '../../lib/fold';
import { useT } from '../../i18n';
import { useSessionPluginBlocks } from '../../hooks/useSessionPluginBlocks';
import { CheckIcon, LayersIcon, PuzzleIcon, ServerIcon } from '../ui/Icons';
import { SideRail, type RailPanel } from '../ui/SideRail';
import { PLUGIN_ICONS } from '../plugins/pluginIcons';
import { CriteriaSection } from './CriteriaSection';
import { ContextSection } from './ContextSection';
import { DecisionsSection } from './DecisionsSection';
import { ModifiedFilesSection } from './ModifiedFilesSection';
import { PluginsSection, PluginOwnSection } from './PluginsSection';
import { ServersSection } from './ServersSection';
import { SkillsSection } from './SkillsSection';
import { TokensSection } from './TokensSection';
import { VisionSection } from './VisionSection';
import { WorkplaceSection } from './WorkplaceSection';

interface ContextPanelProps {
  view: ThreadView;
  skills: SessionSkills | null;
  usage: SessionUsage | null;
  /** False on the new-session tab, where there is no log to fold: the panel is not shown. */
  hasSession: boolean;
  worktree: SessionDetail['worktree'];
  /** Opens the folder the agent works in (worktree, else project) in the chosen IDE; null while unknown. */
  editorUrl: string | null;
  sessionId: string | null;
  onWorktreeChanged: () => void;
  integrationVersion: number;
  vision: SeenImage[];
  served: ServedThread;
  context: ContextView | null;
  /** Bumped by the header's gauge: opens the Session tab on its Context section. */
  contextRevealKey: number;
}

export function ContextPanel({
  view,
  skills,
  usage,
  hasSession,
  worktree,
  editorUrl,
  sessionId,
  onWorktreeChanged,
  integrationVersion,
  vision,
  served,
  context,
  contextRevealKey,
}: ContextPanelProps) {
  const t = useT();
  const [runningServers, setRunningServers] = useState(0);
  const plugins = useSessionPluginBlocks(sessionId);

  if (!hasSession || sessionId === null) return null;

  const blocks = plugins?.blocks ?? [];
  const sharedBlocks = blocks.filter((block) => block.placement !== 'own_panel');
  const panels: RailPanel[] = [];

  const { passed, total, failed } = served.criteriaTally;
  const criteriaAlert = failed > 0;
  const criteriaClass = criteriaAlert ? 'text-err' : total > 0 && passed === total ? 'text-ok' : undefined;

  panels.push({
    id: 'work',
    title: t('contextTabWork'),
    label: t('contextTabWork'),
    Icon: CheckIcon,
    status: total > 0 ? `${passed}/${total}` : undefined,
    statusClass: criteriaClass,
    alert: criteriaAlert,
    body: (
      <>
        <WorkplaceSection
          sessionId={sessionId}
          worktree={worktree ?? null}
          editorUrl={editorUrl}
          running={view.running}
          onDiscarded={onWorktreeChanged}
          integrationVersion={integrationVersion}
        />
        <CriteriaSection criteria={served.criteria} tally={served.criteriaTally} planAwaitingBuild={served.pendingPlan !== null || served.handedPlan !== null} />
        <ModifiedFilesSection totals={served.modifiedFiles} sessionId={sessionId} hasDiff={Boolean(worktree?.commits)} />
      </>
    ),
  });

  panels.push({
    id: 'servers',
    title: t('contextTabServers'),
    label: t('contextTabServers'),
    Icon: ServerIcon,
    status: runningServers > 0 ? `● ${runningServers}` : undefined,
    statusClass: runningServers > 0 ? 'text-ok' : undefined,
    body: <ServersSection sessionId={sessionId} onServers={(servers: ServerView[]) => setRunningServers(servers.filter((server) => server.running).length)} />,
  });

  panels.push({
    id: 'session',
    title: t('contextTabSession'),
    label: t('contextTabSession'),
    Icon: LayersIcon,
    body: (
      <>
        <TokensSection usage={usage} />
        <ContextSection context={context} revealKey={contextRevealKey} />
        <DecisionsSection decisions={served.decisions} />
        <SkillsSection skills={skills} hasSession={hasSession} />
        <VisionSection images={vision} />
      </>
    ),
  });

  for (const tab of plugins?.tabs ?? []) {
    if (tab.kind === 'shared') {
      panels.push({
        id: 'plugins',
        title: t('panelPlugins'),
        label: t('panelPlugins'),
        Icon: PuzzleIcon,
        body: <PluginsSection blocks={sharedBlocks} />,
      });
    } else {
      const pluginBlocks: ServedPluginBlock[] = blocks.filter((block) => block.placement === 'own_panel' && block.plugin === tab.plugin);
      panels.push({
        id: `plugin:${tab.plugin}`,
        title: t('pluginProvenance', { name: tab.plugin }),
        label: tab.plugin,
        Icon: PLUGIN_ICONS[tab.icon],
        body: <PluginOwnSection plugin={tab.plugin} blocks={pluginBlocks} />,
      });
    }
  }

  return (
    <SideRail
      side="right"
      label={t('sessionContextLabel')}
      storageKey="fennec.contextPanel"
      defaultPinned
      panels={panels}
      reveal={{ id: 'session', key: contextRevealKey }}
    />
  );
}
