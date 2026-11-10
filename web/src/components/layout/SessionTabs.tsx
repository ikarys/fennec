// The header's tab strip: open sessions and pages, then the "+" that starts a session.
import { useRef } from 'react';
import { useT } from '../../i18n';
import type { SessionState } from '../../lib/api';
import type { PageId } from '../../lib/route';
import { SessionStateIcon } from '../session/SessionStateIcon';
import { CloseIcon, EditIcon, FolderIcon, PlusIcon } from '../ui/Icons';
import { Badge } from '../ui/Badge';
import { Button } from '../ui/Button';
import { Menu, MenuItem, MenuLabel } from '../ui/Menu';
import { PageIcon } from './PageNav';

/** The chat's project id, fixed by the server (`chat::ID`). */
export const CHAT_PROJECT_ID = 'chat';

export type TabInfo =
  | { id: string; title: string; project: string; state: SessionState; kind: 'session' }
  | { id: string; title: string; kind: 'page'; page: PageId }
  | { id: string; title: string; kind: 'editor'; project: string };

interface SessionTabsProps {
  tabs: TabInfo[];
  activeTabId: string | null;
  onPickTab: (id: string) => void;
  onCloseTab: (id: string) => void;
  /** The project of the open session, when there is one: the "+" menu lists it first. */
  currentProject: { id: string; name: string } | null;
  /** The projects the "+" menu offers, most recently used first. */
  projects: { id: string; name: string }[];
  onNewSession: (projectId: string) => void;
}

export function SessionTabs({
  tabs,
  activeTabId,
  onPickTab,
  onCloseTab,
  currentProject,
  projects,
  onNewSession
}: SessionTabsProps) {
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
    <div className="flex min-w-0 flex-1 items-end">
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
        className="flex min-w-0 items-end gap-0.5 overflow-x-auto [scrollbar-width:none] [&::-webkit-scrollbar]:hidden"
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
              className={`group relative -mb-px flex h-10 min-w-[96px] max-w-[220px] cursor-pointer items-center gap-2 rounded-t-lg border border-b-0 px-2 pl-3 transition-colors ${
                selected
                  ? 'border-border bg-bg text-text'
                  : 'border-transparent text-muted hover:bg-raised/50 hover:text-text'
              }`}
            >
              {selected ? <span aria-hidden className="absolute inset-x-2 top-0 h-0.5 rounded-b bg-accent" /> : null}
              {tab.kind === 'session' ? (
                <SessionStateIcon state={tab.state} className={selected ? 'h-3.5 w-3.5 text-accent' : 'h-3.5 w-3.5'} />
              ) : tab.kind === 'page' ? (
                <PageIcon page={tab.page} className={selected ? 'h-3.5 w-3.5 text-accent' : 'h-3.5 w-3.5'} />
              ) : (
                <EditIcon className={selected ? 'h-3.5 w-3.5 text-accent' : 'h-3.5 w-3.5'} />
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

      {/* The "+" never guesses a project: the open session's comes first, then the others by recent use (the chat has its own button). */}
      <div className="self-center ml-1 shrink-0">
        <Menu label={t('topBarNewSessionMenuLabel')} icon={<PlusIcon className="h-4 w-4" />}>
          <MenuLabel>{t('topBarNewSessionMenuLabel')}</MenuLabel>
          {currentProject && currentProject.id !== CHAT_PROJECT_ID ? (
            <MenuItem
              onSelect={() => onNewSession(currentProject.id)}
              icon={<FolderIcon className="h-3.5 w-3.5 text-faint" />}
            >
              <span className="truncate">{currentProject.name}</span>
              <Badge tone="accent" appearance="pill" className="ml-auto">
                {t('topBarCurrentProject')}
              </Badge>
            </MenuItem>
          ) : null}
          {projects
            .filter((project) => project.id !== CHAT_PROJECT_ID && project.id !== currentProject?.id)
            .map((project) => (
              <MenuItem
                key={project.id}
                onSelect={() => onNewSession(project.id)}
                icon={<FolderIcon className="h-3.5 w-3.5 text-faint" />}
              >
                <span className="truncate">{project.name}</span>
              </MenuItem>
            ))}
        </Menu>
      </div>
    </div>
  );
}
