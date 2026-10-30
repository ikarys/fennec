import type { ThreadItem } from '../../../lib/fold';
import { formatClockTime, formatDuration } from '../../../lib/format';
import { Badge } from '../../ui/Badge';
import { Button } from '../../ui/Button';
import { Card } from '../../ui/Card';
import { useRunUsage } from '../../session/CallUsage';
import { useIntegrationInvite } from '../../session/IntegrationInvite';

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

type Stat = { label: string; value: string; hint?: string };

function StatCell({ stat }: { stat: Stat }) {
  return (
    <div className="flex items-baseline justify-between gap-3 px-3 py-1.5">
      <span className="text-xs text-muted">{stat.label}</span>
      <span className="whitespace-nowrap text-right text-[0.8125rem] font-medium tabular-nums text-text">
        {stat.value}
        {stat.hint && <span className="ml-1.5 text-[0.6875rem] font-normal text-muted">{stat.hint}</span>}
      </span>
    </div>
  );
}

export function RunEndCard({ item }: { item: WorkflowEndItem }) {
  const outcome = outcomeTitle(item.outcome);
  const stats = item.stats;
  const invite = useIntegrationInvite();
  const usage = useRunUsage(item.runId);

  const startAt = stats?.startedAt;
  const endAt = stats?.finishedAt;

  const statRows: Stat[][] = stats
    ? [
        ...(startAt != null && endAt != null
          ? [
              [
                { label: 'Départ', value: formatClockTime(startAt) },
                { label: 'Fin', value: formatClockTime(endAt) },
              ],
              [{ label: 'Durée totale', value: formatDuration(endAt - startAt) }],
            ]
          : []),
        [
          { label: 'Temps de travail', value: formatDuration(stats.workingMs) },
          { label: 'Appels au modèle', value: usage?.calls ?? '–' },
        ],
        [
          { label: 'Tokens générés', value: usage?.generated ?? '–' },
          { label: 'Tokens lus', value: usage?.read ?? '–', hint: usage?.cache },
        ],
        [
          { label: 'Fichiers modifiés', value: String(stats.filesChanged) },
          {
            label: 'Critères',
            value: `${stats.criteriaPassed}/${stats.criteriaTotal}`,
            hint: `${stats.criteriaVerified} vérifiés · ${stats.criteriaTotal - stats.criteriaVerified} indicatif${
              stats.criteriaTotal - stats.criteriaVerified > 1 ? 's' : ''
            }`,
          },
        ],
      ]
    : [];

  const nodeTotalMs = stats ? stats.nodes.reduce((sum, node) => sum + node.workingMs, 0) : 0;

  return (
    <Card className="px-3 py-2">
      <div className="flex flex-col gap-3">
        <div className="flex flex-col gap-1">
          <div className={`text-sm font-semibold ${outcome.className}`}>{outcome.text}</div>
          {item.reason && <div className="text-sm text-muted">{item.reason}</div>}
        </div>

        {stats && (
          <div className="flex flex-col gap-3">
            <div className="divide-y divide-border border-y border-border">
              {statRows.map((row) => (
                <div key={row[0].label}>
                  <div className="grid grid-cols-1 gap-x-8 sm:grid-cols-2">
                    {row.map((stat) => (
                      <StatCell key={stat.label} stat={stat} />
                    ))}
                  </div>
                </div>
              ))}
            </div>

            {stats.nodes.length > 0 && (
              <details className="group border-b border-border">
                <summary className="flex cursor-pointer list-none items-center gap-2 px-3 py-1.5 text-xs text-muted select-none [&::-webkit-details-marker]:hidden">
                  <span aria-hidden className="text-[0.6875rem] transition-transform group-open:rotate-90">
                    ▶
                  </span>
                  <span>Détail du temps</span>
                  <span className="ml-auto tabular-nums">
                    {formatDuration(stats.workingMs)} · {stats.nodes.length} nœud{stats.nodes.length > 1 ? 's' : ''}
                  </span>
                </summary>
                <table className="w-full border-collapse text-left text-xs tabular-nums">
                  <thead>
                    <tr className="border-t border-border text-muted">
                      <th className="py-1 pl-3 pr-3 font-medium">Nœud</th>
                      <th className="py-1 pr-3 font-medium" />
                      <th className="w-16 py-1 pr-3 text-right font-medium">Temps</th>
                    </tr>
                  </thead>
                  <tbody>
                    {stats.nodes.map((node) => (
                      <tr key={node.nodeId}>
                        <td className="py-1 pl-3 pr-3 text-text">
                          {node.label}
                          {node.runs > 1 && (
                            <span
                              title="boucle"
                              className="ml-1.5 text-[0.6875rem] text-muted tabular-nums"
                            >
                              ×{node.runs}
                            </span>
                          )}
                        </td>
                        <td className="py-1 pr-3">
                          <div className="h-1 w-full rounded-full bg-faint/30">
                            <div
                              className="h-full rounded-full bg-accent"
                              style={{
                                width:
                                  node.workingMs > 0 && nodeTotalMs > 0
                                    ? `${Math.max((node.workingMs / nodeTotalMs) * 100, 1)}%`
                                    : '0%',
                              }}
                            />
                          </div>
                        </td>
                        <td className="py-1 pr-3 text-right text-muted">{formatDuration(node.workingMs)}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </details>
            )}

            <div className="flex items-center gap-2 text-[0.6875rem] text-muted">
              <span>Calculé par fennec à partir du journal</span>
              <Badge tone="ok">vérifié</Badge>
            </div>
          </div>
        )}
        {item.outcome === 'done' && invite && invite.lastRunEndId === item.id && (
          <div className="flex items-center justify-between gap-3 border-t border-border pt-3">
            <span className="text-xs text-muted">Le travail est prêt dans l'espace de l'agent.</span>
            <Button variant="primary" size="sm" onClick={invite.open}>
              Relire et intégrer…
            </Button>
          </div>
        )}
      </div>
    </Card>
  );
}
