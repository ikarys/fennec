/**
 * The queue control bar: start/stop the file, night-mode toggle, caps display.
 */
import { useState } from 'react';
import { Button } from '../../ui/Button';
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
      <div className="flex items-center gap-3">
        <span className="text-sm text-text">
          File en cours{queue.autonomous ? ' · nuit' : ''}
        </span>
        <Button variant="secondary" size="sm" disabled={busy} onClick={handleStop}>
          Arrêter
        </Button>
      </div>
    );
  }

  return (
    <div className="flex items-center gap-3">
      <span className="text-sm text-muted">{queue.ready} prêtes</span>
      {queue.next ? (
        <span className="max-w-[200px] truncate text-xs text-muted" title={queue.next.text}>
          {queue.next.text}
        </span>
      ) : null}

      <label className="flex items-center gap-1.5 text-xs text-muted">
        <input
          type="checkbox"
          checked={autonomous}
          onChange={(e) => setAutonomous(e.target.checked)}
          className="h-3.5 w-3.5 accent-[var(--accent)]"
        />
        ☾ Nuit
      </label>

      <span className="text-xs text-faint">{caps}</span>
      <a
        href="#/settings"
        className="text-xs text-muted hover:text-text"
        aria-label="Réglages"
      >
        ⚙
      </a>

      <Button
        variant="primary"
        size="sm"
        disabled={queue.ready === 0 || busy}
        onClick={handleStart}
      >
        ▶ Lancer la file
      </Button>
    </div>
  );
}
