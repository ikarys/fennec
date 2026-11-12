/**
 * The queue's main control: ready count, next idea, night mode, caps, start/stop.
 */
import { useState } from 'react';
import { Button } from '../../ui/Button';
import { Card } from '../../ui/Card';
import { Badge } from '../../ui/Badge';
import { EditIcon, MoonIcon, PlayIcon, SettingsIcon, StopIcon } from '../../ui/Icons';
import { formatTokens } from '../../../lib/format';
import { startQueue, stopQueue, type IdeaBoard } from '../../../lib/ideas';
import { useT, useLang } from '../../../i18n';

export function QueueBar({
  queue,
  projectId,
  run,
}: {
  queue: IdeaBoard['queue'];
  projectId: string;
  run: (action: () => Promise<IdeaBoard>) => Promise<boolean>;
}) {
  const lang = useLang();
  const t = useT();
  const [autonomous, setAutonomous] = useState(queue.lastAutonomous);
  const [busy, setBusy] = useState(false);

  const handleStart = async () => {
    setBusy(true);
    try {
      await run(() => startQueue(projectId, autonomous));
    } finally {
      setBusy(false);
    }
  };

  const handleStop = async () => {
    setBusy(true);
    try {
      await run(() => stopQueue(projectId));
    } finally {
      setBusy(false);
    }
  };

  // Show only the caps that are set; 0 on both means one "no limit".
  const capParts: string[] = [];
  if (queue.caps.maxMinutes > 0) capParts.push(`max ${queue.caps.maxMinutes} min`);
  if (queue.caps.maxTokens > 0)
    capParts.push(`${formatTokens(lang, queue.caps.maxTokens)} tokens`);
  const caps = capParts.length === 0 ? t('noCap') : capParts.join(' · ');

  if (queue.running) {
    return (
      <Card className="flex flex-col gap-2 px-4 py-3">
        <div className="flex items-center gap-3">
          <span className="flex items-center gap-1.5 text-sm font-medium text-text">
            <Badge tone="accent" dot pulse>
              {t('queueRunning')}
            </Badge>
            {queue.autonomous ? (
              <>
                <MoonIcon className="h-3.5 w-3.5 text-muted" />
                <span className="text-xs text-muted">{t('nightModeCaps', { caps })}</span>
              </>
            ) : null}
          </span>
          <Button
            variant="secondary"
            size="sm"
            disabled={busy}
            iconOnly
            title={t('stopQueueAfter')}
            aria-label={t('stopQueue')}
            className="ml-auto"
            onClick={handleStop}
          >
            <StopIcon className="h-4 w-4" />
          </Button>
        </div>
      </Card>
    );
  }

  return (
    <Card className="flex flex-col gap-2 px-4 py-3">
      <div className="flex items-center gap-3">
        <span className="text-sm font-medium text-text tabular-nums whitespace-nowrap">
          {t('readyCount', { count: queue.ready })}
        </span>
        {queue.next ? (
          <span className="min-w-0 truncate text-xs text-muted" title={queue.next.text}>
            {t('nextLabel', { title: queue.next.title })}
          </span>
        ) : null}

        <div className="ml-auto flex shrink-0 items-center gap-2">
          <label className="flex cursor-pointer items-center gap-1.5 text-xs text-text">
            <input
              type="checkbox"
              checked={autonomous}
              onChange={(e) => setAutonomous(e.target.checked)}
              className="h-4 w-4"
            />
            <MoonIcon className="h-3.5 w-3.5" />
            {t('nightMode')}
          </label>

          <Button
            variant="ghost"
            size="sm"
            iconOnly
            title={t('nightCapsTitle')}
            aria-label={t('nightCapsTitle')}
            onClick={() => {
              window.location.hash = '#/settings/autonomy';
            }}
          >
            <SettingsIcon className="h-4 w-4" />
          </Button>

          {/* Always shown, disabled when empty, so the view's main action never vanishes. */}
          <Button
            variant="primary"
            size="md"
            disabled={busy || queue.ready === 0}
            title={queue.ready === 0 ? t('noReadyIdea') : undefined}
            onClick={handleStart}
          >
            <PlayIcon className="h-4 w-4" />
            {t('startQueue')}
          </Button>
        </div>
      </div>

      {queue.ready === 0 ? (
        <p className="m-0 flex items-center gap-1 text-xs text-muted">
          {t('nothingReadyPrefix')}
          <EditIcon className="h-3.5 w-3.5" aria-hidden />
          <strong className="font-medium text-text">{t('turnIntoTask')}</strong>
          {t('nothingReadySuffix')}
        </p>
      ) : autonomous ? (
        <p className="m-0 text-xs text-muted">
          {caps === t('noCap') ? t('autonomousNoCap') : t('autonomousWithCap', { caps })}
        </p>
      ) : null}
    </Card>
  );
}
