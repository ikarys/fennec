// The attention bell: what needs the human, from any session, anywhere.
import { useEffect, useRef, useState } from 'react';
import { useT } from '../../i18n';
import type { MsgKey } from '../../i18n/messages';
import type { AttentionItem } from '../../lib/api';
import { BellIcon } from '../ui/Icons';
import { Badge } from '../ui/Badge';
import { buttonClass } from '../ui/Button';

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
export function AttentionBell({ attention, onOpen }: { attention: AttentionItem[]; onOpen: (id: string) => void }) {
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
                className="block w-full rounded-md px-2 py-1.5 text-left hover:bg-raised"
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
