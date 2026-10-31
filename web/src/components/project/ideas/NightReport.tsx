/**
 * The night report banner: what the autonomous queue did while the human slept.
 */
import { Badge } from '../../ui/Badge';
import { Button } from '../../ui/Button';
import { CheckIcon, EyeIcon } from '../../Icons';
import { Card } from '../../ui/Card';
import { formatRelative } from '../../../lib/format';
import { IdeaStatusBadge } from './IdeaStatusBadge';
import { IdeaMeta } from './IdeaMeta';
import type { IdeaBoard } from '../../../lib/ideas';

type Report = NonNullable<IdeaBoard['nightReport']>;

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
  // Only non-zero summary counts become badges.
  const badges: { label: string; tone: 'accent' | 'ok' | 'err' }[] = [];
  if (report.summary.review > 0)
    badges.push({ label: `${report.summary.review} à relire`, tone: 'accent' });
  if (report.summary.question > 0)
    badges.push({ label: `${report.summary.question} question`, tone: 'accent' });
  if (report.summary.halted > 0)
    badges.push({ label: `${report.summary.halted} arrêtée`, tone: 'err' });
  if (report.summary.failed > 0)
    badges.push({ label: `${report.summary.failed} échouée`, tone: 'err' });
  if (report.summary.integrated > 0)
    badges.push({ label: `${report.summary.integrated} intégrées`, tone: 'ok' });
  if (report.summary.running > 0)
    badges.push({ label: `${report.summary.running} en cours`, tone: 'accent' });

  return (
    <Card className="flex flex-col gap-3 p-4">
      <div className="flex items-center gap-2">
        <h3 className="m-0 text-sm font-semibold">
          {report.autonomous ? 'Rapport de la nuit' : 'Rapport de la dernière file'}
        </h3>
        <span className="text-xs text-muted">{formatRelative(report.finishedAt)}</span>
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

      <p className="m-0 text-xs text-muted">{report.reason}</p>

      {badges.length > 0 ? (
        <div className="flex flex-wrap gap-1.5">
          {badges.map((b) => (
            <Badge key={b.label} tone={b.tone}>
              {b.label}
            </Badge>
          ))}
        </div>
      ) : null}

      <div className="flex flex-col">
        {report.ideas.map((idea, i) => {
          const sid = idea.sessionId;
          return (
            <div
              key={idea.id}
              className={`flex items-start gap-2 py-2 text-xs ${
                i > 0 ? 'border-t border-border' : ''
              }`}
            >
              <div className="min-w-0 grow">
                <div className="truncate text-sm" title={idea.text}>
                  {idea.text}
                </div>
                <div className="mt-1 flex flex-wrap items-center gap-2">
                  <IdeaStatusBadge status={idea.status} />
                  <IdeaMeta idea={idea} caps={caps} />
                </div>
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
                  <EyeIcon className="h-4 w-4" />
                </Button>
              ) : null}
            </div>
          );
        })}
      </div>
    </Card>
  );
}
