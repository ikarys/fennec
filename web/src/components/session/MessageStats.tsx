import type { AssistantItem } from '../../lib/fold';
import { formatSpeed } from '../../lib/format';
import { useCallUsage } from './CallUsage';

/** Under each answer: the live speed while it streams, then what the server measured. */
export function MessageStatsLine({ item, streaming }: { item: AssistantItem; streaming: boolean }) {
  if (streaming) {
    if (item.liveSpeed == null) return null;
    return (
      <div className="text-[0.6875rem] text-muted tabular-nums" title="Estimé d'après le texte reçu, mesuré à la fin du message">
        ≈ {formatSpeed(item.liveSpeed)}
      </div>
    );
  }
  return <FinalLine messageId={item.id} />;
}

function FinalLine({ messageId }: { messageId: string }) {
  const call = useCallUsage(messageId);
  if (!call) return null;
  return (
    <div className="min-w-0 overflow-hidden text-[0.6875rem] whitespace-nowrap text-muted tabular-nums" title={call.detail}>
      {call.line}
    </div>
  );
}
