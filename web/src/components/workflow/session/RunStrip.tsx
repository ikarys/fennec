import { useEffect, useRef, useState, type PointerEvent } from 'react';
import { portTone, type RunView } from '../../../lib/workflowRun';
import { CheckIcon, CloseIcon } from '../../Icons';
import { Badge } from '../../ui/Badge';
import { Button } from '../../ui/Button';

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

function outcomeTone(outcome: string): 'ok' | 'err' | 'neutral' {
  switch (outcome) {
    case 'done':
      return 'ok';
    case 'blocked':
    case 'stopped':
    case 'error':
      return 'err';
    default:
      return 'neutral';
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
  // Which sides hide chips: the strip has no scrollbar, so a fade and a grab
  // cursor tell the human there is more to see.
  const [hidden, setHidden] = useState({ left: false, right: false });
  const drag = useRef<{ x: number; scrollLeft: number } | null>(null);

  const measure = () => {
    const element = chips.current;
    if (!element) return;
    setHidden({
      left: element.scrollLeft > 1,
      right: element.scrollLeft + element.clientWidth < element.scrollWidth - 1
    });
  };

  // The node running now is the last chip: keep it in view as the run grows.
  useEffect(() => {
    const element = chips.current;
    if (element) element.scrollLeft = element.scrollWidth;
    measure();
  }, [count]);

  useEffect(() => {
    window.addEventListener('resize', measure);
    return () => window.removeEventListener('resize', measure);
  }, []);

  const startDrag = (event: PointerEvent<HTMLDivElement>) => {
    drag.current = { x: event.clientX, scrollLeft: event.currentTarget.scrollLeft };
    event.currentTarget.setPointerCapture(event.pointerId);
  };
  const moveDrag = (event: PointerEvent<HTMLDivElement>) => {
    if (drag.current) event.currentTarget.scrollLeft = drag.current.scrollLeft - (event.clientX - drag.current.x);
  };
  const scrollable = hidden.left || hidden.right;
  const mask = `linear-gradient(to right, ${hidden.left ? 'transparent' : 'black'}, black 2rem, black calc(100% - 2rem), ${hidden.right ? 'transparent' : 'black'})`;

  return (
    <div className="border-b border-border bg-panel px-6 py-2">
      <div className="mx-auto flex max-w-[820px] items-center gap-3">
        <span className="shrink-0 whitespace-nowrap text-xs font-medium text-text">{run.workflow.name}</span>
        <div
          ref={chips}
          onScroll={measure}
          onWheel={(event) => {
            if (event.deltaX === 0) event.currentTarget.scrollLeft += event.deltaY;
          }}
          onPointerDown={startDrag}
          onPointerMove={moveDrag}
          onPointerUp={() => (drag.current = null)}
          onPointerCancel={() => (drag.current = null)}
          style={{ maskImage: mask }}
          className={`flex min-w-0 flex-1 select-none items-center gap-1.5 overflow-x-auto py-0.5 [scrollbar-width:none] ${
            scrollable ? 'cursor-grab active:cursor-grabbing' : ''
          }`}
        >
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
          <Badge tone={outcomeTone(run.finished.outcome)} className="shrink-0">
            {outcomeLabel(run.finished.outcome)}
          </Badge>
        )}
        {active && !running && (
          <Button type="button" variant="danger" size="sm" onClick={onStop} className="shrink-0">
            Arrêter le workflow
          </Button>
        )}
      </div>
    </div>
  );
}
