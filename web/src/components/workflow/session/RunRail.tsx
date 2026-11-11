// The workflow run's left side rail: a SideRail whose run drawer holds the step timeline.
import { useT } from '../../../i18n';
import { nodeKindOf, type RunView } from '../../../lib/runView';
import { BranchIcon } from '../../ui/Icons';
import { SideRail, type RailPanel } from '../../ui/SideRail';
import { RunTimeline } from './RunTimeline';
import { nodeRunTone } from './runStepTone';

type JumpToNode = (runId: string, nodeId: string, pass: number) => void;

export function RunRail({ run, onJump }: { run: RunView; onJump: JumpToNode }) {
  const t = useT();

  const finishedCount = run.counts.done;
  const openCount = run.counts.open;
  const anyHumanRunning = run.runs.some((nodeRun) => nodeRun.state === 'running' && nodeKindOf(run.workflow, nodeRun.nodeId) === 'human');
  const anyErr = run.runs.some((nodeRun) => nodeRunTone(nodeRun, nodeKindOf(run.workflow, nodeRun.nodeId)) === 'err');
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
    status: `${finishedCount}/${run.counts.total}`,
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
