/**
 * The idea's run meta: duration, tokens, criteria, decisions, and git counts.
 */
import { formatDuration, formatTokens } from '../../../lib/format';
import type { IdeaView } from '../../../lib/ideas';

export function IdeaMeta({ idea }: { idea: IdeaView }) {
  return (
    <>
      {idea.durationMs != null ? (
        <span className="text-muted">{formatDuration(idea.durationMs)}</span>
      ) : null}
      {idea.tokens != null ? (
        <span className="text-muted">{formatTokens(idea.tokens)} tok</span>
      ) : null}
      {idea.criteria ? (
        <span className="text-muted">
          {idea.criteria.passed}✓ {idea.criteria.failed}✗
        </span>
      ) : null}
      {idea.decisions.length > 0 ? (
        <span
          className="text-muted"
          title={idea.decisions.map((d) => `${d.title}: ${d.choice}`).join('\n')}
        >
          {idea.decisions.length} décision{idea.decisions.length > 1 ? 's' : ''}
        </span>
      ) : null}
      {idea.commits != null ? (
        <span className="text-muted">
          {idea.commits} commit{idea.commits > 1 ? 's' : ''}
          {idea.files != null
            ? ` · ${idea.files} fichier${idea.files > 1 ? 's' : ''}`
            : ''}
        </span>
      ) : null}
    </>
  );
}
