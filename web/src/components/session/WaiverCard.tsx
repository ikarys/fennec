/**
 * The pending protected-file waiver. It is rendered above the composer so the
 * human can grant a time-limited waiver or refuse it.
 */
import { useState } from 'react';
import type { PendingWaiver } from '../../lib/api/sessions';
import { shortPath } from '../../lib/shortPath';
import { Button } from '../ui/Button';
import { Card } from '../ui/Card';
import { Segmented } from '../ui/Segmented';
import { useT } from '../../i18n';

export function WaiverCard({
  waiver,
  onAnswer,
  disabled
}: {
  waiver: PendingWaiver;
  onAnswer: (hours: number | null) => void;
  disabled: boolean;
}) {
  const t = useT();
  const [hours, setHours] = useState<number>(1);

  return (
    <div className="px-6 py-2">
      <Card className="mx-auto max-w-[820px] px-3 py-2">
        <div className="flex flex-wrap items-center gap-2">
          <span className="text-accent">!</span>
          <span className="text-muted">{t('waiver')}</span>
          <span className="min-w-0 truncate font-mono text-sm text-text" title={waiver.path}>
            {shortPath(waiver.path)}
          </span>
          <div className="ml-auto flex flex-wrap items-center gap-1.5">
            <Segmented
              label={t('waiverDuration')}
              value={hours}
              onChange={setHours}
              disabled={disabled}
              segments={[
                { value: 1, label: '1 h' },
                { value: 8, label: '8 h' },
                { value: 24, label: '24 h' }
              ]}
            />
            <Button
              type="button"
              variant="primary"
              size="sm"
              disabled={disabled}
              title={t('waiverGrantTitle')}
              onClick={() => onAnswer(hours)}
            >
              {t('grant')}
            </Button>
            <Button
              type="button"
              variant="danger"
              size="sm"
              disabled={disabled}
              onClick={() => onAnswer(null)}
            >
              {t('refuse')}
            </Button>
          </div>
        </div>
        {waiver.reason ? (
          <div className="mt-1.5 text-xs text-muted line-clamp-2" title={waiver.reason}>
            {t('waiverReason', { reason: waiver.reason })}
          </div>
        ) : null}
      </Card>
    </div>
  );
}
