import type { DecisionView } from '../../lib/fold';
import { panelBlock } from '../../lib/panelBlocks';
import { Badge } from '../ui/Badge';
import { Panel } from '../ui/Panel';

export function DecisionsSection({ decisions }: { decisions: DecisionView[] }) {
  return (
    <Panel
      id="decisions"
      title={panelBlock('decisions').title}
      summary={decisions.length > 0 ? decisions.length : undefined}
    >
      <p className="m-0 text-[0.6875rem] text-muted">Les choix de conception que l'agent a consignés.</p>
      {decisions.length === 0 ? (
        <div className="text-xs text-muted">Aucune décision consignée.</div>
      ) : (
        <div className="flex flex-col">
          {decisions.map((decision) => (
            <details key={decision.callId || decision.title}>
              <summary className="flex cursor-pointer list-none items-start gap-2 py-1.5 text-xs select-none">
                <span className="grow break-words text-text">{decision.title}</span>
                <Badge tone="accent" title="Consigné par l'agent : à relire">indicatif</Badge>
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
      )}
    </Panel>
  );
}
