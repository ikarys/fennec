import type { DecisionView } from '../../lib/api';
import { DecisionIcon } from '../Icons';
import { Panel } from '../ui/Panel';
import { DecisionList } from './DecisionList';
import { useT } from '../../i18n';

export function DecisionsSection({ decisions }: { decisions: DecisionView[] }) {
  const t = useT();
  return (
    <Panel
      id="decisions"
      title={t('panelDecisions')}
      icon={<DecisionIcon className="h-3.5 w-3.5" />}
      summary={decisions.length > 0 ? decisions.length : undefined}
    >
      <p className="m-0 text-xs text-muted">{t('decisionsHint')}</p>
      <DecisionList decisions={decisions} />
    </Panel>
  );
}
