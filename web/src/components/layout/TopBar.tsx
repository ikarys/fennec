// The app header: brand, tab strip, then the right-hand groups (search, actions and notices, pages, layout).
import { useEffect, useState } from 'react';
import { useT } from '../../i18n';
import { getVersion, type AttentionItem } from '../../lib/api';
import { ChatIcon, SearchIcon, SplitIcon } from '../ui/Icons';
import { buttonClass } from '../ui/Button';
import { Divider } from '../ui/Divider';
import { AttentionBell } from './AttentionBell';
import { PageNav, type PageId } from './PageNav';
import { CHAT_PROJECT_ID, SessionTabs, type TabInfo } from './SessionTabs';
import { UpdateNotice } from './UpdateNotice';
import { WhatsNewNotice } from './WhatsNewNotice';

export type { TabInfo } from './SessionTabs';
export type { PageId } from './PageNav';

/** The binary's version and local build number, fetched once; shown in the brand title, null when the fetch fails. */
function useVersionLabel(): string | null {
  const [version, setVersion] = useState<string | null>(null);
  const [build, setBuild] = useState<string | null>(null);

  useEffect(() => {
    getVersion()
      .then((body) => {
        setVersion(body.version);
        setBuild(body.build);
      })
      .catch(() => undefined);
  }, []);

  if (!version) return null;
  return build ? `${version} · #${build.split('.')[0]}` : version;
}


interface TopBarProps {
  tabs: TabInfo[];
  activeTabId: string | null;
  attention: AttentionItem[];
  onOpenAttention: (id: string) => void;
  onPickTab: (id: string) => void;
  onCloseTab: (id: string) => void;
  onOpenPalette: () => void;
  /** The project of the open session, when there is one: the "+" menu lists it first. */
  currentProject: { id: string; name: string } | null;
  /** The projects the "+" menu offers, most recently used first. */
  projects: { id: string; name: string }[];
  onNewSession: (projectId: string) => void;
  onOpenPage: (page: PageId) => void;
  splitActive: boolean;
  onToggleSplit: () => void;
}

export function TopBar({
  tabs,
  activeTabId,
  attention,
  onOpenAttention,
  onPickTab,
  onCloseTab,
  onOpenPalette,
  currentProject,
  projects,
  onNewSession,
  onOpenPage,
  splitActive,
  onToggleSplit
}: TopBarProps) {
  const t = useT();
  const versionLabel = useVersionLabel();

  return (
    <header className="flex min-h-12 shrink-0 items-stretch gap-1 border-b border-border bg-panel px-3">
      <a
        href="#/"
        aria-label={t('topBarHomeAria')}
        title={versionLabel ? `fennec ${versionLabel}` : undefined}
        className="flex items-center gap-2 py-2 pl-1 pr-3 text-base font-semibold tracking-[-0.01em] text-text"
      >
        <img src="/emblem.png" alt="" className="h-6 w-6 rounded-md" />
        <span>fennec</span>
        {versionLabel ? <span className="font-mono text-xs font-normal text-faint">{versionLabel}</span> : null}
      </a>

      <Divider />

      {/* In split view each pane carries the tab strip. */}
      {splitActive ? (
        <div className="flex-1" />
      ) : (
        <SessionTabs
          tabs={tabs}
          activeTabId={activeTabId}
          onPickTab={onPickTab}
          onCloseTab={onCloseTab}
          currentProject={currentProject}
          projects={projects}
          onNewSession={onNewSession}
        />
      )}

      <div className="flex items-center gap-1.5 py-1.5">
        <button
          type="button"
          onClick={onOpenPalette}
          aria-label={t('paletteSearchLabel')}
          className="flex h-8 items-center gap-2 rounded-lg border border-border px-2.5 text-sm text-muted transition-colors hover:border-border-strong hover:text-text"
        >
          <SearchIcon className="h-3.5 w-3.5" />
          <span className="hidden text-left md:inline">{t('topBarSearchPlaceholder')}</span>
          <kbd className="rounded-md border border-border px-1.5 py-px text-xs text-muted">⌘K</kbd>
        </button>

        <Divider />

        <div role="group" className="flex items-center gap-0.5">
          {/* The chat has no project page to start from: one click from anywhere. */}
          <button
            type="button"
            onClick={() => onNewSession(CHAT_PROJECT_ID)}
            aria-label={t('paletteNewChat')}
            title={t('homeNewChatTitle')}
            className={buttonClass({ variant: 'icon', size: 'sm', iconOnly: true })}
          >
            <ChatIcon className="h-4 w-4" />
          </button>

          <WhatsNewNotice />
          <UpdateNotice />
          <AttentionBell attention={attention} onOpen={onOpenAttention} />
        </div>

        <Divider />

        <PageNav activeTabId={activeTabId} onOpenPage={onOpenPage} />

        <Divider />

        <div role="group" className="flex items-center gap-0.5">
          <button
            type="button"
            onClick={onToggleSplit}
            aria-label={t('topBarSplitAria')}
            aria-pressed={splitActive}
            title={t('topBarSplitTitle')}
            className={buttonClass({ variant: 'ghost', size: 'sm' }, splitActive ? 'bg-accent-soft text-accent' : undefined)}
          >
            <SplitIcon className="h-4 w-4" />
          </button>
        </div>
      </div>
    </header>
  );
}
