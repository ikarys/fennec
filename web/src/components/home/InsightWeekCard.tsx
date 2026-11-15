// The collapsed Insight week card on Home: server-formatted 7-day cost, input and cache values.
import { useT } from '../../i18n';
import type { InsightView } from '../../lib/api';
import { Card } from '../ui/Card';

export function InsightWeekCard({ insight }: { insight: InsightView | null }) {
  const t = useT();

  if (!insight) return null;

  const parts: string[] = [];
  for (const kind of ['cost', 'input', 'cache'] as const) {
    const card = insight.cards.find((candidate) => candidate.kind === kind);
    if (card) parts.push(`${card.label} ${card.value}`);
  }

  if (parts.length === 0) return null;

  return (
    <Card className="overflow-hidden">
      <div className="flex items-center justify-between gap-2 border-b border-border bg-accent-soft px-4 py-2">
        <span className="text-sm font-medium text-accent">{t('homeThisWeek')}</span>
        <a href="#/insight" className="text-xs text-accent hover:text-accent-strong">
          {t('homeOpenInsight')}
        </a>
      </div>
      <div className="px-4 py-3 text-sm text-muted">{parts.join(' · ')}</div>
    </Card>
  );
}
