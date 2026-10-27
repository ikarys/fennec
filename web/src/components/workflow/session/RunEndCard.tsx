import type { ThreadItem } from '../../../lib/fold';
import { cachePercent, formatDuration, formatTokens } from '../../../lib/format';
import { Badge } from '../../ui/Badge';
import { Card } from '../../ui/Card';
import { SectionLabel } from '../../ui/SectionLabel';

type WorkflowEndItem = Extract<ThreadItem, { kind: 'workflow-end' }>;

function outcomeTitle(outcome: WorkflowEndItem['outcome']): { text: string; className: string } {
  switch (outcome) {
    case 'done':
      return { text: 'Workflow terminé', className: 'text-ok' };
    case 'blocked':
      return { text: 'Workflow bloqué', className: 'text-err' };
    case 'stopped':
      return { text: 'Workflow arrêté', className: 'text-muted' };
    case 'error':
      return { text: 'Workflow en erreur', className: 'text-err' };
  }
}

function StatTile({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex flex-col gap-0.5 rounded-lg border border-border bg-panel px-2.5 py-2">
      <SectionLabel>{label}</SectionLabel>
      <div className="text-sm font-medium text-text tabular-nums">{value}</div>
    </div>
  );
}

export function RunEndCard({ item }: { item: WorkflowEndItem }) {
  const outcome = outcomeTitle(item.outcome);

  return (
    <Card className="px-3 py-2">
      <div className="flex flex-col gap-3">
        <div className="flex flex-col gap-1">
          <div className={`text-sm font-semibold ${outcome.className}`}>{outcome.text}</div>
          {item.reason && <div className="text-sm text-muted">{item.reason}</div>}
        </div>

        {item.stats && (
          <div className="flex flex-col gap-3">
            <div className="grid grid-cols-2 gap-2">
              <StatTile label="Temps de travail" value={formatDuration(item.stats.workingMs)} />
              <StatTile label="Appels au modèle" value={String(item.stats.calls)} />
              <StatTile label="Tokens générés" value={formatTokens(item.stats.generationTokens)} />
              <StatTile
                label="Tokens lus"
                value={`${formatTokens(item.stats.prefillTokens)} · ${cachePercent(item.stats.cachedPromptTokens, item.stats.prefillTokens)} % cache`}
              />
              <StatTile label="Fichiers modifiés" value={String(item.stats.filesChanged)} />
              <StatTile
                label="Critères"
                value={`${item.stats.criteriaPassed}/${item.stats.criteriaTotal} · dont ${item.stats.criteriaVerified} vérifiés`}
              />
            </div>

            {item.stats.nodes.length > 0 && (
              <table className="w-full border-collapse text-left text-xs tabular-nums">
                <thead>
                  <tr className="text-muted">
                    <th className="py-1 pr-3 font-medium">Nœud</th>
                    <th className="py-1 pr-3 font-medium">Passages</th>
                    <th className="py-1 font-medium">Temps</th>
                  </tr>
                </thead>
                <tbody>
                  {item.stats.nodes.map((node) => (
                    <tr key={node.nodeId} className={node.runs > 1 ? 'bg-accent-soft/60' : undefined}>
                      <td className="py-1 pr-3 text-text">{node.label}</td>
                      <td className="py-1 pr-3 text-muted">{node.runs}</td>
                      <td className="py-1 text-muted">{formatDuration(node.workingMs)}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            )}

            <div className="flex items-center gap-2 text-[0.6875rem] text-muted">
              <span>Calculé par fennec à partir du journal</span>
              <Badge tone="ok">vérifié</Badge>
            </div>
          </div>
        )}
      </div>
    </Card>
  );
}
