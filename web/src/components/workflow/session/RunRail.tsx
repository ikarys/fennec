// The workflow run's left side rail: a SideRail whose run drawer holds the step timeline.
import { useMemo } from 'react';
import { upcomingSteps, type RunView } from '../../../lib/workflowRun';
import { useT } from '../../../i18n';
import { BranchIcon } from '../../ui/Icons';
import { SideRail, type RailPanel } from '../../ui/SideRail';
import { RunTimeline } from './RunTimeline';
import { nodeRunTone } from './runStepTone';

type JumpToNode = (runId: string, nodeId: string, pass: number) => void;

export function RunRail({ run, onJump }: { run: RunView; onJump: JumpToNode }) {
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
    body: <RunTimeline run={run} onJump={onJump} />,
  };

  return (
    <SideRail
      side="left"
      label={t('runRailWorkflow')}
      storageKey="fennec.runRail"
      defaultPinned
      width="w-[300px]"
      panels={[runPanel]}
    />
  );
}
