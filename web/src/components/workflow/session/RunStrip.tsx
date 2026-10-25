import type { RunView } from '../../../lib/workflowRun';

function outcomeLabel(outcome: string): string {
  switch (outcome) {
    case 'done':
      return 'terminé';
    case 'blocked':
      return 'bloqué';
    case 'stopped':
      return 'arrêté';
    case 'error':
      return 'en erreur';
    default:
      return outcome;
  }
}

export function RunStrip({
  run,
  running,
  onStop
}: {
  run: RunView;
  running: boolean;
  onStop: () => void;
}) {
  const active = !run.finished;

  return (
    <div className="border-b border-border bg-panel px-6 py-2">
      <div className="mx-auto flex max-w-[820px] items-center gap-3">
        <span className="text-xs font-medium text-text">{run.workflow.name}</span>
        <div className="flex flex-1 items-center gap-1.5 overflow-x-auto">
          {run.runs.map((nodeRun, index) => (
            <span
              key={`${nodeRun.nodeId}-${nodeRun.pass}-${index}`}
              className={`flex shrink-0 items-center gap-1 rounded-full border px-2 py-0.5 text-[11px] ${
                nodeRun.state === 'running'
                  ? 'border-accent bg-accent-soft text-accent-strong'
                  : nodeRun.state === 'done'
                    ? 'border-border bg-raised text-text'
                    : 'border-border bg-transparent text-muted'
              }`}
            >
              {nodeRun.state === 'running' && <span className="h-1.5 w-1.5 animate-pulse rounded-full bg-accent" />}
              {nodeRun.label}
              {nodeRun.state === 'done' && nodeRun.port && (
                <span className="text-[10px] text-muted">{nodeRun.port}</span>
              )}
            </span>
          ))}
        </div>
        {run.finished && (
          <span className="text-xs text-muted">{outcomeLabel(run.finished.outcome)}</span>
        )}
        {active && !running && (
          <button
            type="button"
            onClick={onStop}
            className="shrink-0 rounded-lg border border-border bg-raised px-2.5 py-1 text-xs text-text hover:text-err"
          >
            Arrêter le workflow
          </button>
        )}
      </div>
    </div>
  );
}
