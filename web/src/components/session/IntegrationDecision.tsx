/**
 * The last run ended with work on the agent's branch, a row of the decision
 * bar: it offers trying the work or opening the review.
 */
import { DecisionRow } from './DecisionBar';
import { CommitIcon } from '../ui/Icons';
import { useT } from '../../i18n';
import { TryOrIntegrate } from './TryOrIntegrate';
import type { IntegrationInvite } from './IntegrationInvite';

export function IntegrationDecision({
  invite,
  open,
  onToggle,
}: {
  invite: IntegrationInvite;
  open: boolean;
  onToggle: () => void;
}) {
  const t = useT();

  const actions = <TryOrIntegrate invite={invite} primary="integrate" />;

  return (
    <DecisionRow
      icon={<CommitIcon className="h-3.5 w-3.5" />}
      title={t('runEndWorkReady')}
      meta={t('integrationBadge')}
      primary={actions}
      open={open}
      onToggle={onToggle}
      footer={
        <>
          <span className="flex-1" />
          {actions}
        </>
      }
    />
  );
}
