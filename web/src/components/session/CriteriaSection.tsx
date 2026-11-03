import type { CriterionView, Provenance } from '../../lib/fold';
import { panelBlock } from '../../lib/panelBlocks';
import { Badge } from '../ui/Badge';
import { Panel } from '../ui/Panel';

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
  if (!passedBefore) return null;
  return <Badge tone="neutral" title="Sa commande passait déjà avant tout changement : il ne prouve pas ce travail">passait déjà</Badge>;
}

export function CriterionProvenanceBadge({ provenance }: { provenance: Provenance | undefined }) {
  if (!provenance) return null;
  const badge =
    provenance.kind === 'verified'
      ? {
          label: 'vérifié',
          title: 'Fait établi par fennec : la commande a tourné',
          tone: 'ok' as const
        }
      : {
          label: 'indicatif',
          title: `Jugement d'un agent (${provenance.agent}) : à relire`,
          tone: 'accent' as const
        };
  return <Badge tone={badge.tone} title={badge.title}>{badge.label}</Badge>;
}

export function CriteriaSection({ criteria }: { criteria: CriterionView[] }) {
  const passed = criteria.filter((criterion) => criterion.status === 'passed').length;
  const indicative = criteria.filter((criterion) => criterion.status === 'passed' && criterion.provenance?.kind === 'indicative').length;

  return (
    <Panel
      id="criteria"
      title={panelBlock('criteria').title}
      summary={
        criteria.length > 0
          ? indicative > 0
            ? `${passed}/${criteria.length} · ${indicative} indicatif`
            : `${passed}/${criteria.length}`
          : undefined
      }
    >
      {criteria.length === 0 ? (
        <div className="text-xs text-muted">Aucun critère : ils viennent du plan validé d'un workflow.</div>
      ) : (
        <div className="flex flex-col">
          {criteria.map((criterion) => {
            const provenance = criterion.provenance;
            const dotClass = criterionDotClass(criterion.status, provenance);

            return (
              <details key={criterion.id}>
                <summary className="flex cursor-pointer list-none items-start gap-2 py-1.5 text-xs select-none">
                  <span aria-hidden className={`mt-1 h-2 w-2 shrink-0 rounded-full ${dotClass}`} />
                  <span className="shrink-0 font-mono text-muted">{criterion.id}</span>
                  <span className="grow break-words text-text">{criterion.description}</span>
                  <PassedBeforeBadge passedBefore={criterion.passedBefore} />
                  <CriterionProvenanceBadge provenance={provenance} />
                </summary>
                {(provenance || (criterion.status === 'pending' && criterion.check)) && (
                  <div className="flex flex-col gap-1.5 pb-1 pl-4 pr-1 text-xs">
                    {provenance && provenance.kind === 'verified' && (
                      <>
                        <div className="font-mono text-muted">$ {provenance.command}</div>
                        {typeof provenance.exitCode === 'number' && (
                          <div className="text-xs text-muted">Code de sortie : {provenance.exitCode}</div>
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
