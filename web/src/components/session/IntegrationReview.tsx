// What the integration dialog shows of a branch before the human decides: its commits, what its
// files signal, its criteria and the decisions the agent recorded.
import type { IntegrationPreview, ReviewSignal } from '../../lib/api';
import { DecisionList } from './DecisionList';
import { CriterionProvenanceBadge, ExistenceOnlyBadge, PassedBeforeBadge, criterionDotClass } from './CriteriaSection';
import { ChevronRightIcon } from '../ui/Icons';
import { Button } from '../ui/Button';
import { Badge } from '../ui/Badge';
import type { BadgeTone } from '../ui/Badge';
import { SectionLabel } from '../ui/SectionLabel';
import { useT } from '../../i18n';
import type { MsgKey } from '../../i18n/messages';

// `protected` has its own badge, shown before these.
export const SIGNAL_KEYS: Record<Exclude<ReviewSignal, 'protected'>, { label: MsgKey; tone: BadgeTone; title: MsgKey }> = {
  sensitive: {
    label: 'signalSensitiveLabel',
    tone: 'err',
    title: 'signalSensitiveTitle'
  },
  outsidePlan: {
    label: 'signalOutsidePlanLabel',
    tone: 'attention',
    title: 'signalOutsidePlanTitle'
  },
  bypass: {
    label: 'signalBypassLabel',
    tone: 'err',
    title: 'signalBypassTitle'
  },
  architecture: {
    label: 'signalArchitectureLabel',
    tone: 'err',
    title: 'signalArchitectureTitle'
  },
  conflictMarker: {
    label: 'signalConflictLabel',
    tone: 'err',
    title: 'signalConflictTitle'
  },
  testsWeakened: {
    label: 'signalTestsLabel',
    tone: 'err',
    title: 'signalTestsTitle'
  },
  hot: {
    label: 'signalHotLabel',
    tone: 'accent',
    title: 'signalHotTitle'
  },
  untested: {
    label: 'signalUntestedLabel',
    tone: 'accent',
    title: 'signalUntestedTitle'
  },
  large: {
    label: 'signalLargeLabel',
    tone: 'neutral',
    title: 'signalLargeTitle'
  }
};


interface IntegrationReviewProps {
  preview: IntegrationPreview;
  /** The project the narrate agent runs in; without one the Raconter button is not offered. */
  projectId: string | null;
  narrating: boolean;
  onNarrate: () => void;
  /** While a comprehension dialogue is being opened; the Understand button is disabled then. */
  understandingBusy: boolean;
  onUnderstand: () => void;
}

export function IntegrationReview({ preview, projectId, narrating, onNarrate, understandingBusy, onUnderstand }: IntegrationReviewProps) {
  const t = useT();
  const signals = [...new Set(preview.files.flatMap((file) => file.signals))].filter(
    (signal): signal is Exclude<ReviewSignal, 'protected'> => signal !== 'protected',
  );
  const carried = preview.files.some((file) => file.carried);
  return (
    <>
      <section className="flex flex-col gap-1.5">
        <SectionLabel>{t('reviewCommits')}</SectionLabel>
        {preview.commits.length === 0 ? (
          <div className="text-xs text-muted">{t('noCommitsPlain')}</div>
        ) : (
          <details className="group text-xs">
            <summary className="flex cursor-pointer list-none items-center gap-2 rounded-md px-1.5 py-1 select-none text-muted hover:bg-accent-soft hover:text-text [&::-webkit-details-marker]:hidden">
              <ChevronRightIcon className="h-3 w-3 shrink-0 transition-transform group-open:rotate-90" />
              {preview.commits.length === 1
                ? t('commitOneTaken')
                : t('commitsSquashed', { count: preview.commits.length })}
            </summary>
            <div className="flex flex-col pl-6">
              {preview.commits.map((commit) => (
                <div key={commit.sha} className="flex items-start gap-2 py-1 text-xs">
                  <span className="shrink-0 font-mono text-muted">{commit.sha.slice(0, 8)}</span>
                  <span className="grow break-words text-text">{commit.subject}</span>
                </div>
              ))}
            </div>
          </details>
        )}
      </section>

      <section className="flex flex-col gap-1.5">
        <SectionLabel className="flex items-center">
          {t('filesLabel')}
          {preview.files.length > 0 && (
            <span className="ml-1 rounded border border-border bg-raised px-1.5 text-[11px] tabular-nums text-text">
              {preview.files.length}
            </span>
          )}
          {preview.files.length > 0 && (
            <>
              <Badge tone="ok" className="ml-1.5" title={t('verifiedFromLog')}>
                {t('verified')}
              </Badge>
            </>
          )}
          {preview.files.length > 0 && projectId && (
            <Button
              variant="secondary"
              size="sm"
              className="ml-auto"
              title={t('narrateTitle')}
              disabled={narrating}
              onClick={onNarrate}
            >
              {t('narrate')}
            </Button>
          )}
          {preview.files.length > 0 && projectId && preview.understanding.state !== 'none' && (
            <Badge
              tone={preview.understanding.state === 'held' ? 'ok' : 'attention'}
              className="ml-1.5"
              title={t('verifiedFromLog')}
            >
              {preview.understanding.state === 'held' ? t('understandingHeld') : t('understandingStarted')}
            </Badge>
          )}
          {preview.files.length > 0 && projectId && (
            <Button
              variant="secondary"
              size="sm"
              className="ml-1.5"
              title={t('understandTitle')}
              disabled={understandingBusy}
              onClick={onUnderstand}
            >
              {preview.understanding.state === 'none' ? t('understand') : t('understandResume')}
            </Button>
          )}
        </SectionLabel>
        {preview.files.length === 0 ? (
          <div className="text-xs text-muted">{t('noFilesPlain')}</div>
        ) : (
          // The files themselves are listed in the side panel; the dialog keeps what the branch's
          // files signal, so the review's warnings are seen before the decision.
          (signals.length > 0 || carried) && (
            <div className="flex flex-wrap items-center gap-1.5 text-xs">
              {carried && (
                <Badge tone="accent" title={t('carriedTitle')}>
                  {t('carriedBadge')}
                </Badge>
              )}
              {signals.map((signal) => (
                <Badge key={signal} tone={SIGNAL_KEYS[signal].tone} title={t(SIGNAL_KEYS[signal].title)}>
                  {t(SIGNAL_KEYS[signal].label)}
                </Badge>
              ))}
            </div>
          )
        )}
      </section>

      <section className="flex flex-col gap-1.5">
        <SectionLabel>{t('panelCriteria')}</SectionLabel>
        {preview.criteria.length === 0 ? (
          <div className="text-xs text-muted">{t('noCriteriaPlain')}</div>
        ) : (
          <div className="flex flex-col">
            {preview.criteria.map((criterion) => {
              const provenance = criterion.status.type === 'pending' ? undefined : criterion.status.provenance;
              const dotClass = criterionDotClass(criterion.status.type, provenance);
              return (
                <div key={criterion.id} className="flex items-start gap-2 py-1.5 text-xs">
                  <span aria-hidden className={`mt-1 h-2 w-2 shrink-0 rounded-full ${dotClass}`} />
                  <span className="shrink-0 font-mono text-muted">{criterion.id}</span>
                  <span className="grow break-words text-text">{criterion.description}</span>
                  <PassedBeforeBadge passedBefore={criterion.passedBefore} />
                  <ExistenceOnlyBadge existenceOnly={criterion.existenceOnly} />
                  <CriterionProvenanceBadge provenance={provenance} />
                </div>
              );
            })}
          </div>
        )}
      </section>

      <section className="flex flex-col gap-1.5">
        <SectionLabel>{t('panelDecisions')}</SectionLabel>
        <DecisionList decisions={preview.decisions} />
      </section>
    </>
  );
}
