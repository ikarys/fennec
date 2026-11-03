import type { CriterionView, Provenance } from '../../lib/fold';
import { CheckIcon } from '../Icons';
import type { BadgeTone } from '../ui/Badge';
import { Badge } from '../ui/Badge';
import { Panel } from '../ui/Panel';
import { useT } from '../../i18n';

const DOT_CLASSES = {
  ok: { full: 'bg-ok', hollow: 'border-2 border-ok bg-transparent' },
  err: { full: 'bg-err', hollow: 'border-2 border-err bg-transparent' },
  muted: { full: 'bg-muted', hollow: 'border-2 border-muted bg-transparent' },
} as const;

/** A full dot for a fact fennec established, a hollow one for an agent's judgement. */
export function criterionDotClass(status: CriterionView['status'], provenance: Provenance | undefined): string {
  const tone = status === 'passed' ? 'ok' : status === 'failed' ? 'err' : 'muted';
  return provenance?.kind === 'indicative' ? DOT_CLASSES[tone].hollow : DOT_CLASSES[tone].full;
}

export function PassedBeforeBadge({ passedBefore }: { passedBefore: boolean | undefined }) {
  const t = useT();
  if (!passedBefore) return null;
  return <Badge tone="neutral" title={t('passedBeforeTitle')}>{t('passedAlready')}</Badge>;
}

export function CriterionProvenanceBadge({ provenance }: { provenance: Provenance | undefined }) {
  const t = useT();
  if (!provenance) return null;
  const badge =
    provenance.kind === 'verified'
      ? {
          label: t('verified'),
          title: t('verifiedTitle'),
          tone: 'ok' as const
        }
      : {
          label: t('indicative'),
          title: t('indicativeTitle', { agent: provenance.agent }),
          tone: 'accent' as const
        };
  return <Badge tone={badge.tone} title={badge.title}>{badge.label}</Badge>;
}

export function CriteriaSection({ criteria }: { criteria: CriterionView[] }) {
  const t = useT();
  const passed = criteria.filter((criterion) => criterion.status === 'passed').length;
  const failed = criteria.filter((criterion) => criterion.status === 'failed').length;
  const indicative = criteria.filter((criterion) => criterion.status === 'passed' && criterion.provenance?.kind === 'indicative').length;
  // The header's icon reports the block's state: red when a check failed, green when all pass.
  const iconTone: BadgeTone = failed > 0 ? 'err' : criteria.length > 0 && passed === criteria.length ? 'ok' : 'neutral';

  return (
    <Panel
      id="criteria"
      title={t('panelCriteria')}
      icon={<CheckIcon className="h-3.5 w-3.5" />}
      iconTone={iconTone}
      summary={
        criteria.length > 0 ? (
          <span
            className={failed > 0 ? 'text-err' : passed === criteria.length ? 'text-ok' : undefined}
            title={indicative > 0 ? t('indicativeCount', { passed, total: criteria.length, indicative }) : undefined}
          >
            {passed}/{criteria.length}
          </span>
        ) : undefined
      }
    >
      {criteria.length === 0 ? (
        <div className="text-xs text-muted">{t('noCriteriaHint')}</div>
      ) : (
        <div className="flex flex-col">
          <div className="h-1 w-full overflow-hidden rounded-full bg-raised">
            <div className="h-full rounded-full bg-ok" style={{ width: `${(passed / criteria.length) * 100}%` }} />
          </div>
          {criteria.map((criterion) => {
            const provenance = criterion.provenance;
            const dotClass = criterionDotClass(criterion.status, provenance);

            return (
              <details key={criterion.id}>
                <summary className="group flex cursor-pointer list-none items-start gap-2 rounded-md px-1 py-1.5 text-xs select-none hover:bg-accent-soft">
                  <span aria-hidden className={`mt-1 h-2 w-2 shrink-0 rounded-full ${dotClass}`} />
                  <span className="shrink-0 font-mono text-muted">{criterion.id}</span>
                  <span className="grow break-words text-text group-hover:text-accent-strong">{criterion.description}</span>
                  <PassedBeforeBadge passedBefore={criterion.passedBefore} />
                  <CriterionProvenanceBadge provenance={provenance} />
                </summary>
                {(provenance || (criterion.status === 'pending' && criterion.check)) && (
                  <div className="flex flex-col gap-1.5 pb-1 pl-4 pr-1 text-xs">
                    {provenance && provenance.kind === 'verified' && (
                      <>
                        <div className="font-mono text-muted">$ {provenance.command}</div>
                        {typeof provenance.exitCode === 'number' && (
                          <div className="text-xs text-muted">{t('exitCode', { code: provenance.exitCode })}</div>
                        )}
                        <pre className="max-h-40 overflow-auto rounded bg-code p-2 font-mono text-xs text-muted">
                          {provenance.output}
                        </pre>
                      </>
                    )}
                    {provenance && provenance.kind === 'indicative' && (
                      <div className="break-words text-text">{provenance.reason}</div>
                    )}
                    {criterion.status === 'pending' && criterion.check && (
                      <div className="font-mono text-muted">$ {criterion.check}</div>
                    )}
                  </div>
                )}
              </details>
            );
          })}
        </div>
      )}
    </Panel>
  );
}
