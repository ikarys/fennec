import type { RunView } from "../../lib/workflowRun";
import { Button } from "../ui/Button";

/** Above the composer, where the eyes are: holds the auto-scroll switch, always; during a run,
 * also says a turn is going on, and which node. */
export function RunningStrip({
  running,
  run,
  following,
  onFollowChange,
}: {
  running: boolean;
  run: RunView | undefined;
  following: boolean;
  onFollowChange: (on: boolean) => void;
}) {
  const node =
    run && !run.finished
      ? [...run.runs].reverse().find((r) => r.state === "running")
      : undefined;
  return (
    <div
      className="mx-auto flex w-full max-w-[960px] items-center gap-2 px-1 pb-1.5 text-xs text-muted"
      role="status"
    >
      {running && (
        <>
          <span className="h-2 w-2 shrink-0 animate-pulse rounded-full bg-accent" />
          {node ? (
            <>
              <span className="truncate">
                Workflow en cours · <span className="text-text">{node.label}</span>
                {node.pass > 1 && ` (passe ${node.pass})`}
              </span>
              {run?.pauseRequested && (
                <span className="shrink-0 text-text">
                  Pause demandée · arrêt après cette étape
                </span>
              )}
            </>
          ) : (
            <span>L'agent travaille…</span>
          )}
        </>
      )}
      <Button
        variant={following ? "secondary" : "ghost"}
        size="sm"
        className="ml-auto shrink-0 tabular-nums"
        aria-pressed={following}
        title={
          following
            ? "Défilement auto activé : le fil suit l'agent. Cliquer pour le couper."
            : "Défilement auto coupé. Cliquer pour aller en bas et suivre l'agent."
        }
        onClick={() => onFollowChange(!following)}
      >
        Autoscroll : <span className={following ? "text-accent" : ""}>{following ? "ON" : "OFF"}</span>
      </Button>
    </div>
  );
}
