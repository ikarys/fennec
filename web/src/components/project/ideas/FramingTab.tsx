/**
 * The drawer's framing tab: framing happens with the agent in its session, so
 * this tab only starts it or opens it.
 */
import { useState } from 'react';
import { Button } from '../../ui/Button';
import { ChatIcon, EyeIcon } from '../../ui/Icons';
import type { IdeaView } from '../../../lib/ideas';
import { useT } from '../../../i18n';

export function FramingTab({
  idea,
  onFrame,
  onOpenSession,
}: {
  idea: IdeaView;
  onFrame: (idea: IdeaView) => Promise<void>;
  onOpenSession: (sessionId: string) => void;
}) {
  const t = useT();
  const [busy, setBusy] = useState(false);
  const sessionId = idea.sessionId;

  const frame = async () => {
    setBusy(true);
    try {
      await onFrame(idea);
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="flex flex-col items-start gap-3">
      <p className="m-0 text-sm text-muted">{t('framingTabHint')}</p>
      {sessionId ? (
        <Button variant="secondary" size="sm" onClick={() => onOpenSession(sessionId)}>
          <EyeIcon className="h-4 w-4" />
          {t('openFramingSession')}
        </Button>
      ) : (
        <Button variant="secondary" size="sm" disabled={busy} onClick={() => void frame()}>
          <ChatIcon className="h-4 w-4" />
          {t('frameWithAgent')}
        </Button>
      )}
    </div>
  );
}
