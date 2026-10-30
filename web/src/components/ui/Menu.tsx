/**
 * A small overflow menu: one ⋯ button and a panel of MenuItem choices.
 */
import { createContext, useContext, useEffect, useRef, useState, type ReactNode } from 'react';
import { Button } from './Button';

type MenuContextValue = { close: () => void };

const MenuContext = createContext<MenuContextValue | null>(null);

export function Menu({ label, children }: { label: string; children: ReactNode }) {
  const [open, setOpen] = useState(false);
  const rootRef = useRef<HTMLDivElement>(null);

  // Close on outside mousedown or Escape while the menu is open.
  useEffect(() => {
    if (!open) return;
    const onPointer = (e: MouseEvent) => {
      if (rootRef.current && !rootRef.current.contains(e.target as Node)) {
        setOpen(false);
      }
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

  const close = () => setOpen(false);

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
          onClick={() => setOpen(!open)}
        >
          ⋯
        </Button>
        {open ? (
          <div
            role="menu"
            className="absolute right-0 top-full z-10 mt-1 flex min-w-40 flex-col gap-0.5 rounded-md border border-border bg-panel p-1 shadow-lg"
          >
            {children}
          </div>
        ) : null}
      </div>
    </MenuContext.Provider>
  );
}

export function MenuItem({
  onSelect,
  tone = 'neutral',
  children,
}: {
  onSelect: () => void;
  tone?: 'neutral' | 'danger';
  children: ReactNode;
}) {
  const menu = useContext(MenuContext);
  return (
    <button
      type="button"
      role="menuitem"
      className={`rounded px-2 py-1 text-left text-xs hover:bg-raised ${
        tone === 'danger' ? 'text-err' : 'text-text'
      }`}
      onClick={() => {
        menu?.close();
        onSelect();
      }}
    >
      {children}
    </button>
  );
}
