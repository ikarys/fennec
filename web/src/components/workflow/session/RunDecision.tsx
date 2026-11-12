/**
 * A row of the decision bar for a workflow run that is waiting on the human.
 */
import { useState, type ReactNode } from 'react';
import { useT } from '../../../i18n';
import type { RunWaiting } from '../../../lib/runView';
import { DecisionRow } from '../../session/DecisionBar';
import { PauseIcon, StopIcon } from '../../ui/Icons';
import { Button, buttonClass } from '../../ui/Button';
import { TextArea } from '../../ui/Field';
import { splitTitle } from '../../../lib/titleLine';

export function RunDecision({
  waiting,
  onAnswer,
  onContinue,
  onResume,
  onStop,
  disabled,
  comment,
  onCommentChange,
  open,
  onToggle
}: {
  waiting: RunWaiting;
  onAnswer: (choice: string, comment: string) => Promise<void>;
  onContinue: (comment: string) => Promise<void>;
  onResume: () => Promise<void>;
  onStop: () => Promise<void>;
  disabled: boolean;
  /** Held by the page, so a canvas note can land here while the run waits. */
  comment: string;
  onCommentChange: (comment: string) => void;
  open: boolean;
  onToggle: () => void;
}) {
  const t = useT();
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const isBusy = busy || disabled;

  async function handleAction(action: () => Promise<void>) {
    if (busy || disabled) return;
    setBusy(true);
    setError(null);
    try {
      await action();
      onCommentChange('');
    } catch (err) {
      setError(err instanceof Error ? err.message : t('error'));
    } finally {
      setBusy(false);
    }
  }

  const humanChoices = waiting.kind === 'human' ? waiting.choices : [];
  const { title: humanTitle, rest: humanBody } = splitTitle(waiting.kind === 'human' ? waiting.question : '');
  const hasHumanBody = humanBody !== '';
  const firstChoice = humanChoices[0];

  const title =
    waiting.kind === 'human'
      ? humanTitle
      : waiting.kind === 'pause'
        ? t('runActionCardPausedBefore', { toLabel: waiting.toLabel })
        : waiting.kind === 'child'
          ? t('runActionCardChildAsked', { label: waiting.label })
          : waiting.kind === 'interrupted'
            ? t('runActionCardInterruptedQuestion')
            : t('runActionCardRelaunchQuestion');

  const firstChoiceButton = firstChoice !== undefined ? (
    <Button
      type="button"
      variant="primary"
      size="sm"
      disabled={isBusy}
      onClick={() => handleAction(() => onAnswer(firstChoice, comment))}
    >
      {firstChoice}
    </Button>
  ) : null;

  const continueButton = (
    <Button
      type="button"
      variant="primary"
      size="sm"
      disabled={isBusy}
      onClick={() => handleAction(() => onContinue(comment))}
    >
      {t('runActionCardContinue')}
    </Button>
  );

  const resumeButton = (
    <Button
      type="button"
      variant="primary"
      size="sm"
      disabled={isBusy}
      onClick={() => handleAction(onResume)}
    >
      {t('runActionCardResume')}
    </Button>
  );

  const relaunchButton = (
    <Button
      type="button"
      variant="primary"
      size="sm"
      disabled={isBusy}
      onClick={() => handleAction(onResume)}
    >
      {t('runActionCardRelaunch')}
    </Button>
  );

  const stopButton = (
    <Button
      type="button"
      variant="danger"
      size="sm"
      iconOnly
      disabled={isBusy}
      onClick={() => handleAction(onStop)}
      title={t('runActionCardStopWorkflowTitle')}
      aria-label={t('stop')}
    >
      <StopIcon className="h-4 w-4" />
    </Button>
  );

  const childButton = waiting.kind === 'child' ? (
    <a
      href={`#/s/${waiting.childSessionId}`}
      className={buttonClass({ variant: 'primary', size: 'sm' }, 'shrink-0')}
    >
      {t('runActionCardOpenChild')}
    </a>
  ) : null;

  const primary: ReactNode =
    waiting.kind === 'human'
      ? firstChoiceButton
      : waiting.kind === 'pause'
        ? continueButton
        : waiting.kind === 'child'
          ? childButton
          : waiting.kind === 'interrupted'
            ? resumeButton
            : relaunchButton;

  const commentField = (
    <TextArea
      rows={Math.min(6, comment.split('\n').length)}
      value={comment}
      onChange={(e) => onCommentChange(e.target.value)}
      placeholder={t('runActionCardCommentPlaceholder')}
      disabled={isBusy}
      className={hasHumanBody ? 'mt-2 w-full' : 'w-full'}
    />
  );

  const body: ReactNode =
    waiting.kind === 'human' ? (
      <>
        {hasHumanBody && (
          <div className="whitespace-pre-wrap [overflow-wrap:break-word]">{humanBody}</div>
        )}
        {commentField}
        {error && <div className="mt-1.5 text-xs text-err">{error}</div>}
      </>
    ) : waiting.kind === 'pause' ? (
      <>
        {commentField}
        {error && <div className="mt-1.5 text-xs text-err">{error}</div>}
      </>
    ) : error ? (
      <div className="mt-1.5 text-xs text-err">{error}</div>
    ) : null;

  const footer: ReactNode =
    waiting.kind === 'human' ? (
      <>
        {stopButton}
        <span className="flex-1" />
        {humanChoices
          .slice()
          .reverse()
          .map((choice, index) => {
            const originalIndex = humanChoices.length - 1 - index;
            return (
              <Button
                key={choice}
                type="button"
                variant={originalIndex === 0 ? 'primary' : 'secondary'}
                size="sm"
                disabled={isBusy}
                onClick={() => handleAction(() => onAnswer(choice, comment))}
              >
                {choice}
              </Button>
            );
          })}
      </>
    ) : waiting.kind === 'pause' ? (
      <>
        {stopButton}
        <span className="flex-1" />
        {continueButton}
      </>
    ) : waiting.kind === 'child' ? (
      <>
        {stopButton}
        <span className="flex-1" />
        {childButton}
      </>
    ) : waiting.kind === 'interrupted' ? (
      <>
        {stopButton}
        <span className="flex-1" />
        {resumeButton}
      </>
    ) : (
      <>
        <span className="flex-1" />
        {relaunchButton}
      </>
    );

  return (
    <DecisionRow
      icon={<PauseIcon className="h-3.5 w-3.5" />}
      title={title}
      meta={t('workflowBadge')}
      primary={primary}
      open={open}
      onToggle={onToggle}
      footer={footer}
    >
      {body}
    </DecisionRow>
  );
}
