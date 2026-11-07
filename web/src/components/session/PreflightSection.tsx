/** The plan's pre-flight checks, verified by fennec before any change.
 * The inset block and its ok edge mark provenance: fennec ran these, the model did not. */
import type { PreflightResultView } from '../../lib/fold';
import { Badge } from '../ui/Badge';
import { ExistenceOnlyBadge } from './CriteriaSection';
import { Card } from '../ui/Card';
import { SectionLabel } from '../ui/SectionLabel';
import { useT } from '../../i18n';
import type { MsgKey } from '../../i18n/messages';

const INLINE_CODE = 'rounded-md bg-code px-1 py-0.5 font-mono text-xs text-text';

const VERDICTS: Record<PreflightResultView['verdict'], { tone: 'ok' | 'err'; label: MsgKey; title: MsgKey }> = {
  fails: {
    tone: 'ok',
    label: 'verdictFailsLabel',
    title: 'verdictFailsTitle'
  },
  passesAlready: {
    tone: 'err',
    label: 'verdictPassesLabel',
    title: 'verdictPassesTitle'
  },
  cannotRun: {
    tone: 'err',
    label: 'verdictCannotLabel',
    title: 'verdictCannotTitle'
  }
};

export function PreflightSection({ results }: { results: PreflightResultView[] }) {
  const t = useT();
  return (
    <Card variant="inset" className="border-l-2 border-l-ok">
      <div className="flex items-center justify-between gap-2">
        <SectionLabel>{t('preflightTitle')}</SectionLabel>
        <Badge tone="ok" title={t('preflightVerifiedTitle')}>
          {t('verified')}
        </Badge>
      </div>
      <ul className="m-0 flex flex-col gap-1.5">
        {results.map((result, i) => {
          const verdict = VERDICTS[result.verdict];
          const label = result.exitCode !== undefined ? `${t(verdict.label)} · ${t('exitCode', { code: result.exitCode })}` : t(verdict.label);
          return (
            <li key={`${result.criterionId}-${i}`} className="flex flex-col gap-1">
              <div className="flex flex-wrap items-center gap-2">
                <span className="font-mono text-xs text-muted">{result.criterionId}</span>
                <code className={`min-w-0 max-w-full truncate ${INLINE_CODE}`} title={result.command}>
                  {result.command}
                </code>
                <Badge tone={verdict.tone} title={t(verdict.title)}>
                  {label}
                </Badge>
                <ExistenceOnlyBadge existenceOnly={result.existenceOnly} />
              </div>
              {result.output && (
                <details>
                  <summary className="cursor-pointer text-xs text-muted">{t('outputLabel')}</summary>
                  <pre className="max-h-48 overflow-auto whitespace-pre-wrap rounded-md bg-code p-2 font-mono text-xs text-muted">
                    {result.output}
                  </pre>
                </details>
              )}
            </li>
          );
        })}
      </ul>
    </Card>
  );
}
