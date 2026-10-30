/**
 * The queue's main control: ready count, next idea, night mode, caps, start/stop.
 */
import { useState } from 'react';
import { Button } from '../../ui/Button';
import { Card } from '../../ui/Card';
import { Badge } from '../../ui/Badge';
import { EditIcon, MoonIcon, PlayIcon, SettingsIcon, StopIcon } from '../../Icons';
import { formatTokens } from '../../../lib/format';
import { startQueue, stopQueue, type IdeaBoard } from '../../../lib/ideas';

export function QueueBar({
  queue,
  projectId,
  run,
}: {
  queue: IdeaBoard['queue'];
  projectId: string;
  run: (action: () => Promise<IdeaBoard>) => Promise<boolean>;
}) {
  const [autonomous, setAutonomous] = useState(false);
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

  // Show only the caps that are set; 0 on both means one "sans limite".
  const capParts: string[] = [];
  if (queue.caps.maxMinutes > 0) capParts.push(`max ${queue.caps.maxMinutes} min`);
  if (queue.caps.maxTokens > 0)
    capParts.push(`${formatTokens(queue.caps.maxTokens)} tokens`);
  const caps = capParts.length === 0 ? 'sans limite' : capParts.join(' · ');

  if (queue.running) {
    return (
      <Card className="flex flex-col gap-2 px-4 py-3">
        <div className="flex items-center gap-3">
          <span className="flex items-center gap-1.5 text-sm font-medium text-text">
            <Badge tone="accent" dot pulse>
              file en cours
            </Badge>
            {queue.autonomous ? (
              <>
                <MoonIcon className="h-3.5 w-3.5 text-muted" />
                <span className="text-xs text-muted">mode nuit · {caps} par idée</span>
              </>
            ) : null}
          </span>
          <Button
            variant="secondary"
            size="sm"
            disabled={busy}
            iconOnly
            title="Arrêter la file"
            aria-label="Arrêter la file"
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
        <span className="text-sm font-medium text-text tabular-nums">
          {queue.ready} {queue.ready === 0 || queue.ready === 1 ? 'prête' : 'prêtes'}
        </span>
        {queue.next ? (
          <span className="min-w-0 truncate text-xs text-muted" title={queue.next.text}>
            Prochaine : {queue.next.text}
          </span>
        ) : null}

        <div className="ml-auto flex shrink-0 items-center gap-2">
          <label className="flex cursor-pointer items-center gap-1.5 text-xs text-text">
            <input
              type="checkbox"
              checked={autonomous}
              onChange={(e) => setAutonomous(e.target.checked)}
              className="h-4 w-4 accent-[var(--accent)]"
            />
            <MoonIcon className="h-3.5 w-3.5" />
            Mode nuit
          </label>

          <Button
            variant="ghost"
            size="sm"
            iconOnly
            title="Plafonds du mode nuit (Paramètres)"
            aria-label="Plafonds du mode nuit (Paramètres)"
            onClick={() => {
              window.location.hash = '#/settings';
            }}
          >
            <SettingsIcon className="h-4 w-4" />
          </Button>

          {queue.ready > 0 ? (
            <Button variant="primary" size="md" disabled={busy} onClick={handleStart}>
              <PlayIcon className="h-4 w-4" />
              Lancer la file
            </Button>
          ) : null}
        </div>
      </div>

      {queue.ready === 0 ? (
        <p className="m-0 flex items-center gap-1 text-xs text-muted">
          Rien n'est prêt : cadre une idée du backlog avec
          <EditIcon className="h-3.5 w-3.5" />
          pour l'ajouter à la file.
        </p>
      ) : autonomous ? (
        <p className="m-0 text-xs text-muted">
          {caps === 'sans limite'
            ? "L'agent tranche seul ses questions et note chaque décision ; une idée n'a pas de plafond et la file passe à la suivante."
            : `L'agent tranche seul ses questions et note chaque décision ; une idée s'arrête à ${caps} et la file passe à la suivante.`}
        </p>
      ) : null}
    </Card>
  );
}
