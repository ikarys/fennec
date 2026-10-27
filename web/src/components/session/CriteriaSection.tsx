import type { CriterionView } from '../../lib/fold';

export function CriteriaSection({ criteria }: { criteria: CriterionView[] }) {
  const passed = criteria.filter((criterion) => criterion.status === 'passed').length;

  return (
    <section className="flex flex-col gap-1.5">
      <h2 className="text-[11px] font-semibold tracking-[0.06em] text-muted uppercase">
        Critères{criteria.length > 0 && <span className="ml-1 tabular-nums">{passed}/{criteria.length}</span>}
      </h2>
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
            const badge = provenance
              ? provenance.kind === 'verified'
                ? {
                    label: 'vérifié',
                    title: 'Fait établi par fennec : la commande a tourné',
                    className: 'border-ok bg-ok/10 text-ok'
                  }
                : {
                    label: 'indicatif',
                    title: `Jugement d'un agent (${provenance.agent}) : à relire`,
                    className: 'border-accent bg-accent-soft text-accent-strong'
                  }
              : null;

            return (
              <details key={criterion.id}>
                <summary className="flex cursor-pointer list-none items-start gap-2 py-1.5 text-xs select-none">
                  <span aria-hidden className={`mt-1 h-2 w-2 shrink-0 rounded-full ${dotClass}`} />
                  <span className="shrink-0 font-mono text-muted">{criterion.id}</span>
                  <span className="grow break-words text-text">{criterion.description}</span>
                  {badge && (
                    <span
                      title={badge.title}
                      className={`shrink-0 rounded border px-1 py-0.5 text-[10px] ${badge.className}`}
                    >
                      {badge.label}
                    </span>
                  )}
                </summary>
                {(provenance || (criterion.status === 'pending' && criterion.check)) && (
                  <div className="flex flex-col gap-1.5 pb-1 pl-4 pr-1 text-xs">
                    {provenance && provenance.kind === 'verified' && (
                      <>
                        <div className="font-mono text-muted">$ {provenance.command}</div>
                        {typeof provenance.exitCode === 'number' && (
                          <div className="text-[11px] text-muted">Code de sortie : {provenance.exitCode}</div>
                        )}
                        <pre className="max-h-40 overflow-auto rounded bg-code p-2 font-mono text-[11px] text-muted">
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
    </section>
  );
}
