/**
 * One stage of the tasks board: a labelled group of idea rows.
 */
import { useState, type ReactNode } from "react";
import { Card } from "../../ui/Card";
import { SectionLabel } from "../../ui/SectionLabel";
import { IdeaRow } from "./IdeaRow";
import {
  reorderIdeas,
  type IdeaBoard,
  type IdeaView,
  type Stage,
} from "../../../lib/ideas";

const STAGE_LABELS: Record<Stage, string> = {
  yours: "À décider",
  running: "En cours",
  queue: "File",
  drafts: "Idées",
  done: "Terminées",
};

/** The highlighted stages' card already frames them; the other lists get one. */
function ListFrame({
  highlighted,
  spaced,
  children,
}: {
  highlighted: boolean;
  spaced: boolean;
  children: ReactNode;
}) {
  return highlighted ? (
    <div className="flex flex-col gap-1">{children}</div>
  ) : (
    <Card
      className={`flex flex-col divide-y divide-border overflow-hidden ${spaced ? "mt-2" : ""}`}
    >
      {children}
    </Card>
  );
}

export function StageGroup({
  stage,
  ideas,
  projectId,
  onFrame,
  onOpenSession,
  run,
  caps,
  showLabel = true,
}: {
  stage: Stage;
  ideas: IdeaView[];
  projectId: string;
  onFrame: (idea: IdeaView) => Promise<void>;
  onOpenSession: (sessionId: string) => void;
  run: (action: () => Promise<IdeaBoard>) => Promise<boolean>;
  caps: IdeaBoard["queue"]["caps"];
  showLabel?: boolean;
}) {
  const [dragFrom, setDragFrom] = useState<number | null>(null);
  const [dragOver, setDragOver] = useState<number | null>(null);

  const move = async (from: number, to: number) => {
    if (
      from === to ||
      from < 0 ||
      from >= ideas.length ||
      to < 0 ||
      to >= ideas.length
    ) {
      return;
    }
    const ids = ideas.map((i) => i.id);
    const [item] = ids.splice(from, 1);
    ids.splice(to, 0, item);
    await run(() => reorderIdeas(projectId, ids));
  };

  // The stages that need the human stand apart; the others are one list.
  const highlighted = stage === "yours" || stage === "running";

  const content = (
    <>
      {showLabel ? (
        <SectionLabel as="h2">{STAGE_LABELS[stage]}</SectionLabel>
      ) : null}
      <ListFrame highlighted={highlighted} spaced={showLabel}>
        {stage === "queue" && ideas.length === 0 ? (
          <p className="m-0 px-3 py-2 text-sm text-muted">
            Aucune tâche prête : cadre une idée du backlog.
          </p>
        ) : null}
        {ideas.map((idea, index) => {
          const reorder =
            stage === "queue" && ideas.length > 1
              ? {
                  onDragStart: () => setDragFrom(index),
                  onDragOver: () => setDragOver(index),
                  onDrop: () => {
                    if (dragFrom != null) void move(dragFrom, index);
                    setDragFrom(null);
                    setDragOver(null);
                  },
                  onDragEnd: () => {
                    setDragFrom(null);
                    setDragOver(null);
                  },
                  onKeyMove: (d: -1 | 1) => void move(index, index + d),
                  dragging: dragFrom === index,
                  over:
                    dragOver === index &&
                    dragFrom !== null &&
                    dragFrom !== index,
                }
              : null;
          return (
            <IdeaRow
              key={idea.id}
              idea={idea}
              projectId={projectId}
              onFrame={onFrame}
              onOpenSession={onOpenSession}
              run={run}
              caps={caps}
              position={stage === "queue" ? idea.position : null}
              reorder={reorder}
            />
          );
        })}
      </ListFrame>
    </>
  );

  return highlighted ? (
    <section>
      <Card className="flex flex-col gap-2 p-3">{content}</Card>
    </section>
  ) : (
    <section>{content}</section>
  );
}
