/**
 * The idea's run meta: duration, tokens, criteria, decisions, and git counts.
 */
import { formatDuration, formatTokens } from '../../../lib/format';
import type { IdeaBoard, IdeaView } from '../../../lib/ideas';

export function IdeaMeta({
  idea,
  caps,
}: {
  idea: IdeaView;
  caps: IdeaBoard['queue']['caps'];
}) {
  return (
    <>
      {idea.durationMs != null ? (
        idea.autonomous && caps.maxMinutes > 0 ? (
          <span
            className="text-muted tabular-nums"
            title="Durée de l'idée, plafond du mode nuit"
          >
            {formatDuration(idea.durationMs)} / {caps.maxMinutes} min
          </span>
        ) : (
          <span className="text-muted tabular-nums">
            {formatDuration(idea.durationMs)}
          </span>
        )
      ) : null}
      {idea.tokens != null ? (
        idea.autonomous && caps.maxTokens > 0 ? (
          <span className="text-muted tabular-nums">
            {formatTokens(idea.tokens)} / {formatTokens(caps.maxTokens)} tokens
          </span>
        ) : (
          <span className="text-muted tabular-nums">
            {formatTokens(idea.tokens)} tokens
          </span>
        )
      ) : null}
      {idea.criteria ? (
        <span
          className="text-muted tabular-nums"
          title="Critères d'acceptation du plan approuvé"
        >
          critères : {idea.criteria.passed}{' '}
          {idea.criteria.passed === 0 || idea.criteria.passed === 1
            ? 'validé'
            : 'validés'}{' '}
          ·{' '}
          <span
            className={
              idea.criteria.failed > 0 ? 'text-err tabular-nums' : 'tabular-nums'
            }
          >
            {idea.criteria.failed}{' '}
            {idea.criteria.failed === 0 || idea.criteria.failed === 1
              ? 'échoué'
              : 'échoués'}
          </span>
          {idea.criteria.pending > 0 ? (
            <>
              {' '}
              · {idea.criteria.pending} en attente
            </>
          ) : null}
        </span>
      ) : null}
      {idea.decisions.length > 0 ? (
        <span
          className="text-muted tabular-nums"
          title={idea.decisions.map((d) => `${d.title}: ${d.choice}`).join('\n')}
        >
          {idea.decisions.length} décision{idea.decisions.length > 1 ? 's' : ''}
        </span>
      ) : null}
      {idea.commits != null ? (
        <span className="text-muted tabular-nums">
          {idea.commits} commit{idea.commits > 1 ? 's' : ''}
          {idea.files != null
            ? ` · ${idea.files} fichier${idea.files > 1 ? 's' : ''}`
            : ''}
        </span>
      ) : null}
    </>
  );
}
