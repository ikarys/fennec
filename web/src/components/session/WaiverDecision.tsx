/**
 * The pending protected-file waiver. It is a row of the decision bar so the
 * human can grant a time-limited waiver or refuse it.
 */
import { useState } from 'react';
import type { PendingWaiver } from '../../lib/api/sessions';
import { shortPath } from '../../lib/shortPath';
import { DecisionRow } from './DecisionBar';
import { AlertIcon } from '../ui/Icons';
import { Button } from '../ui/Button';
import { Segmented } from '../ui/Segmented';
import { useT } from '../../i18n';

export function WaiverDecision({
  waiver,
  onAnswer,
  disabled,
  open,
  onToggle
}: {
  waiver: PendingWaiver;
  onAnswer: (hours: number | null) => void;
  disabled: boolean;
  open: boolean;
  onToggle: () => void;
}) {
  const t = useT();
  const [hours, setHours] = useState<number>(1);

  return (
    <DecisionRow
      icon={<AlertIcon className="h-3.5 w-3.5" />}
      title={<span className="font-mono">{shortPath(waiver.path)}</span>}
      meta={t('waiver')}
      primary={
        <Button
          type="button"
          variant="primary"
          size="sm"
          disabled={disabled}
          title={t('waiverGrantTitle')}
          onClick={() => onAnswer(hours)}
        >
          {`${t('grant')} · ${hours} h`}
        </Button>
      }
      open={open}
      onToggle={onToggle}
      footer={
        <>
          <span className="text-xs text-muted">{t('waiverDuration')}</span>
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
          <span className="flex-1" />
          <Button
            type="button"
            variant="danger"
            size="sm"
            disabled={disabled}
            onClick={() => onAnswer(null)}
          >
            {t('refuse')}
          </Button>
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
        </>
      }
    >
      <div className="font-mono text-text break-all" title={waiver.path}>
        {waiver.path}
      </div>
      {waiver.reason ? (
        <div className="mt-1.5 text-xs text-muted">{t('waiverReason', { reason: waiver.reason })}</div>
      ) : null}
    </DecisionRow>
  );
}
