// The scrolling thread of a session: older-page button, empty-session hint and the messages.
import type { RefObject, UIEventHandler } from 'react';
import type { ServedThread } from '../../lib/api';
import type { ThreadView } from '../../lib/fold';
import { useT } from '../../i18n';
import { Button } from '../ui/Button';
import { ThreadItemView } from './Thread';

interface SessionThreadPaneProps {
  scrollRef: RefObject<HTMLDivElement | null>;
  contentRef: RefObject<HTMLDivElement | null>;
  onScroll: UIEventHandler<HTMLDivElement>;
  sessionId: string | null;
  view: ThreadView;
  hasMore: boolean;
  loadingOlder: boolean;
  onLoadOlder: () => void;
  pendingCallId: string | undefined;
  /** The model's short name shown on its messages. */
  model: string;
  running: boolean;
  /** Ids of the items that can be replayed. */
  replayable: ServedThread['replayable'];
  onReplay: (id: string) => void;
}

export function SessionThreadPane({
  scrollRef,
  contentRef,
  onScroll,
  sessionId,
  view,
  hasMore,
  loadingOlder,
  onLoadOlder,
  pendingCallId,
  model,
  running,
  replayable,
  onReplay,
}: SessionThreadPaneProps) {
  const t = useT();
  const lastRunEndId = [...view.items].reverse().find((item) => item.kind === "workflow-end")?.id;
  return (
    /* The thread scrolls in its column, so the scrollbar sits beside it;
       a wheel over the margins still scrolls the thread. */
    <div
      className="relative min-h-0 grow"
      onWheel={(event) => {
        if (event.target === event.currentTarget) scrollRef.current?.scrollBy({ top: event.deltaY });
      }}
    >
      <div
        ref={scrollRef}
        onScroll={onScroll}
        className="mx-auto h-full max-w-[calc(var(--container-page)+3rem)] overflow-y-auto p-4 md:p-6"
      >
        <div
          ref={contentRef}
          className="mx-auto flex max-w-page flex-col gap-4.5"
        >
          {sessionId && hasMore && (
            <Button
              className="mx-auto"
              onClick={onLoadOlder}
              disabled={loadingOlder}
            >
              {loadingOlder
                ? t('loading')
                : t('sessionPageLoadOlderMessages')}
            </Button>
          )}
          {view.items.length === 0 && !sessionId && (
            <div className="py-10 text-center text-sm text-muted">
              {t('sessionPageNewSessionHint')}
            </div>
          )}
          {/* Only the last run's end card invites trying its work. */}
          {view.items.map((item, index) => (
            <ThreadItemView
              key={item.id}
              item={item}
              pendingCallId={pendingCallId}
              model={model}
              running={running}
              showHeader={
                view.items[index - 1]?.kind !== "assistant"
              }
              onReplay={sessionId && replayable.includes(item.id) ? onReplay : undefined}
              latest={item.id === lastRunEndId}
            />
          ))}
        </div>
      </div>
    </div>
  );
}
