import type { AssistantItem } from '../../lib/fold';
import { formatSpeed } from '../../lib/format';
import { useCallUsage } from './CallUsage';
import { useT } from '../../i18n';

/** Under each answer: the live speed while it streams, then what the server measured. */
export function MessageStatsLine({ item, streaming }: { item: AssistantItem; streaming: boolean }) {
  const t = useT();
  if (streaming) {
    if (item.liveSpeed == null) return null;
    return (
      <div className="text-xs text-muted tabular-nums" title={t('speedEstimated')}>
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
    <div className="min-w-0 overflow-hidden text-xs whitespace-nowrap text-muted tabular-nums" title={call.detail}>
      {call.line}
    </div>
  );
}
