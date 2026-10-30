import type { RunView } from "../../lib/workflowRun";

/** Above the composer, where the eyes are during a run: says a turn is going on, and which node. */
export function RunningStrip({ run }: { run: RunView | undefined }) {
  const node =
    run && !run.finished
      ? [...run.runs].reverse().find((r) => r.state === "running")
      : undefined;
  return (
    <div
      className="mx-auto flex w-full max-w-[960px] items-center gap-2 px-1 pb-1.5 text-xs text-muted"
      role="status"
    >
      <span className="h-2 w-2 shrink-0 animate-pulse rounded-full bg-accent" />
      {node ? (
        <>
          <span className="truncate">
            Workflow en cours · <span className="text-text">{node.label}</span>
            {node.pass > 1 && ` (passe ${node.pass})`}
          </span>
          {run?.pauseRequested && (
            <span className="ml-auto shrink-0 text-text">
              Pause demandée · arrêt après cette étape
            </span>
          )}
        </>
      ) : (
        <span>L'agent travaille…</span>
      )}
    </div>
  );
}
