/**
 * The agent switch an agent suggested to the human, offered after the turn.
 *
 * It sits after the other cards and only opens a new-session page: the new
 * session starts when the human sends the prefilled brief.
 */
import { type AgentSuggestion } from '../../lib/api';
import { Button } from '../ui/Button';
import { Card } from '../ui/Card';
import { useT } from '../../i18n';

export function AgentSuggestionCard({
  suggestion,
  sessionId,
  projectId,
}: {
  suggestion: AgentSuggestion;
  sessionId: string;
  projectId: string;
}) {
  const t = useT();
  return (
    <Card className="flex flex-col gap-3 p-4">
      <div className="flex flex-col gap-1">
        <p className="m-0 flex items-center gap-2 text-sm font-medium text-text">
          <span
            className="h-1.5 w-1.5 rounded-full bg-muted"
            style={suggestion.color ? { backgroundColor: suggestion.color } : undefined}
          />
          {t('switchTo', { name: suggestion.name })}
        </p>
        <p className="m-0 text-xs text-muted">
          {suggestion.reason} {t('suggestedHint')}
        </p>
      </div>
      <div className="flex items-center gap-2">
        <Button
          variant="primary"
          size="md"
          title={t('suggestionTitle')}
          onClick={() => {
            window.location.hash = `#/new/${encodeURIComponent(projectId)}?from=${encodeURIComponent(sessionId)}`;
          }}
        >
          {t('newSession')}
        </Button>
      </div>
    </Card>
  );
}
