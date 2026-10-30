/**
 * The night report banner: what the autonomous queue did while the human slept.
 */
import { Badge } from '../../ui/Badge';
import { Button } from '../../ui/Button';
import { Card } from '../../ui/Card';
import { formatRelative } from '../../../lib/format';
import { IdeaStatusBadge } from './IdeaStatusBadge';
import { IdeaMeta } from './IdeaMeta';
import type { IdeaBoard } from '../../../lib/ideas';

type Report = NonNullable<IdeaBoard['nightReport']>;

export function NightReport({
  report,
  onOpenSession,
  onDismiss,
}: {
  report: Report;
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
        <Button variant="ghost" size="sm" onClick={() => void onDismiss()}>
          Marquer comme lu
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
              className={`flex items-center gap-2 py-1.5 text-xs ${
                i > 0 ? 'border-t border-border' : ''
              }`}
            >
              <span className="min-w-0 grow truncate" title={idea.text}>
                {idea.text}
              </span>
              <IdeaStatusBadge status={idea.status} />
              <IdeaMeta idea={idea} />
              {sid ? (
                <Button variant="ghost" size="sm" onClick={() => onOpenSession(sid)}>
                  Relire
                </Button>
              ) : null}
            </div>
          );
        })}
      </div>
    </Card>
  );
}
