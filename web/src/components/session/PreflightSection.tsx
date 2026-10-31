/** The plan's pre-flight checks, verified by fennec before any change. */
import type { PreflightResultView } from '../../lib/fold';
import { Badge } from '../ui/Badge';
import { SectionLabel } from '../ui/SectionLabel';

const INLINE_CODE = 'rounded bg-code px-1 py-0.5 font-mono text-xs text-text';

const VERDICTS: Record<PreflightResultView['verdict'], { tone: 'ok' | 'err'; label: string; title: string }> = {
  fails: {
    tone: 'ok',
    label: 'échoue avant le travail',
    title: "Attendu : le critère n'est pas encore rempli"
  },
  passesAlready: {
    tone: 'err',
    label: 'passe déjà',
    title: 'Ne prouve rien : renvoyé au planificateur'
  },
  cannotRun: {
    tone: 'err',
    label: 'ne peut pas tourner ici',
    title: 'Environnement : renvoyé au planificateur'
  }
};

export function PreflightSection({ results }: { results: PreflightResultView[] }) {
  return (
    <div className="flex flex-col gap-1.5">
      <div className="flex items-center justify-between gap-2">
        <SectionLabel>Pré-contrôle</SectionLabel>
        <Badge tone="ok" title="Fait établi par fennec : les commandes ont tourné avant tout changement">
          vérifié
        </Badge>
      </div>
      <ul className="m-0 flex flex-col gap-1">
        {results.map((result, i) => {
          const verdict = VERDICTS[result.verdict];
          const label = result.exitCode !== undefined ? `${verdict.label} · exit ${result.exitCode}` : verdict.label;
          return (
            <li key={`${result.criterionId}-${i}`} className="flex flex-col gap-1">
              <div className="flex flex-wrap items-center gap-2">
                <span className="font-mono text-xs text-muted">{result.criterionId}</span>
                <code className={`min-w-0 max-w-full truncate ${INLINE_CODE}`} title={result.command}>
                  {result.command}
                </code>
                <Badge tone={verdict.tone} title={verdict.title}>
                  {label}
                </Badge>
              </div>
              {result.output && (
                <details>
                  <summary className="cursor-pointer text-xs text-muted">Sortie</summary>
                  <pre className="max-h-48 overflow-auto whitespace-pre-wrap rounded bg-code p-2 font-mono text-xs text-muted">
                    {result.output}
                  </pre>
                </details>
              )}
            </li>
          );
        })}
      </ul>
    </div>
  );
}
