/**
 * The last run ended with work on the agent's branch, a row of the decision
 * bar: it opens the review before integration.
 */
import { Button } from '../ui/Button';
import { DecisionRow } from './DecisionBar';
import { CommitIcon } from '../ui/Icons';
import { useT } from '../../i18n';

export function IntegrationDecision({
  onOpen,
  open,
  onToggle
}: {
  onOpen: () => void;
  open: boolean;
  onToggle: () => void;
}) {
  const t = useT();

  const reviewButton = (
    <Button variant="primary" size="sm" onClick={onOpen}>
      {t('reviewAndIntegrate')}
    </Button>
  );

  return (
    <DecisionRow
      icon={<CommitIcon className="h-3.5 w-3.5" />}
      title={t('runEndWorkReady')}
      meta={t('integrationBadge')}
      primary={reviewButton}
      open={open}
      onToggle={onToggle}
      footer={
        <>
          <span className="flex-1" />
          {reviewButton}
        </>
      }
    />
  );
}
