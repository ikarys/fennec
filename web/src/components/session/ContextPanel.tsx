/**
 * The session's right context rail: three tabs that open a drawer with the work, server,
 * and session blocks. The drawer floats over the thread or pins beside it; only its open
 * state is persisted while pinned. SessionPage keys it by session, so a floating drawer closes
 * when the session changes.
 */
import { useEffect, useState } from 'react';
import type { SeenImage, ServedThread, SessionDetail, SessionSkills, SessionUsage, ServerView } from '../../lib/api';
import type { ThreadView } from '../../lib/fold';
import { useT } from '../../i18n';
import { Button } from '../ui/Button';
import { CheckIcon, CloseIcon, LayersIcon, PinIcon, ServerIcon } from '../ui/Icons';
import { CriteriaSection } from './CriteriaSection';
import { DecisionsSection } from './DecisionsSection';
import { ModifiedFilesSection } from './ModifiedFilesSection';
import { PluginsSection } from './PluginsSection';
import { ServersSection } from './ServersSection';
import { SkillsSection } from './SkillsSection';
import { TokensSection } from './TokensSection';
import { VisionSection } from './VisionSection';
import { WorkplaceSection } from './WorkplaceSection';

const PINNED_KEY = 'fennec.contextPanel.pinned';
const TAB_KEY = 'fennec.contextPanel.tab';

const TABS = {
  work: { titleKey: 'contextTabWork', Icon: CheckIcon },
  servers: { titleKey: 'contextTabServers', Icon: ServerIcon },
  session: { titleKey: 'contextTabSession', Icon: LayersIcon },
} as const;

const TAB_ORDER = ['work', 'servers', 'session'] as const;
type ContextTab = (typeof TAB_ORDER)[number];

function isContextTab(value: string | null): value is ContextTab {
  return value === 'work' || value === 'servers' || value === 'session';
}

function loadPinned(): boolean {
  try {
    return localStorage.getItem(PINNED_KEY) === '1';
  } catch {
    return false;
  }
}

function loadTab(): ContextTab | null {
  try {
    const stored = localStorage.getItem(TAB_KEY);
    return isContextTab(stored) ? stored : null;
  } catch {
    return null;
  }
}

function savePinned(pinned: boolean): void {
  try {
    localStorage.setItem(PINNED_KEY, pinned ? '1' : '0');
  } catch {
    // A blocked storage only loses the pin state on reload.
  }
}

function saveTab(tab: ContextTab): void {
  try {
    localStorage.setItem(TAB_KEY, tab);
  } catch {
    // A blocked storage only loses the open tab on reload.
  }
}

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
  const [open, setOpen] = useState<ContextTab | null>(() => (pinned ? loadTab() ?? 'work' : null));
  const [runningServers, setRunningServers] = useState(0);

  useEffect(() => {
    savePinned(pinned);
  }, [pinned]);

  useEffect(() => {
    if (pinned && open) saveTab(open);
  }, [pinned, open]);

  useEffect(() => {
    if (pinned || open === null) return;
    const onKeyDown = (event: globalThis.KeyboardEvent) => {
      if (event.key === 'Escape') setOpen(null);
    };
    document.addEventListener('keydown', onKeyDown);
    return () => document.removeEventListener('keydown', onKeyDown);
  }, [pinned, open]);

  if (!hasSession || sessionId === null) return null;

  const drawerTitle = t(TABS[open ?? 'work'].titleKey);
  const DrawerIcon = TABS[open ?? 'work'].Icon;

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
      <div className={open === 'work' ? undefined : 'hidden'}>
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
      <div className={open === 'servers' ? undefined : 'hidden'}>
        <ServersSection
          sessionId={sessionId}
          onServers={(servers: ServerView[]) => setRunningServers(servers.filter((server) => server.running).length)}
        />
      </div>
      <div className={open === 'session' ? undefined : 'hidden'}>
        <TokensSection usage={usage} />
        <DecisionsSection decisions={served.decisions} />
        <SkillsSection skills={skills} hasSession={hasSession} />
        <VisionSection images={vision} />
        <PluginsSection sessionId={sessionId} />
      </div>
    </>
  );

  const drawer = pinned ? (
    <div
      role="complementary"
      aria-label={drawerTitle}
      className={`flex min-h-0 w-[380px] shrink-0 flex-col border-l border-border bg-panel ${open === null ? 'hidden' : ''}`}
    >
      {header}
      <div className="min-h-0 grow overflow-y-auto [scrollbar-width:thin]">{bodies}</div>
    </div>
  ) : (
    <div
      role="dialog"
      aria-label={drawerTitle}
      className={`absolute inset-y-0 right-full z-20 flex w-[380px] min-h-0 flex-col border-l border-border bg-panel shadow-float ${open === null ? 'hidden' : ''}`}
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
        {TAB_ORDER.map((tab) => {
          const TabIcon = TABS[tab].Icon;
          const title = t(TABS[tab].titleKey);
          const active = open === tab;

          let status = '';
          let statusClass = 'text-muted';
          if (tab === 'work') {
            const { passed, total, failed } = served.criteriaTally;
            if (total > 0) status = `${passed}/${total}`;
            if (failed > 0) statusClass = 'text-err';
            else if (total > 0 && passed === total) statusClass = 'text-ok';
          } else if (tab === 'servers') {
            if (runningServers > 0) {
              status = `● ${runningServers}`;
              statusClass = 'text-ok';
            }
          }

          return (
            <button
              key={tab}
              type="button"
              title={title}
              aria-label={title}
              aria-pressed={active}
              onClick={() => setOpen((current) => (current === tab ? null : tab))}
              className={`relative flex h-16 w-full flex-col items-center justify-center gap-1 ${active ? 'bg-accent-soft text-accent' : 'text-muted hover:bg-raised hover:text-text'}`}
            >
              {active ? <span aria-hidden className="absolute inset-y-0 left-0 w-0.5 bg-accent" /> : null}
              {tab === 'work' && served.criteriaTally.failed > 0 ? (
                <span aria-hidden className="absolute right-2 top-2 size-1.5 rounded-full bg-err" />
              ) : null}
              <TabIcon className="h-4 w-4" />
              <span className="text-[10px] leading-none">{title}</span>
              <span className={`text-[10px] leading-none font-mono tabular-nums ${statusClass}`}>{status}</span>
            </button>
          );
        })}
      </nav>
    </>
  );
}
