/**
 * One idea in the board: title, the rest of its text, status, meta, one
 * primary action, and a ⋯ menu. Ready ideas use it too, numbered and
 * reorderable, with their framing session one click away.
 */
import { useState } from "react";
import { Button } from "../../ui/Button";
import {
  ChatIcon,
  ReviewIcon,
  EyeIcon,
  GripIcon,
  PlayIcon,
  ReplyIcon,
} from "../../ui/Icons";
import { IdeaStatusBadge } from "./IdeaStatusBadge";
import { IdeaMeta } from "./IdeaMeta";
import { IdeaMenu } from "./IdeaMenu";
import { formatDate } from "../../../lib/format";
import {
  startIdea,
  type IdeaAction,
  type IdeaBoard,
  type IdeaView,
} from "../../../lib/ideas";
import type { MsgKey } from "../../../i18n/messages";
import { useT } from "../../../i18n";

const ACTION_LABELS: Record<IdeaAction, MsgKey> = {
  frame: "actionFrame",
  resumeFraming: "actionResumeFraming",
  start: "actionStart",
  open: "actionOpen",
  answer: "actionAnswer",
  review: "actionReview",
};

const ACTION_ICONS: Record<IdeaAction, typeof ChatIcon> = {
  frame: ChatIcon,
  resumeFraming: ChatIcon,
  start: PlayIcon,
  open: EyeIcon,
  answer: ReplyIcon,
  review: ReviewIcon,
};

export type RowReorder = {
  onDragStart: () => void;
  onDragOver: () => void;
  onDrop: () => void;
  onDragEnd: () => void;
  onKeyMove: (direction: -1 | 1) => void;
  dragging: boolean;
  over: boolean;
};

export function IdeaRow({
  idea,
  projectId,
  onFrame,
  onOpenSession,
  run,
  caps,
  position,
  reorder,
}: {
  idea: IdeaView;
  projectId: string;
  onFrame: (idea: IdeaView) => Promise<void>;
  onOpenSession: (sessionId: string) => void;
  run: (action: () => Promise<IdeaBoard>) => Promise<boolean>;
  caps: IdeaBoard["queue"]["caps"];
  position: number | null;
  reorder: RowReorder | null;
}) {
  const [busy, setBusy] = useState(false);
  const t = useT();

  const showMeta =
    idea.durationMs != null ||
    idea.tokens != null ||
    idea.criteria != null ||
    idea.decisions.length > 0 ||
    idea.commits != null;

  const handleStart = async () => {
    setBusy(true);
    try {
      await run(() => startIdea(projectId, idea.id));
    } finally {
      setBusy(false);
    }
  };

  const handleFrame = async () => {
    setBusy(true);
    try {
      await onFrame(idea);
    } finally {
      setBusy(false);
    }
  };

  // The one primary action the server chose for this idea.
  const renderAction = () => {
    const onClick =
      idea.action === "frame"
        ? () => void handleFrame()
        : idea.action === "start"
          ? () => void handleStart()
          : idea.sessionId
            ? () => onOpenSession(idea.sessionId as string)
            : null;
    if (!onClick || !idea.action) return null;
    const Icon = ACTION_ICONS[idea.action];
    const actionLabel = t(ACTION_LABELS[idea.action]);
    return (
      <Button
        variant="soft"
        size="sm"
        iconOnly
        disabled={busy}
        title={actionLabel}
        aria-label={actionLabel}
        onClick={onClick}
      >
        <Icon className="h-4 w-4" />
      </Button>
    );
  };

  // The primary action already opens the session for these.
  const opensSession =
    idea.action === "open" ||
    idea.action === "answer" ||
    idea.action === "review";

  return (
    <div
      className={[
        "relative flex items-start gap-2 px-3 py-2 hover:bg-raised",
        reorder?.dragging ? "opacity-50" : null,
        reorder?.over ? "rounded-md ring-1 ring-accent" : null,
      ]
        .filter(Boolean)
        .join(" ")}
      draggable={reorder ? true : undefined}
      onDragStart={
        reorder
          ? (e) => {
              e.dataTransfer.effectAllowed = "move";
              // Firefox starts no drag without data.
              e.dataTransfer.setData("text/plain", idea.id);
              reorder.onDragStart();
            }
          : undefined
      }
      onDragOver={
        reorder
          ? (e) => {
              e.preventDefault();
              reorder.onDragOver();
            }
          : undefined
      }
      onDrop={
        reorder
          ? (e) => {
              e.preventDefault();
              reorder.onDrop();
            }
          : undefined
      }
      onDragEnd={reorder ? reorder.onDragEnd : undefined}
    >
      {position != null ? (
        <span className="w-5 shrink-0 text-right font-mono text-xs text-muted">
          {position}
        </span>
      ) : null}

      {reorder ? (
        <Button
          variant="ghost"
          size="sm"
          iconOnly
          title={t('reorderTitle')}
          aria-label={t('reorder')}
          className="cursor-grab"
          onKeyDown={(e) => {
            if (e.key === "ArrowUp") {
              e.preventDefault();
              reorder.onKeyMove(-1);
            } else if (e.key === "ArrowDown") {
              e.preventDefault();
              reorder.onKeyMove(1);
            }
          }}
        >
          <GripIcon className="h-4 w-4" />
        </Button>
      ) : null}

      <div className="min-w-0 grow">
        <p className="m-0 truncate text-sm text-text" title={idea.text}>
          {idea.title}
        </p>
        {idea.halted ? (
          <p className="mt-0.5 text-xs text-muted">{idea.halted}</p>
        ) : null}
        <div className="mt-1 flex flex-wrap items-center gap-1.5 text-xs">
          <IdeaStatusBadge status={idea.status} />
          {showMeta ? <IdeaMeta idea={idea} caps={caps} /> : null}
        </div>
      </div>

      <span className="shrink-0 font-mono text-xs text-faint" title={t('createdOn')}>
        {formatDate(idea.createdAt)}
      </span>

      <div className="flex shrink-0 items-center gap-1">
        {idea.sessionId && !opensSession ? (
          <Button
            variant="ghost"
            size="sm"
            iconOnly
            title={t('openFramingSession')}
            aria-label={t('openFramingSession')}
            onClick={() => onOpenSession(idea.sessionId as string)}
          >
            <EyeIcon className="h-4 w-4" />
          </Button>
        ) : null}
        {renderAction()}
        <IdeaMenu
          idea={idea}
          projectId={projectId}
          onFrame={onFrame}
          run={run}
        />
      </div>
    </div>
  );
}
