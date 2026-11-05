/**
 * The idea's status badge: one table shared by the board and the night report.
 */
import { Badge, type BadgeTone } from '../../ui/Badge';
import type { IdeaStatus } from '../../../lib/ideas';
import type { MsgKey } from '../../../i18n/messages';
import { useT } from '../../../i18n';

const STATUS_CONFIG: Record<IdeaStatus, { labelKey: MsgKey; tone: BadgeTone; pulse?: boolean }> = {
  inbox: { labelKey: 'statusInbox', tone: 'neutral' },
  framing: { labelKey: 'statusFraming', tone: 'info' },
  queued: { labelKey: 'statusQueued', tone: 'info' },
  running: { labelKey: 'statusRunning', tone: 'accent', pulse: true },
  question: { labelKey: 'statusQuestion', tone: 'accent' },
  review: { labelKey: 'statusReview', tone: 'accent' },
  halted: { labelKey: 'statusHalted', tone: 'err' },
  failed: { labelKey: 'statusFailed', tone: 'err' },
  integrated: { labelKey: 'statusIntegrated', tone: 'ok' },
  completed: { labelKey: 'statusCompleted', tone: 'ok' },
};

export function IdeaStatusBadge({ status }: { status: IdeaStatus }) {
  const t = useT();
  const cfg = STATUS_CONFIG[status];
  const label = t(cfg.labelKey);
  return (
    <Badge tone={cfg.tone} pill dot pulse={cfg.pulse ?? false} title={t('statusLabel', { label })}>
      {label}
    </Badge>
  );
}
