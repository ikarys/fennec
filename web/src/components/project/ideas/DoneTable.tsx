/**
 * The done tasks table: the server's order, no sorting in the UI.
 */
import { Button } from '../../ui/Button';
import { IdeaStatusBadge } from './IdeaStatusBadge';
import { formatDuration, formatRelative, formatTokens } from '../../../lib/format';
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
          <th className="px-3 py-2 text-right text-xs font-medium text-muted">Commits</th>
          <th className="px-3 py-2 text-right text-xs font-medium text-muted">Décisions</th>
          <th className="px-3 py-2 text-left text-xs font-medium text-muted">Démarrée</th>
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
              <td className="px-3 py-2">
                <IdeaStatusBadge status={idea.status} />
              </td>
              <td className="px-3 py-2 text-right">
                {idea.durationMs != null ? formatDuration(idea.durationMs) : '—'}
              </td>
              <td className="px-3 py-2 text-right">
                {idea.tokens != null ? formatTokens(idea.tokens) : '—'}
              </td>
              <td className="px-3 py-2 text-right">
                {idea.criteria ? `${idea.criteria.passed}✓ ${idea.criteria.failed}✗` : '—'}
              </td>
              <td className="px-3 py-2 text-right">
                {idea.commits != null
                  ? `${idea.commits} · ${idea.files ?? 0} fichier${(idea.files ?? 0) > 1 ? 's' : ''}`
                  : '—'}
              </td>
              <td className="px-3 py-2 text-right">
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
              <td className="px-3 py-2">
                {idea.startedAt != null ? formatRelative(idea.startedAt) : '—'}
              </td>
              <td className="px-3 py-2 text-right">
                {sessionId ? (
                  <Button variant="ghost" size="sm" onClick={() => onOpenSession(sessionId)}>
                    Relire
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
