/**
 * The agent suggested for what comes next, a row of the decision bar: it
 * opens a new session seeded from this one.
 */
import { type AgentSuggestion } from '../../lib/api';
import { DecisionRow } from './DecisionBar';
import { Button } from '../ui/Button';
import { useT } from '../../i18n';

export function AgentSuggestionDecision({
  suggestion,
  sessionId,
  projectId,
  open,
  onToggle,
}: {
  suggestion: AgentSuggestion;
  sessionId: string;
  projectId: string;
  open: boolean;
  onToggle: () => void;
}) {
  const t = useT();

  const newSessionButton = (
    <Button
      variant="primary"
      size="sm"
      title={t('suggestionTitle')}
      onClick={() => {
        window.location.hash = `#/new/${encodeURIComponent(projectId)}?from=${encodeURIComponent(sessionId)}`;
      }}
    >
      {t('newSession')}
    </Button>
  );

  return (
    <DecisionRow
      icon={
        <span
          className="h-1.5 w-1.5 rounded-full bg-muted"
          style={suggestion.color ? { backgroundColor: suggestion.color } : undefined}
        />
      }
      title={t('switchTo', { name: suggestion.name })}
      meta={t('suggestionBadge')}
      primary={newSessionButton}
      open={open}
      onToggle={onToggle}
      footer={
        <>
          <span className="flex-1" />
          {newSessionButton}
        </>
      }
    >
      <p className="m-0 text-sm">
        {suggestion.reason} {t('suggestedHint')}
      </p>
    </DecisionRow>
  );
}
