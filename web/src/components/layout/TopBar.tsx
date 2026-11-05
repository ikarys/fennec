import { useEffect, useRef, useState } from 'react';
import { useT } from '../../i18n';
import type { MsgKey } from '../../i18n/messages';
import type { AttentionItem, SessionState } from '../../lib/api';
import { SessionStateIcon } from '../session/SessionStateIcon';
import { BellIcon, ChatIcon, CloseIcon, FoxIcon, PlusIcon, SearchIcon, SettingsIcon, SplitIcon } from '../Icons';
import { Badge } from '../ui/Badge';
import { Button, buttonClass } from '../ui/Button';
import { Menu, MenuItem } from '../ui/Menu';
import { UpdateNotice } from './UpdateNotice';

function InsightIcon({ className }: { className?: string }) {
  return (
    <svg
      className={className}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="2"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
    >
      <path d="M3 3v18h18" />
      <path d="M8 17v-5" />
      <path d="M13 17v-9" />
      <path d="M18 17v-3" />
    </svg>
  );
}

function WorkflowsIcon({ className }: { className?: string }) {
  return (
    <svg
      className={className}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="2"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
    >
      <circle cx="6" cy="6" r="3" />
      <circle cx="18" cy="6" r="3" />
      <circle cx="12" cy="18" r="3" />
      <path d="M6 9v3a3 3 0 003 3h6a3 3 0 003-3V9" />
    </svg>
  );
}

function BacklogIcon({ className }: { className?: string }) {
  return (
    <svg
      className={className}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="2"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
    >
      <path d="M3 6l2 2 3-3" />
      <path d="M3 13l2 2 3-3" />
      <path d="M3 20l2 2 3-3" />
      <path d="M13 6h8" />
      <path d="M13 13h8" />
      <path d="M13 20h8" />
    </svg>
  );
}

function kindLabel(kind: AttentionItem['kind'], t: (key: MsgKey, vars?: Record<string, string | number>) => string): string {
  switch (kind) {
    case 'question':
      return t('questionHeader');
    case 'workflowPause':
      return t('workflowPickerLabel');
    case 'awaitingIntegration':
      return t('topBarAwaitingIntegration');
    case 'conflict':
      return t('topBarConflict');
    case 'error':
      return t('error');
  }
}

/** The attention bell: what needs the human, from any session, anywhere. */
function AttentionBell({ attention, onOpen }: { attention: AttentionItem[]; onOpen: (id: string) => void }) {
  const t = useT();
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);
  const count = attention.length;

  // Close on outside mousedown or Escape while open.
  useEffect(() => {
    if (!open) return;
    const onPointer = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false);
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') setOpen(false);
    };
    document.addEventListener('mousedown', onPointer);
    document.addEventListener('keydown', onKey);
    return () => {
      document.removeEventListener('mousedown', onPointer);
      document.removeEventListener('keydown', onKey);
    };
  }, [open]);

  return (
    <div ref={ref} className="relative">
      <button
        type="button"
        onClick={() => setOpen((value) => !value)}
        aria-label={t('topBarAttentionAria')}
        aria-haspopup="true"
        aria-expanded={open}
        title={count > 0 ? t('topBarAttentionCount', { count }) : t('topBarAttentionNone')}
        className={buttonClass({ variant: 'ghost', size: 'sm' }, count > 0 ? 'text-attention' : undefined)}
      >
        <span className="flex items-center gap-1">
          <BellIcon className="h-4 w-4" />
          {count > 0 ? <Badge tone="attention">{count}</Badge> : null}
        </span>
      </button>
      {open ? (
        <div className="absolute right-0 top-full z-10 mt-1 max-h-[70vh] w-72 overflow-y-auto rounded-md border border-border bg-panel p-1 shadow-lg">
          {count === 0 ? (
            <div className="px-2 py-1.5 text-xs text-muted">{t('topBarAttentionEmpty')}</div>
          ) : (
            attention.map((item) => (
              <button
                key={item.sessionId}
                type="button"
                onClick={() => {
                  setOpen(false);
                  onOpen(item.sessionId);
                }}
                className="block w-full rounded px-2 py-1.5 text-left hover:bg-raised"
              >
                <div className="flex items-center gap-1.5">
                  <Badge tone={item.kind === 'error' ? 'err' : 'attention'} dot>
                    {kindLabel(item.kind, t)}
                  </Badge>
                  <span className="min-w-0 flex-1 truncate text-xs font-medium text-text">{item.title}</span>
                </div>
                {item.detail ? <div className="mt-0.5 truncate text-xs text-muted">{item.detail}</div> : null}
              </button>
            ))
          )}
        </div>
      ) : null}
    </div>
  );
}

/** The binary's version, fetched once; muted next to the wordmark, hidden when the fetch fails. */
function VersionTag() {
  const [version, setVersion] = useState<string | null>(null);

  useEffect(() => {
    fetch('/api/version')
      .then((response) => (response.ok ? response.json() : null))
      .then((body: { version?: string } | null) => {
        if (body?.version) setVersion(body.version);
      })
      .catch(() => undefined);
  }, []);

  return version ? <span className="text-xs font-normal text-muted">{version}</span> : null;
}

import type { PageId } from '../../lib/route';
export type { PageId };

/** The chat's project id, fixed by the server (`chat::ID`). */
const CHAT_PROJECT_ID = 'chat';

/** The fixed pages, in the bar's order; `PageIcon` draws each. */
const PAGES: { page: PageId; title: MsgKey }[] = [
  { page: 'backlog', title: 'tasks' },
  { page: 'workflows', title: 'workflows' },
  { page: 'insight', title: 'insight' },
  { page: 'settings', title: 'settings' }
];

export type TabInfo =
  | { id: string; title: string; project: string; state: SessionState; kind: 'session' }
  | { id: string; title: string; kind: 'page'; page: PageId };

function PageIcon({ page, className }: { page: PageId; className?: string }) {
  switch (page) {
    case 'settings':
      return <SettingsIcon className={className} />;
    case 'insight':
      return <InsightIcon className={className} />;
    case 'workflows':
      return <WorkflowsIcon className={className} />;
    case 'backlog':
      return <BacklogIcon className={className} />;
  }
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
  /** The projects the "+" menu offers. */
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
  const tabRefs = useRef(new Map<string, HTMLDivElement>());

  // ARIA tablist navigation: arrows move the selection and the focus.
  const moveTab = (delta: number) => {
    if (tabs.length === 0) return;
    const index = tabs.findIndex((tab) => tab.id === activeTabId);
    const from = index === -1 ? 0 : index;
    const target = tabs[(from + delta + tabs.length) % tabs.length];
    onPickTab(target.id);
    tabRefs.current.get(target.id)?.focus();
  };

  return (
    <header className="flex min-h-12 shrink-0 items-stretch gap-1 border-b border-border bg-panel px-3">
      <a
        href="#/"
        aria-label={t('topBarHomeAria')}
        className="flex items-center gap-2 py-2 pl-1 pr-3 text-base font-semibold tracking-[-0.01em] text-text"
      >
        <FoxIcon className="h-5 w-5 text-accent" />
        <span>fennec</span>
        <VersionTag />
      </a>

      {/* The "+" never guesses a project: the open session's comes first, then the chat, then the others. */}
      <div className="self-center">
        <Menu label={t('topBarNewSessionMenuLabel')} icon={<PlusIcon className="h-4 w-4" />}>
          {currentProject && currentProject.id !== CHAT_PROJECT_ID ? (
            <MenuItem onSelect={() => onNewSession(currentProject.id)}>
              {t('paletteNewSessionIn', { name: currentProject.name })}
            </MenuItem>
          ) : null}
          <MenuItem onSelect={() => onNewSession(CHAT_PROJECT_ID)}>
            <span className="flex items-center gap-1.5">
              <ChatIcon className="h-3.5 w-3.5 text-accent" />
              {t('paletteNewChat')}
            </span>
          </MenuItem>
          {projects
            .filter((project) => project.id !== CHAT_PROJECT_ID && project.id !== currentProject?.id)
            .map((project) => (
              <MenuItem key={project.id} onSelect={() => onNewSession(project.id)}>
                {project.name}
              </MenuItem>
            ))}
        </Menu>
      </div>

      <div
        role="tablist"
        aria-label={t('topBarTabsAria')}
        onKeyDown={(event) => {
          if (event.key === 'ArrowRight') {
            event.preventDefault();
            moveTab(1);
          } else if (event.key === 'ArrowLeft') {
            event.preventDefault();
            moveTab(-1);
          }
        }}
        className="flex min-w-0 flex-1 items-end gap-0.5 overflow-x-auto [scrollbar-width:none] [&::-webkit-scrollbar]:hidden"
      >
        {tabs.map((tab) => {
          const selected = tab.id === activeTabId;
          return (
            <div
              key={tab.id}
              role="tab"
              aria-selected={selected}
              tabIndex={selected ? 0 : -1}
              ref={(node) => {
                if (node) tabRefs.current.set(tab.id, node);
                else tabRefs.current.delete(tab.id);
              }}
              title={tab.kind === 'session' ? t('topBarSessionTabTitle', { project: tab.project, title: tab.title }) : tab.title}
              onClick={() => onPickTab(tab.id)}
              onMouseDown={(event) => {
                // Middle click (wheel) closes the tab, like a browser tab bar.
                // preventDefault stops Chrome's autoscroll, which would
                // otherwise swallow the click before auxclick fires.
                if (event.button === 1) {
                  event.preventDefault();
                  onCloseTab(tab.id);
                }
              }}
              onKeyDown={(event) => {
                // The tab div is focusable but not a button: Enter/Space activate it.
                if (event.target !== event.currentTarget) return;
                if (event.key === 'Enter' || event.key === ' ') {
                  event.preventDefault();
                  onPickTab(tab.id);
                }
              }}
              className={`group -mb-px flex h-10 min-w-[96px] max-w-[220px] cursor-pointer items-center gap-2 rounded-t-lg border border-b-0 px-2 pl-3 transition-colors ${
                selected
                  ? 'border-border bg-bg text-text'
                  : 'border-transparent text-muted hover:bg-raised/50 hover:text-text'
              }`}
            >
              {tab.kind === 'session' ? (
                <SessionStateIcon state={tab.state} className="h-3.5 w-3.5" />
              ) : (
                <PageIcon page={tab.page} className="h-3.5 w-3.5" />
              )}
              <span className="min-w-0 overflow-hidden text-ellipsis whitespace-nowrap text-xs font-medium">
                {tab.title}
              </span>
              <Button
                variant="ghost"
                size="sm"
                iconOnly
                aria-label={t('topBarCloseTabAria')}
                onClick={(event) => {
                  event.stopPropagation();
                  onCloseTab(tab.id);
                }}
                className="[@media(hover:hover)]:opacity-0 focus-visible:opacity-100 group-hover:opacity-100"
              >
                <CloseIcon className="h-3 w-3" />
              </Button>
            </div>
          );
        })}
      </div>

      <div className="flex items-center gap-2 py-1.5">
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

        <UpdateNotice />

        <AttentionBell attention={attention} onOpen={onOpenAttention} />

        {/* The page's icon is lit when its tab is active, so the tab and the icon show one state. */}
        {PAGES.map(({ page, title }) => {
          const active = activeTabId === `page:${page}`;
          return (
            <button
              key={page}
              type="button"
              onClick={() => onOpenPage(page)}
              aria-label={t(title)}
              aria-pressed={active}
              title={t(title)}
              className={buttonClass({ variant: 'icon', size: 'sm', iconOnly: true }, active ? 'text-accent' : undefined)}
            >
              <PageIcon page={page} className="h-4 w-4" />
            </button>
          );
        })}

        <button
          type="button"
          onClick={onToggleSplit}
          aria-label={t('topBarSplitAria')}
          aria-pressed={splitActive}
          title={t('topBarSplitTitle')}
          className={buttonClass({ variant: 'ghost', size: 'sm' }, splitActive ? 'text-accent' : undefined)}
        >
          <SplitIcon className="h-4 w-4" />
        </button>
      </div>
    </header>
  );
}
