import { useEffect, useRef } from 'react';
import { portTone, type RunView } from '../../../lib/workflowRun';
import { CheckIcon, CloseIcon } from '../../Icons';

function PortBadge({ port }: { port: string }) {
  const tone = portTone(port);
  if (tone === 'positive') {
    return (
      <span className="text-ok" title={port} aria-label={port}>
        <CheckIcon className="h-3 w-3" />
      </span>
    );
  }
  if (tone === 'negative') {
    return (
      <span className="text-err" title={port} aria-label={port}>
        <CloseIcon className="h-3 w-3" />
      </span>
    );
  }
  return <span className="text-[0.625rem] text-muted" title={port} aria-label={port}>{port}</span>;
}

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
  const chips = useRef<HTMLDivElement>(null);
  const count = run.runs.length;

  // The node running now is the last chip: keep it in view as the run grows.
  useEffect(() => {
    const element = chips.current;
    if (element) element.scrollLeft = element.scrollWidth;
  }, [count]);

  return (
    <div className="border-b border-border bg-panel px-6 py-2">
      <div className="mx-auto flex max-w-[820px] items-center gap-3">
        <span className="shrink-0 whitespace-nowrap text-xs font-medium text-text">{run.workflow.name}</span>
        <div ref={chips} className="flex min-w-0 flex-1 items-center gap-1.5 overflow-x-auto py-0.5 [scrollbar-width:none] [mask-image:linear-gradient(to_right,transparent,black_2rem)]">
          {run.runs.map((nodeRun, index) => (
            <span
              key={`${nodeRun.nodeId}-${nodeRun.pass}-${index}`}
              className={`flex shrink-0 items-center gap-1 rounded-full border px-2 py-0.5 text-[0.6875rem] ${
                nodeRun.state === 'running'
                  ? 'border-accent bg-accent-soft text-accent-strong'
                  : nodeRun.state === 'done'
                    ? 'border-border bg-raised text-text'
                    : 'border-border bg-transparent text-muted'
              }`}
            >
              {nodeRun.state === 'running' && <span className="h-1.5 w-1.5 animate-pulse rounded-full bg-accent" />}
              {nodeRun.label}
              {nodeRun.state === 'done' && nodeRun.port && <PortBadge port={nodeRun.port} />}
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
