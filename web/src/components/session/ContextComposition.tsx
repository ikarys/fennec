/**
 * What a context is made of (the gauge's popover, Insight's consumers): a
 * stacked bar by kind, then each part with its share. Every figure comes from the server.
 */
import type { ContextComposition as Composition, ContextPartKind } from '../../lib/api';
import { Badge } from '../ui/Badge';
import { SectionLabel } from '../ui/SectionLabel';
import { useT } from '../../i18n';

const KIND_CLASS: Record<ContextPartKind, string> = {
  prompt: 'bg-accent',
  conversation: 'bg-ok',
  tools: 'bg-text/55'
};

function kindClass(kind: ContextPartKind | null): string {
  return kind ? KIND_CLASS[kind] : 'bg-border-strong';
}

export function ContextComposition({ composition, title }: { composition: Composition; title?: string }) {
  const t = useT();
  return (
    <div className="flex flex-col gap-3">
      <div className="flex items-center justify-between gap-2">
        <SectionLabel>{title ?? t('composition')}</SectionLabel>
        <Badge tone="accent" title={t('compositionIndicativeTitle')}>{t('indicative')}</Badge>
      </div>

      <div className="flex h-2 w-full gap-px overflow-hidden rounded-full bg-raised">
        {composition.kinds.map((k) => (
          <div key={k.kind} className={`h-full ${kindClass(k.kind)}`} style={{ width: `${k.percent}%` }} />
        ))}
      </div>
      <ul className="m-0 flex list-none flex-wrap gap-x-4 gap-y-1 p-0 text-xs">
        {composition.kinds.map((k) => (
          <li key={k.kind} className="flex items-center gap-1.5">
            <span className={`h-2 w-2 rounded-full ${kindClass(k.kind)}`} />
            <span className="text-text">{k.label}</span>
            <span className="text-muted tabular-nums">{k.percent} %</span>
          </li>
        ))}
      </ul>

      <ul className="m-0 flex list-none flex-col gap-2 border-t border-border p-0 pt-3">
        {composition.parts.map((p) => (
          <li key={p.label} className="flex flex-col gap-1">
            <div className="flex items-baseline gap-2 text-xs">
              <span className="min-w-0 flex-1 truncate text-text" title={p.label}>
                {p.label}
              </span>
              <span className="shrink-0 text-text tabular-nums">{p.tokens}</span>
              <span className="w-10 shrink-0 text-right whitespace-nowrap text-muted tabular-nums">{p.percent} %</span>
            </div>
            <div className="h-1 w-full overflow-hidden rounded-full bg-raised">
              <div className={`h-full min-w-0.5 rounded-full ${kindClass(p.kind)}`} style={{ width: `${p.percent}%` }} />
            </div>
          </li>
        ))}
      </ul>

      {composition.images && <p className="m-0 text-xs text-muted">{composition.images}</p>}
    </div>
  );
}
