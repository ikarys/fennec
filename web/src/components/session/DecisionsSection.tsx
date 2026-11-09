import type { DecisionView } from '../../lib/api';
import { DecisionIcon } from '../ui/Icons';
import { Panel } from '../ui/Panel';
import { DecisionList } from './DecisionList';
import { useT } from '../../i18n';

export function DecisionsSection({ decisions }: { decisions: DecisionView[] }) {
  const t = useT();
  return (
    <Panel
      id="decisions"
      title={t('panelDecisions')}
      hint={t('decisionsHint')}
      icon={<DecisionIcon className="h-3.5 w-3.5" />}
      summary={decisions.length > 0 ? decisions.length : undefined}
    >
      <DecisionList decisions={decisions} />
    </Panel>
  );
}
