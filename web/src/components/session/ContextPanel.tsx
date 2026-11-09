/**
 * The session's right context rail: fixed tabs for work, servers, and session, plus a
 * shared plugins tab and one tab per plugin that owns a panel. The drawer floats over the
 * thread or pins beside it; only its open state is persisted while pinned. SessionPage keys it
 * by session, so a floating drawer closes when the session changes.
 */
import { useEffect, useState } from 'react';
import type {
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
import { Button } from '../ui/Button';
import { CheckIcon, CloseIcon, LayersIcon, PinIcon, ServerIcon, PuzzleIcon } from '../ui/Icons';
import { PLUGIN_ICONS } from '../plugins/pluginIcons';
import { loadPinned, loadTab, savePinned, saveTab } from './contextRailStorage';
import { RailTab } from './RailTab';
import { CriteriaSection } from './CriteriaSection';
import { DecisionsSection } from './DecisionsSection';
import { ModifiedFilesSection } from './ModifiedFilesSection';
import { PluginsSection, PluginOwnSection } from './PluginsSection';
import { ServersSection } from './ServersSection';
import { SkillsSection } from './SkillsSection';
import { TokensSection } from './TokensSection';
import { VisionSection } from './VisionSection';
import { WorkplaceSection } from './WorkplaceSection';

type TabDescriptor = {
  id: string;
  title: string;
  label: string;
  Icon: typeof PuzzleIcon;
};

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
}: ContextPanelProps) {
  const t = useT();
  const [pinned, setPinned] = useState(loadPinned);
  const [open, setOpen] = useState<string | null>(() => (pinned ? loadTab() ?? 'work' : null));
  const [runningServers, setRunningServers] = useState(0);
  const blocks = useSessionPluginBlocks(sessionId);

  const sharedBlocks = blocks?.filter((block) => block.placement !== 'own_panel') ?? [];

  const ownPlugins: { name: string; icon: ServedPluginBlock['icon']; blocks: ServedPluginBlock[] }[] = [];
  if (blocks !== null) {
    const byPlugin = new Map<string, ServedPluginBlock[]>();
    for (const block of blocks) {
      if (block.placement !== 'own_panel') continue;
      const existing = byPlugin.get(block.plugin);
      if (existing) existing.push(block);
      else byPlugin.set(block.plugin, [block]);
    }
    for (const [name, pluginBlocks] of byPlugin) {
      ownPlugins.push({ name, icon: pluginBlocks[0]!.icon, blocks: pluginBlocks });
    }
  }

  const tabs: TabDescriptor[] = [
    { id: 'work', title: t('contextTabWork'), label: t('contextTabWork'), Icon: CheckIcon },
    { id: 'servers', title: t('contextTabServers'), label: t('contextTabServers'), Icon: ServerIcon },
    { id: 'session', title: t('contextTabSession'), label: t('contextTabSession'), Icon: LayersIcon },
  ];

  if (sharedBlocks.length > 0) {
    tabs.push({ id: 'plugins', title: t('panelPlugins'), label: t('panelPlugins'), Icon: PuzzleIcon });
  }

  for (const plugin of ownPlugins) {
    tabs.push({
      id: `plugin:${plugin.name}`,
      title: t('pluginProvenance', { name: plugin.name }),
      label: plugin.name,
      Icon: PLUGIN_ICONS[plugin.icon],
    });
  }

  // A plugin tab that is gone (or not fetched yet) shows the work tab; `open` keeps the wish.
  const openTab = tabs.find((tab) => tab.id === open);
  const current = open === null ? null : (openTab?.id ?? 'work');

  useEffect(() => {
    savePinned(pinned);
  }, [pinned]);

  useEffect(() => {
    if (pinned && current !== null && current === open) saveTab(current);
  }, [pinned, current, open]);

  useEffect(() => {
    if (pinned || open === null) return;
    const onKeyDown = (event: globalThis.KeyboardEvent) => {
      if (event.key === 'Escape') setOpen(null);
    };
    document.addEventListener('keydown', onKeyDown);
    return () => document.removeEventListener('keydown', onKeyDown);
  }, [pinned, open]);

  if (!hasSession || sessionId === null) return null;

  const drawerTab = openTab ?? tabs[0]!;
  const drawerTitle = drawerTab.label;
  const DrawerIcon = drawerTab.Icon;

  const header = (
    <header className="flex h-10 shrink-0 items-center gap-2 border-b border-border pl-3 pr-2">
      <span className="grid size-6 shrink-0 place-items-center rounded-md bg-accent-soft text-accent">
        <DrawerIcon className="h-3.5 w-3.5" />
      </span>
      <span className="truncate text-sm font-semibold text-text">{drawerTitle}</span>
      <div className="ml-auto flex items-center gap-0.5">
        <Button
          type="button"
          variant="ghost"
          size="sm"
          iconOnly
          aria-pressed={pinned}
          title={pinned ? t('contextUnpin') : t('contextPin')}
          aria-label={pinned ? t('contextUnpin') : t('contextPin')}
          onClick={() => setPinned((current) => !current)}
        >
          <PinIcon className="h-3.5 w-3.5" />
        </Button>
        <Button
          type="button"
          variant="ghost"
          size="sm"
          iconOnly
          title={t('contextClose')}
          aria-label={t('contextClose')}
          onClick={() => setOpen(null)}
        >
          <CloseIcon className="h-3.5 w-3.5" />
        </Button>
      </div>
    </header>
  );

  const bodies = (
    <>
      <div className={current === 'work' ? undefined : 'hidden'}>
        <WorkplaceSection
          sessionId={sessionId}
          worktree={worktree ?? null}
          editorUrl={editorUrl}
          running={view.running}
          onDiscarded={onWorktreeChanged}
          integrationVersion={integrationVersion}
        />
        <CriteriaSection criteria={served.criteria} tally={served.criteriaTally} />
        <ModifiedFilesSection totals={served.modifiedFiles} sessionId={sessionId} />
      </div>
      <div className={current === 'servers' ? undefined : 'hidden'}>
        <ServersSection
          sessionId={sessionId}
          onServers={(servers: ServerView[]) => setRunningServers(servers.filter((server) => server.running).length)}
        />
      </div>
      <div className={current === 'session' ? undefined : 'hidden'}>
        <TokensSection usage={usage} />
        <DecisionsSection decisions={served.decisions} />
        <SkillsSection skills={skills} hasSession={hasSession} />
        <VisionSection images={vision} />
      </div>
      <div className={current === 'plugins' ? undefined : 'hidden'}>
        <PluginsSection blocks={sharedBlocks} />
      </div>
      {ownPlugins.map((plugin) => (
        <div key={plugin.name} className={current === `plugin:${plugin.name}` ? undefined : 'hidden'}>
          <PluginOwnSection plugin={plugin.name} blocks={plugin.blocks} />
        </div>
      ))}
    </>
  );

  const drawer = pinned ? (
    <div
      role="complementary"
      aria-label={drawerTitle}
      className={`flex min-h-0 w-[380px] shrink-0 flex-col border-l border-border bg-panel ${current === null ? 'hidden' : ''}`}
    >
      {header}
      <div className="min-h-0 grow overflow-y-auto [scrollbar-width:thin]">{bodies}</div>
    </div>
  ) : (
    <div
      role="dialog"
      aria-label={drawerTitle}
      className={`absolute inset-y-0 right-full z-20 flex w-[380px] min-h-0 flex-col border-l border-border bg-panel shadow-float ${current === null ? 'hidden' : ''}`}
    >
      {header}
      <div className="min-h-0 grow overflow-y-auto [scrollbar-width:thin]">{bodies}</div>
    </div>
  );

  return (
    <>
      {pinned ? drawer : null}
      <nav aria-label={t('sessionContextLabel')} className="relative w-[52px] shrink-0 flex flex-col items-center border-l border-border bg-bg pt-2">
        {!pinned ? drawer : null}
        {tabs.map((tab) => {
          let status: string | undefined;
          let statusClass: string | undefined;
          let alert = false;

          if (tab.id === 'work') {
            const { passed, total, failed } = served.criteriaTally;
            if (total > 0) status = `${passed}/${total}`;
            if (failed > 0) {
              statusClass = 'text-err';
              alert = true;
            } else if (total > 0 && passed === total) {
              statusClass = 'text-ok';
            }
          } else if (tab.id === 'servers') {
            if (runningServers > 0) {
              status = `● ${runningServers}`;
              statusClass = 'text-ok';
            }
          }

          return (
            <RailTab
              key={tab.id}
              title={tab.title}
              label={tab.label}
              Icon={tab.Icon}
              active={current === tab.id}
              status={status}
              statusClass={statusClass}
              alert={alert}
              onClick={() => setOpen(current === tab.id ? null : tab.id)}
            />
          );
        })}
      </nav>
    </>
  );
}
