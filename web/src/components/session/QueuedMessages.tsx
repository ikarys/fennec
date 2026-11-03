/**
 * Messages queued while a turn runs. Rendered above the composer so the human
 * can see what will be dropped into the turn, and remove one.
 */
import type { QueuedMessage } from '../../lib/api/sessions';
import { CloseIcon } from '../Icons';
import { Badge } from '../ui/Badge';
import { Button } from '../ui/Button';

export function QueuedMessages({
  queued,
  onCancel
}: {
  queued: QueuedMessage[];
  onCancel: (queueId: string) => void;
}) {
  if (queued.length === 0) return null;

  return (
    <div className="px-6 py-2">
      <div className="mx-auto flex max-w-[960px] flex-col gap-1.5">
        {queued.map((message) => (
          <div
            key={message.queueId}
            className="flex items-center gap-2 rounded-lg border border-border bg-panel px-2.5 py-1.5"
          >
            <Badge>En file</Badge>
            <span className="min-w-0 truncate text-sm text-text" title={message.content}>
              {message.content}
            </span>
            <Button
              variant="ghost"
              iconOnly
              aria-label="Retirer de la file"
              onClick={() => onCancel(message.queueId)}
              className="ml-auto"
            >
              <CloseIcon className="h-3.5 w-3.5" />
            </Button>
          </div>
        ))}
      </div>
    </div>
  );
}
