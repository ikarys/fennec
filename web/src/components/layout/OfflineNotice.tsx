/**
 * Full-window overlay shown when the fennec backend stream is offline.
 * It tells the user the connection is down and offers a reload.
 */
import { Button } from '../ui/Button';
import { Card } from '../ui/Card';
import { RefreshIcon, SpinnerIcon } from '../ui/Icons';
import { useT } from '../../i18n';

export function OfflineNotice() {
  const t = useT();

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-bg/85 backdrop-blur-sm"
      role="alertdialog"
      aria-live="assertive"
      aria-labelledby="offline-notice-title"
    >
      <Card className="flex max-w-md flex-col items-center gap-4 p-8 text-center">
        <div className="font-mono text-5xl font-bold text-accent">404</div>
        <img
          src="/emblem.png"
          alt=""
          className="h-36 w-36 -rotate-12 rounded-full opacity-80 grayscale"
        />
        <h2 id="offline-notice-title" className="text-lg font-semibold text-text">
          {t('offlineTitle')}
        </h2>
        <p className="text-sm text-muted">{t('offlineBody')}</p>
        <span className="flex items-center gap-2 text-xs text-muted">
          <SpinnerIcon className="h-4 w-4 animate-spin" />
          {t('offlineRetrying')}
        </span>
        <Button
          variant="icon"
          iconOnly
          title={t('offlineReload')}
          aria-label={t('offlineReload')}
          onClick={() => window.location.reload()}
        >
          <RefreshIcon className="h-4 w-4" />
        </Button>
      </Card>
    </div>
  );
}
