import { useEffect, useRef, useState } from 'react';
import type { AttentionItem, SessionState } from '../../lib/api';
import { SessionStateIcon } from '../session/SessionStateIcon';
import { BellIcon, CloseIcon, FoxIcon, PlusIcon, SearchIcon, SettingsIcon, SplitIcon } from '../Icons';
import { Badge } from '../ui/Badge';
import { Button, buttonClass } from '../ui/Button';

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

function kindLabel(kind: AttentionItem['kind']): string {
  switch (kind) {
    case 'question':
      return 'Question';
    case 'workflowPause':
      return 'Workflow';
    case 'awaitingIntegration':
      return 'À intégrer';
    case 'conflict':
      return 'Conflit';
    case 'error':
      return 'Erreur';
  }
}

/** The "N à regarder" bell: what needs the human, from any session, anywhere. */
function AttentionBell({ attention, onOpen }: { attention: AttentionItem[]; onOpen: (id: string) => void }) {
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
        aria-label="À regarder"
        aria-haspopup="true"
        aria-expanded={open}
        title={count > 0 ? `${count} à regarder` : 'Rien à regarder'}
        className={buttonClass({ variant: 'ghost', size: 'sm' }, count > 0 ? 'text-accent' : undefined)}
      >
        <span className="flex items-center gap-1">
          <BellIcon className="h-4 w-4" />
          {count > 0 ? <Badge tone="accent">{count}</Badge> : null}
        </span>
      </button>
      {open ? (
        <div className="absolute right-0 top-full z-10 mt-1 max-h-[70vh] w-72 overflow-y-auto rounded-md border border-border bg-panel p-1 shadow-lg">
          {count === 0 ? (
            <div className="px-2 py-1.5 text-xs text-muted">Rien à regarder.</div>
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
                  <Badge tone={item.kind === 'error' ? 'err' : 'accent'} dot>
                    {kindLabel(item.kind)}
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

export interface TabInfo {
  id: string;
  title: string;
  project: string;
  state: SessionState;
}

interface TopBarProps {
  tabs: TabInfo[];
  activeTabId: string | null;
  attention: AttentionItem[];
  onOpenAttention: (id: string) => void;
  onPickTab: (id: string) => void;
  onCloseTab: (id: string) => void;
  onOpenPalette: () => void;
  onNewSession: () => void;
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
  onNewSession,
  splitActive,
  onToggleSplit
}: TopBarProps) {
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
        aria-label="Accueil fennec"
        className="flex items-center gap-2 py-2 pl-1 pr-3 text-base font-semibold tracking-[-0.01em] text-text"
      >
        <FoxIcon className="h-5 w-5 text-accent" />
        <span>fennec</span>
      </a>

      <button
        type="button"
        onClick={onNewSession}
        aria-label="Nouveau chat"
        title="Nouveau chat"
        className="flex h-8 shrink-0 items-center gap-1.5 rounded-lg border border-border px-2 text-xs font-medium text-muted transition-colors hover:border-border-strong hover:text-text"
      >
        <PlusIcon className="h-3.5 w-3.5" />
        <span>Nouveau chat</span>
      </button>

      <div
        role="tablist"
        aria-label="Sessions ouvertes"
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
              title={`${tab.project} — ${tab.title}`}
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
              <SessionStateIcon state={tab.state} className="h-3.5 w-3.5" />
              <span className="min-w-0 overflow-hidden text-ellipsis whitespace-nowrap text-xs font-medium">
                {tab.title}
              </span>
              <Button
                variant="ghost"
                size="sm"
                iconOnly
                aria-label="Fermer l'onglet"
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
          aria-label="Rechercher"
          className="flex h-8 items-center gap-2 rounded-lg border border-border px-2.5 text-sm text-muted transition-colors hover:border-border-strong hover:text-text"
        >
          <SearchIcon className="h-3.5 w-3.5" />
          <span className="hidden text-left md:inline">Rechercher…</span>
          <kbd className="rounded-md border border-border px-1.5 py-px text-xs text-muted">⌘K</kbd>
        </button>

        <AttentionBell attention={attention} onOpen={onOpenAttention} />

        <button
          type="button"
          onClick={onToggleSplit}
          aria-label="Vue scindée"
          aria-pressed={splitActive}
          title="Vue scindée (⌘\)"
          className={buttonClass({ variant: 'ghost', size: 'sm' }, splitActive ? 'text-accent' : undefined)}
        >
          <SplitIcon className="h-4 w-4" />
        </button>
        <a
          href="#/insight"
          title="Insight"
          aria-label="Insight"
          className={buttonClass({ variant: 'ghost', size: 'sm' })}
        >
          <InsightIcon className="h-4 w-4" />
        </a>
        <a
          href="#/workflows"
          title="Workflows"
          aria-label="Workflows"
          className={buttonClass({ variant: 'ghost', size: 'sm' })}
        >
          <WorkflowsIcon className="h-4 w-4" />
        </a>
        <a
          href="#/backlog"
          title="Backlog"
          aria-label="Backlog"
          className={buttonClass({ variant: 'ghost', size: 'sm' })}
        >
          <BacklogIcon className="h-4 w-4" />
        </a>
        <a
          href="#/settings"
          title="Réglages"
          aria-label="Réglages"
          className={buttonClass({ variant: 'ghost', size: 'sm' })}
        >
          <SettingsIcon className="h-4 w-4" />
        </a>
      </div>
    </header>
  );
}
