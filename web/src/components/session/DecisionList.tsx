import type { DecisionView } from '../../lib/api';
import { DecisionIcon } from '../Icons';
import { Badge } from '../ui/Badge';
import { useT } from '../../i18n';

/** The recorded decisions, one folded entry each; shared by the context panel and the integration review. */
export function DecisionList({ decisions }: { decisions: DecisionView[] }) {
  const t = useT();
  if (decisions.length === 0) return <div className="text-xs text-muted">{t('noDecisions')}</div>;
  return (
    <div className="flex flex-col">
      {decisions.map((decision) => (
        <details key={decision.callId || decision.title}>
          <summary className="group flex cursor-pointer list-none items-start gap-2 rounded-md px-1 py-1.5 text-xs select-none hover:bg-accent-soft">
            <DecisionIcon className="mt-0.5 h-3 w-3 shrink-0 text-muted" />
            <span className="grow break-words text-text group-hover:text-accent-strong">{decision.title}</span>
            <Badge tone="accent" title={t('decisionIndicativeTitle')}>{t('indicative')}</Badge>
          </summary>
          <div className="flex flex-col gap-1.5 pb-1 pl-4 pr-1 text-xs">
            <div className="break-words text-text">{decision.choice}</div>
            <div className="break-words text-muted">{decision.reason}</div>
            {decision.alternatives.length > 0 && (
              <ul className="m-0 list-disc pl-4 text-muted">
                {decision.alternatives.map((alternative) => (
                  <li key={alternative} className="break-words">{alternative}</li>
                ))}
              </ul>
            )}
          </div>
        </details>
      ))}
    </div>
  );
}
