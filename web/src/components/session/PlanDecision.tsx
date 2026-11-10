import { useState } from 'react';
import type { Plan } from '../../lib/plan';
import type { PreflightResultView } from '../../lib/fold';
import { PlanBody, IDEA_LABELS, type IdeaButton } from './PlanCard';
import { DecisionRow } from './DecisionBar';
import { ArrowUpIcon, DecisionIcon, EditIcon, PlayIcon, StarIcon } from '../ui/Icons';
import { Button } from '../ui/Button';
import { Menu, MenuItem } from '../ui/Menu';
import { Segmented } from '../ui/Segmented';
import { useT } from '../../i18n';

export interface PlanDecisionActions {
  onBuild: (worktree: boolean) => void;
  onBuildAlone: () => void;
  onRevise: () => void;
  onShowInThread: () => void;
  /** False when a launch cannot open a worktree (no default workflow, a chat): it runs in the human's folder. */
  worktreeAvailable: boolean;
  ideaReady?: { onMark: () => void; idea: IdeaButton };
  disabled?: boolean;
}

export function PlanDecision({
  plan,
  preflight,
  actions,
  open,
  onToggle
}: {
  plan: Plan;
  preflight?: PreflightResultView[];
  actions: PlanDecisionActions;
  open: boolean;
  onToggle: () => void;
}) {
  const t = useT();
  const [worktreeChosen, setWorktree] = useState(true);
  const worktree = actions.worktreeAvailable && worktreeChosen;

  const buildButton = (
    <Button
      variant="primary"
      size="sm"
      onClick={() => actions.onBuild(worktree)}
      disabled={actions.disabled}
      title={t('launchPlanTitle')}
    >
      <PlayIcon className="h-3 w-3" />
      {t('launch')}
    </Button>
  );

  return (
    <DecisionRow
      icon={<DecisionIcon className="h-3.5 w-3.5" />}
      title={plan.title}
      meta={`${t('planBadge')} · ${t('planDecisionMeta', { steps: plan.steps.length, plural: plan.steps.length > 1 ? 's' : '', criteria: plan.doneWhen.length })}`}
      primary={buildButton}
      open={open}
      onToggle={onToggle}
      danger={!worktree}
      footer={
        <>
          <span className="text-xs text-muted">{t('decisionWhere')}</span>
          <Segmented
            label={t('decisionWhere')}
            value={worktree ? 'worktree' : 'checkout'}
            onChange={(value) => setWorktree(value === 'worktree')}
            disabled={actions.disabled || !actions.worktreeAvailable}
            segments={[
              { value: 'worktree', label: t('decisionWorktree'), title: t('decisionWorktreeTitle') },
              { value: 'checkout', label: t('decisionCheckout'), title: t('decisionCheckoutTitle') }
            ]}
          />
          <Button variant="ghost" size="sm" onClick={actions.onShowInThread}>
            <ArrowUpIcon className="h-3 w-3" />
            {t('decisionShowInThread')}
          </Button>
          <span className="flex-1" />
          <Button
            variant="secondary"
            size="sm"
            onClick={actions.onRevise}
            disabled={actions.disabled}
            title={t('revisePlanTitle')}
          >
            <EditIcon className="h-3.5 w-3.5" />
            {t('revise')}
          </Button>
          {actions.ideaReady && (
            <Button
              variant="secondary"
              size="sm"
              onClick={actions.ideaReady.onMark}
              disabled={
                actions.disabled || actions.ideaReady.idea === 'saved' || actions.ideaReady.idea === 'applied'
              }
              title={t(
                actions.ideaReady.idea === 'sheet' || actions.ideaReady.idea === 'applied'
                  ? 'applyToSheetTitle'
                  : 'saveToBacklogTitle',
              )}
            >
              <StarIcon className="h-3.5 w-3.5" />
              {t(IDEA_LABELS[actions.ideaReady.idea])}
            </Button>
          )}
          {buildButton}
          {!actions.disabled && (
            <Menu label={t('otherLaunchesLabel')}>
              <MenuItem onSelect={actions.onBuildAlone}>{t('buildAlone')}</MenuItem>
            </Menu>
          )}
        </>
      }
    >
      <PlanBody plan={plan} preflight={preflight} />
    </DecisionRow>
  );
}
