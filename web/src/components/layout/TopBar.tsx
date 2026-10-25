import type { SessionState } from '../../lib/api';
import { themes, type ThemeId } from '../../lib/theme';
import { stateDotClass } from '../../lib/format';
import { CloseIcon, FoxIcon, PlusIcon, SearchIcon, SettingsIcon } from '../Icons';

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
        className="flex items-center gap-2 py-2 pl-1 pr-3 text-[15px] font-semibold tracking-[-0.01em] text-text"
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
              className={`-mb-px flex min-h-[38px] max-w-[280px] items-center gap-2 rounded-t-lg border border-b-0 px-2 pl-3 ${
                selected ? 'border-border bg-bg text-text' : 'border-transparent bg-transparent text-muted'
              }`}
            >
              <span className={`h-[7px] w-[7px] flex-none rounded-full ${stateDotClass(tab.state)}`} />
              <button
                type="button"
                onClick={() => onPickTab(tab.id)}
                className="min-w-0 overflow-hidden text-ellipsis whitespace-nowrap text-[13px] font-medium"
              >
                {tab.title}
              </button>
              <span className="whitespace-nowrap text-[11px] text-muted">{tab.project}</span>
              <button
                type="button"
                aria-label="Fermer l'onglet"
                onClick={() => onCloseTab(tab.id)}
                className="flex h-6 w-6 items-center justify-center rounded-md text-muted hover:text-text"
              >
                <CloseIcon className="h-3 w-3" />
              </button>
            </div>
          );
        })}
        <button
          type="button"
          aria-label="Nouvelle session"
          onClick={onNewSession}
          className="mx-1 flex h-8 w-8 flex-none items-center justify-center self-center rounded-lg text-muted hover:text-text"
        >
          <PlusIcon className="h-4 w-4" />
        </button>
      </div>

      <div className="flex items-center gap-2 py-1.5">
        <button
          type="button"
          onClick={onOpenPalette}
          aria-label="Sessions, projets, commandes"
          className="flex min-h-8 items-center md:min-w-[220px] gap-2.5 rounded-lg border border-border bg-raised px-2.5 py-1 text-[13px] text-muted"
        >
          <SearchIcon className="h-3.5 w-3.5" />
          <span className="hidden grow text-left md:inline">Sessions, projets, commandes</span>
          <kbd className="rounded-md border border-border px-1.5 py-px text-[11px] text-muted">⌘K</kbd>
        </button>

        <div role="group" aria-label="Thème" className="flex gap-0.5 rounded-[10px] border border-border bg-raised p-0.5">
          {themes.map((option) => {
            const pressed = option.id === theme;
            return (
              <button
                key={option.id}
                type="button"
                aria-pressed={pressed}
                aria-label={`Thème ${option.label}`}
                onClick={() => onThemeChange(option.id)}
                className={`flex min-h-[26px] items-center gap-1.5 rounded-[7px] border-0 px-2 text-xs font-medium ${
                  pressed ? 'bg-panel text-text' : 'bg-transparent text-muted'
                }`}
              >
                <span
                  className="h-3 w-3 rounded-full border border-border"
                  style={{ background: option.swatch }}
                />
                <span className="hidden md:inline">{option.label}</span>
              </button>
            );
          })}
        </div>

        <a
          href="#/settings"
          title="Réglages"
          aria-label="Réglages"
          className="flex h-8 w-8 items-center justify-center rounded-lg text-muted hover:text-text"
        >
          <SettingsIcon className="h-4 w-4" />
        </a>
      </div>
    </header>
  );
}
