// The workflow run's left side rail: a SideRail with step dots and a run timeline drawer.
import { useMemo } from 'react';
import { upcomingSteps, type RunView } from '../../../lib/workflowRun';
import { useT } from '../../../i18n';
import { BranchIcon } from '../../ui/Icons';
import { SideRail, type RailPanel } from '../../ui/SideRail';
import { RunTimeline } from './RunTimeline';
import { STEP_DOT_CLASS, nodeRunTone } from './runStepTone';

function StepDots({ run }: { run: RunView }) {
  const t = useT();
  const upcoming = useMemo(() => upcomingSteps(run), [run]);

  return (
    <div className="flex flex-col items-center gap-2 py-3">
      {run.runs.map((nodeRun, index) => (
        <span
          key={`${nodeRun.nodeId}-${nodeRun.pass}-${index}`}
          title={nodeRun.state === 'interrupted' ? t('runRailInterrupted', { label: nodeRun.label }) : `${nodeRun.label} — ${nodeRun.port || nodeRun.state}`}
          className={`h-2 w-2 shrink-0 rounded-full ${STEP_DOT_CLASS[nodeRunTone(nodeRun)]}`}
        />
      ))}
      {upcoming.map((step) => (
        <span key={`upcoming-${step.nodeId}`} title={t('runRailUpcoming', { label: step.label })} className="h-2 w-2 shrink-0 rounded-full border border-dashed border-faint" />
      ))}
    </div>
  );
}

export function RunRail({ run }: { run: RunView }) {
  const t = useT();
  const upcoming = useMemo(() => upcomingSteps(run), [run]);

  const finishedCount = run.runs.filter((nodeRun) => nodeRun.state === 'done' || nodeRun.state === 'ended').length;
  const openCount = run.runs.filter((nodeRun) => nodeRun.state === 'running').length;
  const anyHumanRunning = run.runs.some((nodeRun) => nodeRun.state === 'running' && nodeRun.kind === 'human');
  const anyErr = run.runs.some((nodeRun) => nodeRunTone(nodeRun) === 'err');
  const outcome = run.finished?.outcome;

  let statusClass: string | undefined;
  let alert = false;

  if (outcome === 'blocked' || outcome === 'error' || anyErr) {
    statusClass = 'text-err';
    alert = true;
  } else if (outcome === 'done') {
    statusClass = 'text-ok';
  } else if (openCount > 0) {
    statusClass = 'text-accent';
  }

  if (anyHumanRunning) alert = true;

  const runPanel: RailPanel = {
    id: 'run',
    label: t('runRailRun'),
    title: run.workflow.name,
    Icon: BranchIcon,
    status: `${finishedCount}/${finishedCount + openCount + upcoming.length}`,
    statusClass,
    alert,
    body: <RunTimeline run={run} />,
  };

  return (
    <SideRail
      side="left"
      label={t('runRailWorkflow')}
      storageKey="fennec.runRail"
      defaultPinned
      width="w-[300px]"
      panels={[runPanel]}
      footer={<StepDots run={run} />}
    />
  );
}
