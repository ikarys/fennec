/**
 * The idea's status badge: one table shared by the board and the night report.
 */
import { Badge, type BadgeTone } from '../../ui/Badge';
import type { IdeaStatus } from '../../../lib/ideas';

const STATUS_CONFIG: Record<
  IdeaStatus,
  { label: string; tone: BadgeTone; pulse?: boolean }
> = {
  inbox: { label: 'idée', tone: 'neutral' },
  framing: { label: 'en cadrage', tone: 'info' },
  queued: { label: 'prête', tone: 'info' },
  running: { label: 'en cours', tone: 'accent', pulse: true },
  question: { label: 'question', tone: 'accent' },
  review: { label: 'à relire', tone: 'accent' },
  halted: { label: 'arrêtée', tone: 'err' },
  failed: { label: 'échouée', tone: 'err' },
  integrated: { label: 'intégrée', tone: 'ok' },
  completed: { label: 'faite', tone: 'ok' },
};

export function IdeaStatusBadge({ status }: { status: IdeaStatus }) {
  const cfg = STATUS_CONFIG[status];
  return (
    <Badge tone={cfg.tone} pill dot pulse={cfg.pulse ?? false} title={`Statut : ${cfg.label}`}>
      {cfg.label}
    </Badge>
  );
}
