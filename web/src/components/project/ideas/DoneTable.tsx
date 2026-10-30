/**
 * The done tasks table: the server's order, no sorting in the UI.
 */
import { Button } from '../../ui/Button';
import { EyeIcon } from '../../Icons';
import { IdeaStatusBadge } from './IdeaStatusBadge';
import { formatDuration, formatTokens } from '../../../lib/format';
import type { IdeaView } from '../../../lib/ideas';

export function DoneTable({
  ideas,
  onOpenSession,
}: {
  ideas: IdeaView[];
  onOpenSession: (sessionId: string) => void;
}) {
  if (ideas.length === 0) {
    return <p className="text-sm text-muted">Aucune tâche terminée.</p>;
  }

  return (
    <table className="w-full text-sm tabular-nums">
      <thead>
        <tr>
          <th className="px-3 py-2 text-left text-xs font-medium text-muted">Tâche</th>
          <th className="px-3 py-2 text-left text-xs font-medium text-muted">Statut</th>
          <th className="px-3 py-2 text-right text-xs font-medium text-muted">Durée</th>
          <th className="px-3 py-2 text-right text-xs font-medium text-muted">Tokens</th>
          <th className="px-3 py-2 text-right text-xs font-medium text-muted">Critères</th>
          <th className="px-3 py-2 text-right text-xs font-medium text-muted">Décisions</th>
          <th className="px-3 py-2" />
        </tr>
      </thead>
      <tbody>
        {ideas.map((idea) => {
          const sessionId = idea.sessionId;
          return (
            <tr key={idea.id} className="border-t border-border">
              <td className="max-w-[320px] px-3 py-2">
                <span className="block truncate" title={idea.text}>
                  {idea.text}
                </span>
              </td>
              <td className="px-3 py-2 whitespace-nowrap">
                <IdeaStatusBadge status={idea.status} />
              </td>
              <td className="px-3 py-2 text-right whitespace-nowrap">
                {idea.durationMs != null ? formatDuration(idea.durationMs) : '—'}
              </td>
              <td className="px-3 py-2 text-right whitespace-nowrap">
                {idea.tokens != null ? formatTokens(idea.tokens) : '—'}
              </td>
              <td className="px-3 py-2 text-right whitespace-nowrap">
                {idea.criteria ? `${idea.criteria.passed}✓ ${idea.criteria.failed}✗` : '—'}
              </td>
              <td className="px-3 py-2 text-right whitespace-nowrap">
                {idea.decisions.length > 0 ? (
                  <span
                    title={idea.decisions.map((d) => `${d.title}: ${d.choice}`).join('\n')}
                  >
                    {idea.decisions.length}
                  </span>
                ) : (
                  '—'
                )}
              </td>
              <td className="px-3 py-2 text-right whitespace-nowrap">
                {sessionId ? (
                  <Button
                    variant="ghost"
                    size="sm"
                    iconOnly
                    title="Relire"
                    aria-label="Relire"
                    onClick={() => onOpenSession(sessionId)}
                  >
                    <EyeIcon className="h-4 w-4" />
                  </Button>
                ) : null}
              </td>
            </tr>
          );
        })}
      </tbody>
    </table>
  );
}
