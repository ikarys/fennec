import type { DecisionView } from '../../lib/api';
import { panelBlock } from '../../lib/panelBlocks';
import { Panel } from '../ui/Panel';
import { DecisionList } from './DecisionList';

export function DecisionsSection({ decisions }: { decisions: DecisionView[] }) {
  return (
    <Panel
      id="decisions"
      title={panelBlock('decisions').title}
      summary={decisions.length > 0 ? decisions.length : undefined}
    >
      <p className="m-0 text-xs text-muted">Les choix de conception que l'agent a consignés.</p>
      <DecisionList decisions={decisions} />
    </Panel>
  );
}
