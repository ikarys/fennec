/**
 * The pending rule approval. It is rendered above the composer so the human
 * can allow or refuse the paused call with one click.
 */
import { useState } from 'react';
import type { PendingApproval } from '../../lib/fold';
import type { ApprovalChoice } from '../../lib/api/sessions';
import { shortPath } from '../../lib/shortPath';
import { Button } from '../ui/Button';
import { Card } from '../ui/Card';
import { Select } from '../ui/Field';

function approvalAction(approval: PendingApproval): string {
  if (approval.kind === 'command') return 'Lancer';
  if (approval.kind === 'tool') return 'Utiliser';
  if (approval.access === 'write') return 'Écrire';
  if (approval.access === 'readWrite') return 'Lire et écrire';
  return 'Lire';
}

function sessionScope(
  folder: string,
  kind: PendingApproval['kind']
): { pattern: string; suffix: string | null } {
  const pattern = folder;
  const suffix = kind === 'path' && pattern.endsWith('/') ? ' et ses sous-dossiers' : null;
  return { pattern, suffix };
}

export function RuleApprovalCard({
  approval,
  onAnswer,
  disabled
}: {
  approval: PendingApproval;
  onAnswer: (choice: ApprovalChoice, pattern?: string) => void;
  disabled: boolean;
}) {
  const [folder, setFolder] = useState(approval.pattern);
  const [forCall, setForCall] = useState(approval.callId);
  if (forCall !== approval.callId) {
    setForCall(approval.callId);
    setFolder(approval.pattern);
  }

  const action = approvalAction(approval);
  const scope = sessionScope(folder, approval.kind);

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
            {approval.folders.length > 1 && (
              <Select
                aria-label="Dossier accordé par Session, Toujours ou Jamais"
                title="Dossier accordé par Session, Toujours ou Jamais"
                className="font-mono text-xs"
                disabled={disabled}
                value={folder}
                onChange={(event) => setFolder(event.target.value)}
              >
                {approval.folders.map((candidate) => (
                  <option key={candidate} value={candidate}>
                    {shortPath(candidate)}
                  </option>
                ))}
              </Select>
            )}
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
              onClick={() =>
                onAnswer('session', folder !== approval.pattern ? folder : undefined)
              }
            >
              Session
            </Button>
            <Button
              type="button"
              variant="secondary"
              size="sm"
              disabled={disabled}
              title="Règle du projet, visible et supprimable dans Réglages › Règles"
              onClick={() =>
                onAnswer('always', folder !== approval.pattern ? folder : undefined)
              }
            >
              Toujours
            </Button>
            <Button
              type="button"
              variant="danger"
              size="sm"
              disabled={disabled}
              title="Refuser et créer une règle du projet (Réglages › Règles)"
              onClick={() =>
                onAnswer('never', folder !== approval.pattern ? folder : undefined)
              }
            >
              Jamais
            </Button>
          </div>
        </div>
      </Card>
    </div>
  );
}
