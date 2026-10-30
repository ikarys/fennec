/**
 * The idea's status badge: one table shared by the board and the night report.
 */
import { Badge } from '../../ui/Badge';
import type { IdeaStatus } from '../../../lib/ideas';

const STATUS_CONFIG: Record<
  IdeaStatus,
  { label: string; tone: 'neutral' | 'accent' | 'ok' | 'err'; pulse?: boolean }
> = {
  inbox: { label: 'idée', tone: 'neutral' },
  framing: { label: 'en cadrage', tone: 'neutral' },
  queued: { label: 'prête', tone: 'neutral' },
  running: { label: 'en cours', tone: 'accent', pulse: true },
  question: { label: 'question', tone: 'accent' },
  review: { label: 'à relire', tone: 'accent' },
  halted: { label: 'arrêtée', tone: 'err' },
  failed: { label: 'échouée', tone: 'err' },
  integrated: { label: 'intégrée', tone: 'ok' },
};

export function IdeaStatusBadge({ status }: { status: IdeaStatus }) {
  const cfg = STATUS_CONFIG[status];
  return (
    <Badge tone={cfg.tone} dot={cfg.pulse ?? false} pulse={cfg.pulse ?? false}>
      {cfg.label}
    </Badge>
  );
}
