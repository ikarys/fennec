import type { CriterionView, Provenance } from '../../lib/fold';
import { panelBlock } from '../../lib/panelBlocks';
import { Badge } from '../ui/Badge';
import { Panel } from '../ui/Panel';

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

  return (
    <Panel
      id="criteria"
      title={panelBlock('criteria').title}
      pinned
      summary={criteria.length > 0 ? `${passed}/${criteria.length}` : undefined}
    >
      {criteria.length === 0 ? (
        <div className="text-xs text-muted">Aucun critère : ils viennent du plan validé d'un workflow.</div>
      ) : (
        <div className="flex flex-col">
          {criteria.map((criterion) => {
            const dotClass =
              criterion.status === 'passed' ? 'bg-ok' :
              criterion.status === 'failed' ? 'bg-err' :
              'bg-muted';
            const provenance = criterion.provenance;

            return (
              <details key={criterion.id}>
                <summary className="flex cursor-pointer list-none items-start gap-2 py-1.5 text-xs select-none">
                  <span aria-hidden className={`mt-1 h-2 w-2 shrink-0 rounded-full ${dotClass}`} />
                  <span className="shrink-0 font-mono text-muted">{criterion.id}</span>
                  <span className="grow break-words text-text">{criterion.description}</span>
                  <CriterionProvenanceBadge provenance={provenance} />
                </summary>
                {(provenance || (criterion.status === 'pending' && criterion.check)) && (
                  <div className="flex flex-col gap-1.5 pb-1 pl-4 pr-1 text-xs">
                    {provenance && provenance.kind === 'verified' && (
                      <>
                        <div className="font-mono text-muted">$ {provenance.command}</div>
                        {typeof provenance.exitCode === 'number' && (
                          <div className="text-[0.6875rem] text-muted">Code de sortie : {provenance.exitCode}</div>
                        )}
                        <pre className="max-h-40 overflow-auto rounded bg-code p-2 font-mono text-[0.6875rem] text-muted">
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
