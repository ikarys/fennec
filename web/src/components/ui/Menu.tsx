/**
 * A small overflow menu: one ⋯ button and a panel of MenuItem choices.
 * The panel is portaled to `<body>` and positioned fixed: the board's cards
 * clip their overflow and the page scrolls, so an in-tree panel would be cut
 * by both.
 */
import { createContext, useContext, useEffect, useRef, useState, type CSSProperties, type ReactNode } from 'react';
import { createPortal } from 'react-dom';
import { Button } from './Button';

type MenuContextValue = { close: () => void };

const MenuContext = createContext<MenuContextValue | null>(null);

/** The panel's maximum height, for the flip test; past it, the panel scrolls. */
const PANEL_MAX_HEIGHT = 320;
const GAP = 4;

type PanelPos = CSSProperties;

export function Menu({ label, icon, children }: { label: string; icon?: ReactNode; children: ReactNode }) {
  const [open, setOpen] = useState(false);
  const [pos, setPos] = useState<PanelPos | null>(null);
  const rootRef = useRef<HTMLDivElement>(null);
  const panelRef = useRef<HTMLDivElement>(null);

  // Close on outside mousedown, Escape, or a scroll (the panel is fixed and
  // would drift off its button) while the menu is open.
  useEffect(() => {
    if (!open) return;
    const onPointer = (e: MouseEvent) => {
      const target = e.target as Node;
      if (!rootRef.current?.contains(target) && !panelRef.current?.contains(target)) {
        setOpen(false);
      }
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') setOpen(false);
    };
    const onScroll = (e: Event) => {
      if (!panelRef.current?.contains(e.target as Node)) setOpen(false);
    };
    document.addEventListener('mousedown', onPointer);
    document.addEventListener('keydown', onKey);
    window.addEventListener('scroll', onScroll, true);
    return () => {
      document.removeEventListener('mousedown', onPointer);
      document.removeEventListener('keydown', onKey);
      window.removeEventListener('scroll', onScroll, true);
    };
  }, [open]);

  const close = () => setOpen(false);

  const toggle = () => {
    if (open) {
      setOpen(false);
      return;
    }
    const rect = rootRef.current?.getBoundingClientRect();
    if (!rect) return;
    // Open above when the panel would not fit below the button, and grow
    // away from the nearest side so it never leaves the screen.
    const above = rect.bottom + PANEL_MAX_HEIGHT > window.innerHeight;
    const vertical = above ? { bottom: window.innerHeight - rect.top + GAP } : { top: rect.bottom + GAP };
    const leftHalf = rect.left + rect.width / 2 < window.innerWidth / 2;
    setPos({ ...vertical, ...(leftHalf ? { left: rect.left } : { right: window.innerWidth - rect.right }) });
    setOpen(true);
  };

  return (
    <MenuContext.Provider value={{ close }}>
      <div ref={rootRef} className="relative">
        <Button
          variant="ghost"
          size="sm"
          iconOnly
          aria-label={label}
          aria-haspopup="menu"
          aria-expanded={open}
          onClick={toggle}
        >
          {icon ?? '⋯'}
        </Button>
      </div>
      {open && pos ? (
        createPortal(
          <div
            ref={panelRef}
            role="menu"
            style={pos}
            className="fixed z-50 flex max-h-80 min-w-52 flex-col gap-0.5 overflow-y-auto rounded-md border border-border bg-panel p-1 shadow-lg"
          >
            {children}
          </div>,
          document.body,
        )
      ) : null}
    </MenuContext.Provider>
  );
}

export function MenuItem({
  onSelect,
  tone = 'neutral',
  icon,
  children,
}: {
  onSelect: () => void;
  tone?: 'neutral' | 'danger';
  icon?: ReactNode;
  children: ReactNode;
}) {
  const menu = useContext(MenuContext);
  return (
    <button
      type="button"
      role="menuitem"
      className={`flex items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm transition-colors hover:bg-accent-soft hover:text-accent focus-visible:bg-accent-soft focus-visible:outline-none ${
        tone === 'danger' ? 'text-err' : 'text-text'
      }`}
      onClick={() => {
        menu?.close();
        onSelect();
      }}
    >
      {icon}
      {children}
    </button>
  );
}

export function MenuLabel({ children }: { children: ReactNode }) {
  return <div className="px-2 pt-1 pb-0.5 text-[11px] font-semibold tracking-wider text-faint uppercase">{children}</div>;
}
