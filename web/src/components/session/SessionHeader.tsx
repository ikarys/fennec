// The session's header: title, state, project and date, the chat-link and canvas actions, and the context gauge.
import type { ContextView, Project, SessionDetail } from '../../lib/api';
import type { ThreadView } from '../../lib/fold';
import { formatDateTime } from '../../lib/format';
import type { sessionStatus } from '../../lib/sessionStatus';
import { useLang, useT } from '../../i18n';
import { SessionCanvasMenu } from '../canvas/SessionCanvasMenu';
import { Badge } from '../ui/Badge';
import { Button } from '../ui/Button';
import { CloseIcon } from '../ui/Icons';
import { ChatLinkPicker } from './ChatLinkPicker';
import { ContextGauge } from './ContextGauge';

interface SessionHeaderProps {
  title: string;
  status: ReturnType<typeof sessionStatus>;
  sessionId: string | null;
  isChat: boolean;
  projectName: string;
  detail: SessionDetail | null;
  projects: Project[];
  view: ThreadView;
  context: ContextView | null;
  compactDisabled: boolean;
  onCompact: () => void;
  compactError: string | null;
  onOpenCanvas: (artifactId: number) => void;
  onOpenContext: () => void;
  /** The chat link changed. */
  onRenamed: () => void;
  /** Set in split view: closes this pane. */
  onClose?: () => void;
}

export function SessionHeader({
  title,
  status,
  sessionId,
  isChat,
  projectName,
  detail,
  projects,
  view,
  context,
  compactDisabled,
  onCompact,
  compactError,
  onOpenCanvas,
  onOpenContext,
  onRenamed,
  onClose,
}: SessionHeaderProps) {
  const lang = useLang();
  const t = useT();
  return (
    /* A shadow under the title: the thread fades under it instead of meeting a hard edge. */
    <div className="relative z-10 flex items-center gap-2 px-4 md:px-6 py-2.5 after:pointer-events-none after:absolute after:inset-x-0 after:top-full after:h-5 after:bg-linear-to-b after:from-black/45 after:to-transparent">
      <div className="mx-auto flex w-full max-w-page items-center gap-2">
        {/* Title and state first; project and date on a quieter line below: the header stays light. */}
        <div className="min-w-0">
          <div className="flex min-w-0 items-center gap-2">
            <h1
              className="min-w-0 truncate text-base font-semibold"
              title={title}
            >
              {title}
            </h1>
            <Badge
              tone={status.tone}
              appearance="pill"
              dot={status.live}
              pulse={status.live}
            >
              {status.label}
            </Badge>
          </div>
          <div className="flex min-w-0 items-center gap-1.5 text-xs text-muted">
            <span
              className="truncate"
              title={isChat ? undefined : detail?.project.workdir}
            >
              {isChat ? t('sessionPageChat') : projectName}
            </span>
            {detail && (
              <>
                <span aria-hidden>·</span>
                <span
                  className="shrink-0 tabular-nums"
                  title={t('sessionPageCreatedTitle')}
                >
                  {formatDateTime(lang, detail.createdAt)}
                </span>
              </>
            )}
          </div>
        </div>
        <div className="ml-auto flex shrink-0 items-center gap-1">
          {sessionId && isChat && detail && (
            <ChatLinkPicker
              sessionId={sessionId}
              linkedProjectId={detail.linkedProjectId}
              projects={projects}
              onLinked={onRenamed}
            />
          )}
          {sessionId && (
            <SessionCanvasMenu sessionId={sessionId} refreshKey={view} onOpen={onOpenCanvas} />
          )}
        </div>
        <span aria-hidden className="mx-2 h-5 w-px shrink-0 bg-border" />
        <ContextGauge
          context={context}
          compactDisabled={compactDisabled}
          onCompact={onCompact}
          compactError={compactError}
          onOpen={onOpenContext}
        />
        {onClose && (
          <Button
            variant="ghost"
            size="sm"
            iconOnly
            className="ml-1"
            title={t('splitClosePane')}
            aria-label={t('splitClosePane')}
            onClick={onClose}
          >
            <CloseIcon className="h-3.5 w-3.5" />
          </Button>
        )}
      </div>
    </div>
  );
}
