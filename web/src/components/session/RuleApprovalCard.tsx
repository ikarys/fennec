/**
 * The pending rule approval. It is rendered above the composer so the human
 * can allow or refuse the paused call with one click.
 */
import type { PendingApproval } from '../../lib/fold';
import type { ApprovalChoice } from '../../lib/api/sessions';
import { shortPath } from '../../lib/shortPath';
import { Button } from '../ui/Button';
import { Card } from '../ui/Card';

function approvalAction(approval: PendingApproval): string {
  if (approval.kind === 'command') return 'Lancer';
  if (approval.kind === 'tool') return 'Utiliser';
  if (approval.access === 'write') return 'Écrire';
  if (approval.access === 'readWrite') return 'Lire et écrire';
  return 'Lire';
}

function sessionScope(approval: PendingApproval): { pattern: string; suffix: string | null } {
  const pattern = approval.pattern;
  const suffix = approval.kind === 'path' && pattern.endsWith('/') ? ' et ses sous-dossiers' : null;
  return { pattern, suffix };
}

export function RuleApprovalCard({
  approval,
  onAnswer,
  disabled
}: {
  approval: PendingApproval;
  onAnswer: (choice: ApprovalChoice) => void;
  disabled: boolean;
}) {
  const action = approvalAction(approval);
  const scope = sessionScope(approval);

  return (
    <div className="px-6 py-2">
      <Card className="mx-auto max-w-[820px] px-3 py-2">
        <div className="flex flex-wrap items-center gap-2">
          <span className="text-accent">!</span>
          <span className="text-muted">{action}</span>
          <span className="min-w-0 truncate font-mono text-sm text-text" title={approval.subject}>
            {approval.kind === 'path' ? shortPath(approval.subject) : approval.subject}
          </span>
          <div className="ml-auto flex flex-wrap items-center gap-1.5">
            <Button
              type="button"
              variant="primary"
              size="sm"
              disabled={disabled}
              title="Autoriser cet appel seulement"
              onClick={() => onAnswer('once')}
            >
              Une fois
            </Button>
            <Button
              type="button"
              variant="secondary"
              size="sm"
              disabled={disabled}
              title={`Pour cette session : ${scope.pattern}${scope.suffix ?? ''}`}
              onClick={() => onAnswer('session')}
            >
              Session
            </Button>
            <Button
              type="button"
              variant="secondary"
              size="sm"
              disabled={disabled}
              title="Règle du projet, visible et supprimable dans Réglages › Règles"
              onClick={() => onAnswer('always')}
            >
              Toujours
            </Button>
            <Button
              type="button"
              variant="danger"
              size="sm"
              disabled={disabled}
              title="Refuser et créer une règle du projet (Réglages › Règles)"
              onClick={() => onAnswer('never')}
            >
              Jamais
            </Button>
          </div>
        </div>
      </Card>
    </div>
  );
}
