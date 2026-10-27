import type { SessionState } from '../../lib/api';
import { Segmented } from '../ui/Segmented';
import { themes, type ThemeId } from '../../lib/theme';
import { stateDotClass } from '../../lib/format';
import { CloseIcon, FoxIcon, PlusIcon, SearchIcon, SettingsIcon } from '../Icons';
import { Button, buttonClass } from '../ui/Button';

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

export interface TabInfo {
  id: string;
  title: string;
  project: string;
  state: SessionState;
}

interface TopBarProps {
  tabs: TabInfo[];
  activeTabId: string | null;
  theme: ThemeId;
  onPickTab: (id: string) => void;
  onCloseTab: (id: string) => void;
  onNewSession: () => void;
  onOpenPalette: () => void;
  onThemeChange: (theme: ThemeId) => void;
}

export function TopBar({
  tabs,
  activeTabId,
  theme,
  onPickTab,
  onCloseTab,
  onNewSession,
  onOpenPalette,
  onThemeChange
}: TopBarProps) {
  return (
    <header className="flex min-h-12 shrink-0 flex-wrap items-stretch gap-1 border-b border-border bg-panel px-3">
      <a
        href="#/"
        aria-label="Accueil fennec"
        className="flex items-center gap-2 py-2 pl-1 pr-3 text-[0.9375rem] font-semibold tracking-[-0.01em] text-text"
      >
        <FoxIcon className="h-5 w-5 text-accent" />
        <span>fennec</span>
      </a>

      <div
        role="tablist"
        aria-label="Sessions ouvertes"
        className="flex min-w-0 flex-1 items-end gap-0.5 overflow-x-auto"
      >
        {tabs.map((tab) => {
          const selected = tab.id === activeTabId;
          return (
            <div
              key={tab.id}
              role="tab"
              aria-selected={selected}
              className={`-mb-px flex h-7 max-w-[280px] items-center gap-2 rounded-t-lg border border-b-0 px-2 pl-3 ${
                selected ? 'border-border bg-bg text-text' : 'border-transparent bg-transparent text-muted'
              }`}
            >
              <span className={`h-[7px] w-[7px] flex-none rounded-full ${stateDotClass(tab.state)}`} />
              <button
                type="button"
                onClick={() => onPickTab(tab.id)}
                className="min-w-0 overflow-hidden text-ellipsis whitespace-nowrap text-xs font-medium"
              >
                {tab.title}
              </button>
              <span className="whitespace-nowrap text-xs text-muted">{tab.project}</span>
              <Button
                variant="ghost"
                size="sm"
                iconOnly
                aria-label="Fermer l'onglet"
                onClick={() => onCloseTab(tab.id)}
              >
                <CloseIcon className="h-3 w-3" />
              </Button>
            </div>
          );
        })}
        <Button
          variant="ghost"
          size="sm"
          iconOnly
          aria-label="Nouvelle session"
          onClick={onNewSession}
          className="self-center"
        >
          <PlusIcon className="h-4 w-4" />
        </Button>
      </div>

      <div className="flex items-center gap-2 py-1.5">
        <button
          type="button"
          onClick={onOpenPalette}
          aria-label="Sessions, projets, commandes"
          className="flex h-8 items-center gap-2.5 rounded-lg border border-border px-2.5 text-[0.8125rem] text-muted transition-colors hover:border-border-strong hover:text-text md:min-w-[220px]"
        >
          <SearchIcon className="h-3.5 w-3.5" />
          <span className="hidden grow text-left md:inline">Sessions, projets, commandes</span>
          <kbd className="rounded-md border border-border px-1.5 py-px text-[0.6875rem] text-muted">⌘K</kbd>
        </button>

        <Segmented
          label="Thème"
          value={theme}
          onChange={onThemeChange}
          segments={themes.map((option) => ({
            value: option.id,
            ariaLabel: `Thème ${option.label}`,
            label: (
              <>
                <span className="h-3 w-3 rounded-full border border-border" style={{ background: option.swatch }} />
                <span className="hidden md:inline">{option.label}</span>
              </>
            )
          }))}
        />

        <a
          href="#/workflows"
          title="Workflows"
          aria-label="Workflows"
          className={buttonClass({ variant: 'ghost', size: 'sm', iconOnly: true })}
        >
          <WorkflowsIcon className="h-4 w-4" />
        </a>
        <a
          href="#/settings"
          title="Réglages"
          aria-label="Réglages"
          className={buttonClass({ variant: 'ghost', size: 'sm', iconOnly: true })}
        >
          <SettingsIcon className="h-4 w-4" />
        </a>
      </div>
    </header>
  );
}
