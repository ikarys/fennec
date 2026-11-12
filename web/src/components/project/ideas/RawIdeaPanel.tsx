/**
 * The drawer of an idea without a sheet: its text, and the two ways forward,
 * run it as a prompt or turn it into a task to frame.
 */
import { useState } from 'react';
import { Button } from '../../ui/Button';
import { Card } from '../../ui/Card';
import { EditIcon, PlayIcon } from '../../ui/Icons';
import { startIdea, turnIntoTask, type IdeaBoard, type IdeaView } from '../../../lib/ideas';
import { useT } from '../../../i18n';

export function RawIdeaPanel({
  idea,
  projectId,
  run,
}: {
  idea: IdeaView;
  projectId: string;
  run: (action: () => Promise<IdeaBoard>) => Promise<boolean>;
}) {
  const t = useT();
  const [busy, setBusy] = useState(false);
  const open = idea.status === 'inbox' || idea.status === 'framing';

  const go = async (action: () => Promise<IdeaBoard>) => {
    setBusy(true);
    await run(action);
    setBusy(false);
  };

  return (
    <div className="flex flex-col gap-3">
      <Card variant="inset" className="whitespace-pre-wrap text-sm">
        {idea.text}
      </Card>
      <p className="m-0 text-sm text-muted">{t('rawIdeaHint')}</p>
      <div className="flex items-center gap-2">
        <Button variant="secondary" size="sm" disabled={busy || !open} onClick={() => void go(() => startIdea(projectId, idea.id))}>
          <PlayIcon className="h-4 w-4" />
          {t('runIdea')}
        </Button>
        <Button variant="secondary" size="sm" disabled={busy || !open} onClick={() => void go(() => turnIntoTask(projectId, idea.id))}>
          <EditIcon className="h-4 w-4" />
          {t('turnIntoTask')}
        </Button>
      </div>
    </div>
  );
}
