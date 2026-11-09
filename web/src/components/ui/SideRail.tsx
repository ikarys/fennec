// A shared side rail: an icon tab rail with a floating or pinned drawer.
import { useEffect, useState, type ReactNode } from 'react';
import { useT } from '../../i18n';
import { Button } from './Button';
import { CloseIcon, PinIcon, type PuzzleIcon } from './Icons';
import { RailTab } from './RailTab';

export interface RailPanel {
  id: string;
  /** Tooltip and aria-label of the tab. */
  title: string;
  label: string;
  Icon: typeof PuzzleIcon;
  status?: string;
  statusClass?: string;
  alert?: boolean;
  body: ReactNode;
}

interface SideRailProps {
  side: 'left' | 'right';
  /** aria-label of the rail's <nav>. */
  label: string;
  /** localStorage prefix: `${storageKey}.pinned` ('1'/'0') and `${storageKey}.tab`. */
  storageKey: string;
  /** Pinned (and open on its first panel) when nothing is stored yet. */
  defaultPinned?: boolean;
  panels: RailPanel[];
  /** Bumping `key` (> 0) opens panel `id`. */
  reveal?: { id: string; key: number };
  /** Drawer width class, default 'w-[380px]'. */
  width?: string;
}

function loadPinned(storageKey: string): boolean | null {
  try {
    const value = localStorage.getItem(`${storageKey}.pinned`);
    if (value === '1') return true;
    if (value === '0') return false;
    return null;
  } catch {
    return null;
  }
}

function loadTab(storageKey: string): string | null {
  try {
    return localStorage.getItem(`${storageKey}.tab`);
  } catch {
    return null;
  }
}

function savePinned(storageKey: string, pinned: boolean): void {
  try {
    localStorage.setItem(`${storageKey}.pinned`, pinned ? '1' : '0');
  } catch {
    // A blocked storage only loses the pin state on reload.
  }
}

function saveTab(storageKey: string, tab: string): void {
  try {
    localStorage.setItem(`${storageKey}.tab`, tab);
  } catch {
    // A blocked storage only loses the open tab on reload.
  }
}

export function SideRail({
  side,
  label,
  storageKey,
  defaultPinned,
  panels,
  reveal,
  width = 'w-[380px]',
}: SideRailProps) {
  const t = useT();
  const [pinned, setPinned] = useState(() => loadPinned(storageKey) ?? defaultPinned ?? false);
  const [open, setOpen] = useState<string | null>(() => (pinned ? loadTab(storageKey) ?? panels[0]?.id ?? null : null));

  const openPanel = panels.find((panel) => panel.id === open);
  const current = open === null ? null : (openPanel?.id ?? panels[0]?.id ?? null);

  useEffect(() => {
    savePinned(storageKey, pinned);
  }, [pinned, storageKey]);

  useEffect(() => {
    if (pinned && current !== null && current === open) saveTab(storageKey, current);
  }, [pinned, current, open, storageKey]);

  useEffect(() => {
    if (reveal && reveal.key > 0) setOpen(reveal.id);
  }, [reveal?.key]);

  useEffect(() => {
    if (pinned || open === null) return;
    const onKeyDown = (event: globalThis.KeyboardEvent) => {
      if (event.key === 'Escape') setOpen(null);
    };
    document.addEventListener('keydown', onKeyDown);
    return () => document.removeEventListener('keydown', onKeyDown);
  }, [pinned, open]);

  if (panels.length === 0) return null;

  const drawerPanel = openPanel ?? panels[0]!;
  const edge = side === 'right' ? 'border-l' : 'border-r';
  const drawerTitle = drawerPanel.label;
  const DrawerIcon = drawerPanel.Icon;

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
      {panels.map((panel) => (
        <div key={panel.id} className={current === panel.id ? undefined : 'hidden'}>
          {panel.body}
        </div>
      ))}
    </>
  );

  const drawer = pinned ? (
    <div
      role="complementary"
      aria-label={drawerTitle}
      className={`flex min-h-0 ${width} shrink-0 flex-col ${edge} border-border bg-panel ${current === null ? 'hidden' : ''}`}
    >
      {header}
      <div className="min-h-0 grow overflow-y-auto [scrollbar-width:thin]">{bodies}</div>
    </div>
  ) : (
    <div
      role="dialog"
      aria-label={drawerTitle}
      className={`absolute inset-y-0 ${side === 'right' ? 'right-full' : 'left-full'} z-20 flex ${width} min-h-0 flex-col ${edge} border-border bg-panel shadow-float ${current === null ? 'hidden' : ''}`}
    >
      {header}
      <div className="min-h-0 grow overflow-y-auto [scrollbar-width:thin]">{bodies}</div>
    </div>
  );

  const nav = (
    <nav
      aria-label={label}
      className={`relative w-[52px] shrink-0 flex flex-col items-center bg-bg pt-2 ${edge} border-border`}
    >
      {!pinned ? drawer : null}
      {panels.map((panel) => (
        <RailTab
          key={panel.id}
          side={side}
          title={panel.title}
          label={panel.label}
          Icon={panel.Icon}
          active={current === panel.id}
          status={panel.status}
          statusClass={panel.statusClass}
          alert={panel.alert}
          onClick={() => setOpen(current === panel.id ? null : panel.id)}
        />
      ))}
    </nav>
  );

  return side === 'right' ? (
    <>
      {pinned ? drawer : null}
      {nav}
    </>
  ) : (
    <>
      {nav}
      {pinned ? drawer : null}
    </>
  );
}
