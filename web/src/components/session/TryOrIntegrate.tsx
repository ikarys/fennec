// The two ways to take a finished run's work: try it in its worktree, or open the review.
import { Button } from '../ui/Button';
import { PlayIcon } from '../ui/Icons';
import { useT } from '../../i18n';
import type { IntegrationInvite } from './IntegrationInvite';

export function TryOrIntegrate({
  invite,
  primary,
}: {
  invite: IntegrationInvite;
  primary: 'try' | 'integrate';
}) {
  const t = useT();

  return (
    <div className="flex items-center gap-1.5">
      <Button
        variant={primary === 'try' ? 'primary' : 'secondary'}
        size="sm"
        title={t('tryChange')}
        onClick={invite.tryIt}
      >
        <PlayIcon className="h-3.5 w-3.5" />{t('tryBeforeIntegrating')}
      </Button>
      <Button
        variant={primary === 'integrate' ? 'primary' : 'secondary'}
        size="sm"
        onClick={invite.open}
      >
        {t('reviewAndIntegrate')}
      </Button>
    </div>
  );
}
