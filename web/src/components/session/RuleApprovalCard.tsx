/**
 * The pending rule approval. It is rendered above the composer so the human
 * can allow or refuse the paused call with one click.
 */
import type { PendingApproval } from '../../lib/fold';
import type { ApprovalChoice } from '../../lib/api/sessions';
import { Button } from '../ui/Button';
import { Card } from '../ui/Card';

function approvalAction(approval: PendingApproval): string {
  if (approval.kind === 'command') return 'Lancer la commande';
  if (approval.kind === 'tool') return "Utiliser l'outil";
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
        <div className="flex flex-col gap-2">
          <div className="flex items-center gap-1.5 text-xs font-semibold">
            <span className="text-accent">!</span>
            <span className="text-accent-strong">Autorisation demandée</span>
          </div>
          <div className="text-sm leading-snug [overflow-wrap:break-word]">
            <span className="text-muted">{action}</span>{' '}
            <span className="font-mono text-text">{approval.subject}</span>
          </div>
          <div className="flex flex-wrap gap-1.5">
            <Button
              type="button"
              variant="primary"
              size="sm"
              disabled={disabled}
              onClick={() => onAnswer('once')}
            >
              Une fois
            </Button>
            <Button
              type="button"
              variant="secondary"
              size="sm"
              multiline
              disabled={disabled}
              onClick={() => onAnswer('session')}
              className="max-w-full"
            >
              <span>
                <span className="font-medium">Pour la session</span>
                <span className="font-normal opacity-75">
                  {' '}
                  <span className="font-mono">{scope.pattern}</span>
                  {scope.suffix}
                </span>
              </span>
            </Button>
            <Button
              type="button"
              variant="secondary"
              size="sm"
              disabled={disabled}
              onClick={() => onAnswer('always')}
            >
              Toujours (projet)
            </Button>
            <Button
              type="button"
              variant="danger"
              size="sm"
              disabled={disabled}
              onClick={() => onAnswer('never')}
            >
              Jamais (projet)
            </Button>
          </div>
          <div className="text-xs text-faint">
            « Toujours » et « Jamais » créent une règle du projet, visible et supprimable dans Réglages › Règles.
          </div>
        </div>
      </Card>
    </div>
  );
}
