// The editor page's file tab strip: the open files, the active one highlighted, closing one.
import { useRef } from 'react';
import { useT } from '../../i18n';
import { CloseIcon, FileIcon } from '../ui/Icons';
import { Button } from '../ui/Button';

export interface FileTab {
  path: string;
  name: string;
  dirty: boolean;
}

interface FileTabsProps {
  tabs: FileTab[];
  activePath: string | null;
  onPick: (path: string) => void;
  onClose: (path: string) => void;
}

export function FileTabs({ tabs, activePath, onPick, onClose }: FileTabsProps) {
  const t = useT();
  const tabRefs = useRef(new Map<string, HTMLDivElement>());

  // ARIA tablist navigation: arrows move the selection and the focus.
  const moveTab = (delta: number) => {
    if (tabs.length === 0) return;
    const index = tabs.findIndex((tab) => tab.path === activePath);
    const from = index === -1 ? 0 : index;
    const target = tabs[(from + delta + tabs.length) % tabs.length];
    onPick(target.path);
    tabRefs.current.get(target.path)?.focus();
  };

  return (
    <div
      role="tablist"
      aria-label={t('fileTabsAria')}
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
        const selected = tab.path === activePath;
        return (
          <div
            key={tab.path}
            role="tab"
            aria-selected={selected}
            tabIndex={selected ? 0 : -1}
            ref={(node) => {
              if (node) tabRefs.current.set(tab.path, node);
              else tabRefs.current.delete(tab.path);
            }}
            title={tab.path}
            onClick={() => onPick(tab.path)}
            onMouseDown={(event) => {
              // Middle click (wheel) closes the tab, like the header's tab bar.
              if (event.button === 1) {
                event.preventDefault();
                onClose(tab.path);
              }
            }}
            onKeyDown={(event) => {
              // The tab div is focusable but not a button: Enter/Space activate it.
              if (event.target !== event.currentTarget) return;
              if (event.key === 'Enter' || event.key === ' ') {
                event.preventDefault();
                onPick(tab.path);
              }
            }}
            className={`group relative -mb-px flex h-10 min-w-[96px] max-w-[220px] cursor-pointer items-center gap-2 rounded-t-lg border border-b-0 px-2 pl-3 transition-colors ${
              selected
                ? 'border-border bg-bg text-text'
                : 'border-transparent text-muted hover:bg-raised/50 hover:text-text'
            }`}
          >
            {selected ? <span aria-hidden className="absolute inset-x-2 top-0 h-0.5 rounded-b bg-accent" /> : null}
            <FileIcon className={selected ? 'h-3.5 w-3.5 text-accent' : 'h-3.5 w-3.5'} />
            <span className="min-w-0 overflow-hidden text-ellipsis whitespace-nowrap text-xs font-medium">
              {tab.name}
            </span>
            {tab.dirty ? <span aria-hidden className="h-1.5 w-1.5 shrink-0 rounded-full bg-accent" /> : null}
            <Button
              variant="ghost"
              size="sm"
              iconOnly
              aria-label={t('fileTabCloseAria', { file: tab.name })}
              onClick={(event) => {
                event.stopPropagation();
                onClose(tab.path);
              }}
              className="[@media(hover:hover)]:opacity-0 focus-visible:opacity-100 group-hover:opacity-100"
            >
              <CloseIcon className="h-3 w-3" />
            </Button>
          </div>
        );
      })}
    </div>
  );
}
