/**
 * The night report banner: what the autonomous queue did while the human slept.
 */
import { useState } from 'react';
import { Badge } from '../../ui/Badge';
import { Button } from '../../ui/Button';
import { CheckIcon, ChevronDownIcon, ChevronRightIcon, MoonIcon, ReviewIcon } from '../../Icons';
import { Card } from '../../ui/Card';
import { formatRelative } from '../../../lib/format';
import { IdeaStatusBadge } from './IdeaStatusBadge';
import { IdeaMeta } from './IdeaMeta';
import type { IdeaBoard } from '../../../lib/ideas';

type Report = NonNullable<IdeaBoard['nightReport']>;

// French summary labels keep the singular for 1 and the plural for 2+.
function frCount(n: number, word: string): string {
  return `${n} ${n === 1 ? word : `${word}s`}`;
}

export function NightReport({
  report,
  caps,
  onOpenSession,
  onDismiss,
}: {
  report: Report;
  caps: IdeaBoard['queue']['caps'];
  onOpenSession: (sessionId: string) => void;
  onDismiss: () => Promise<boolean>;
}) {
  // Open by default: the report is what the human reads first after a run.
  const [open, setOpen] = useState(true);

  const badges: { label: string; tone: 'accent' | 'ok' | 'err' }[] = [];
  if (report.summary.review > 0)
    badges.push({ label: `${report.summary.review} à relire`, tone: 'accent' });
  if (report.summary.question > 0)
    badges.push({ label: frCount(report.summary.question, 'question'), tone: 'accent' });
  if (report.summary.halted > 0)
    badges.push({ label: frCount(report.summary.halted, 'arrêtée'), tone: 'err' });
  if (report.summary.failed > 0)
    badges.push({ label: frCount(report.summary.failed, 'échouée'), tone: 'err' });
  if (report.summary.integrated > 0)
    badges.push({ label: frCount(report.summary.integrated, 'intégrée'), tone: 'ok' });
  if (report.summary.running > 0)
    badges.push({ label: `${report.summary.running} en cours`, tone: 'accent' });

  const toggleLabel = open ? 'Masquer les idées' : 'Voir les idées';

  return (
    <Card className="flex flex-col px-4 py-2.5">
      <div className="flex items-center gap-3 min-h-8">
        <Button
          variant="ghost"
          size="sm"
          iconOnly
          title={toggleLabel}
          aria-label={toggleLabel}
          aria-expanded={open}
          onClick={() => setOpen((v) => !v)}
        >
          {open ? <ChevronDownIcon className="h-4 w-4" /> : <ChevronRightIcon className="h-4 w-4" />}
        </Button>
        {report.autonomous ? <MoonIcon className="h-4 w-4 text-muted" /> : null}
        <h3 className="m-0 text-sm font-medium">
          {report.autonomous ? 'Rapport de la nuit' : 'Rapport de la dernière file'}
        </h3>
        <span className="text-xs text-muted" title={report.reason}>
          {formatRelative(report.finishedAt)}
        </span>
        {badges.length > 0 ? (
          <div className="flex flex-wrap items-center gap-1.5">
            {badges.map((b) => (
              <Badge key={b.label} tone={b.tone}>
                {b.label}
              </Badge>
            ))}
          </div>
        ) : null}
        <div className="grow" />
        <Button
          variant="ghost"
          size="sm"
          iconOnly
          title="Marquer comme lu"
          aria-label="Marquer comme lu"
          onClick={() => void onDismiss()}
        >
          <CheckIcon className="h-4 w-4" />
        </Button>
      </div>

      {open ? (
        <div className="mt-2 flex max-h-[32rem] flex-col overflow-y-auto border-t border-border">
          {report.ideas.map((idea, i) => {
            const sid = idea.sessionId;
            return (
              <div
                key={idea.id}
                className={`flex flex-wrap items-center gap-x-3 gap-y-1 py-2 ${
                  i > 0 ? 'border-t border-border' : ''
                }`}
              >
                <span className="w-16 shrink-0">
                  <IdeaStatusBadge status={idea.status} />
                </span>
                <span className="min-w-0 flex-1 truncate text-sm text-text" title={idea.text}>
                  {idea.title}
                </span>
                <div className="flex flex-wrap items-center gap-1">
                  <IdeaMeta idea={idea} caps={caps} />
                </div>
                {sid ? (
                  <Button
                    variant="ghost"
                    size="sm"
                    iconOnly
                    title="Relire"
                    aria-label="Relire"
                    onClick={() => onOpenSession(sid)}
                  >
                    <ReviewIcon className="h-4 w-4" />
                  </Button>
                ) : null}
              </div>
            );
          })}
        </div>
      ) : null}
    </Card>
  );
}
