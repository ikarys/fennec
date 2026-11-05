import type { RunView } from "../../lib/workflowRun";
import { useT } from "../../i18n";

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
  const t = useT();
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
                {t("workflowRunningNode", { label: node.label })}
                {node.pass > 1 && t("passN", { n: node.pass })}
              </span>
              {run?.pauseRequested && (
                <span className="shrink-0 text-text">
                  {t("pauseRequestedStop")}
                </span>
              )}
            </>
          ) : (
            <span>{t("agentWorking")}</span>
          )}
        </>
      )}
      <label
        className="ml-auto flex shrink-0 cursor-pointer items-center gap-1.5 select-none"
        title={
          following
            ? t("followOn")
            : t("followOff")
        }
      >
        {t("autoFollow")}
        <input
          type="checkbox"
          role="switch"
          checked={following}
          onChange={(event) => onFollowChange(event.target.checked)}
          className="relative h-4 w-7 shrink-0 cursor-pointer appearance-none rounded-full bg-raised transition-colors checked:bg-accent before:absolute before:top-0.5 before:left-0.5 before:h-3 before:w-3 before:rounded-full before:bg-muted before:transition-transform checked:before:translate-x-3 checked:before:bg-on-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent-soft"
        />
      </label>
    </div>
  );
}
