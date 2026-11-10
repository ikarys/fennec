/**
 * The right-side drawer of one task: its sheet and framing for a task, the
 * Run / Turn into a task choice for a raw idea. Writes go through the board's
 * run() path; the server decides completeness.
 */
import { useState } from 'react';
import { Drawer } from '../../ui/Drawer';
import { Button } from '../../ui/Button';
import { Tabs } from '../../ui/Tabs';
import { ChatIcon, EditIcon } from '../../ui/Icons';
import { IdeaStatusBadge } from './IdeaStatusBadge';
import { FIELD_LABELS } from './SheetSection';
import { SheetTab } from './SheetTab';
import { FramingTab } from './FramingTab';
import { RawIdeaPanel } from './RawIdeaPanel';
import { markSheetReady, type IdeaBoard, type IdeaView } from '../../../lib/ideas';
import { useT } from '../../../i18n';

type DrawerTab = 'sheet' | 'framing';

export function TaskDrawer({
  idea,
  projectId,
  run,
  onFrame,
  onOpenSession,
  onClose,
}: {
  idea: IdeaView;
  projectId: string;
  run: (action: () => Promise<IdeaBoard>) => Promise<boolean>;
  onFrame: (idea: IdeaView) => Promise<void>;
  onOpenSession: (sessionId: string) => void;
  onClose: () => void;
}) {
  const t = useT();
  const [tab, setTab] = useState<DrawerTab>('sheet');
  const [busy, setBusy] = useState(false);
  // The server refuses sheet writes once the task is past the queue.
  const editable = idea.status === 'inbox' || idea.status === 'framing' || idea.status === 'queued';
  const canReady = idea.sheet !== null && (idea.status === 'inbox' || idea.status === 'framing');

  const ready = async () => {
    setBusy(true);
    await run(() => markSheetReady(projectId, idea.id));
    setBusy(false);
  };

  const actions = (
    <>
      <IdeaStatusBadge status={idea.status} />
      {idea.completeness ? (
        <span className="text-xs text-muted tabular-nums">
          {t('sheetCounts', { required: idea.completeness.required, suggested: idea.completeness.suggested })}
        </span>
      ) : null}
      {canReady ? (
        <Button
          variant="primary"
          size="sm"
          disabled={busy || idea.missing.length > 0}
          title={
            idea.missing.length > 0
              ? t('sheetMissing', { fields: idea.missing.map((f) => t(FIELD_LABELS[f])).join(', ') })
              : t('sheetReadyTitle')
          }
          onClick={() => void ready()}
        >
          {t('sheetReady')}
        </Button>
      ) : null}
    </>
  );

  return (
    <Drawer open label={idea.title} title={idea.title} actions={actions} onClose={onClose}>
      {idea.sheet ? (
        <div className="flex flex-col gap-4">
          <Tabs
            label={t('taskDrawerTabs')}
            orientation="horizontal"
            tabs={[
              { id: 'sheet', label: t('sheetTab'), Icon: EditIcon },
              { id: 'framing', label: t('framingTab'), Icon: ChatIcon },
            ]}
            active={tab}
            onSelect={setTab}
          />
          {tab === 'sheet' ? (
            <SheetTab idea={idea} sheet={idea.sheet} projectId={projectId} run={run} editable={editable} />
          ) : (
            <FramingTab idea={idea} onFrame={onFrame} onOpenSession={onOpenSession} />
          )}
        </div>
      ) : (
        <RawIdeaPanel idea={idea} projectId={projectId} run={run} />
      )}
    </Drawer>
  );
}
