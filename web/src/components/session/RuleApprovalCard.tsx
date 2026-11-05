/**
 * The pending rule approval. It is rendered above the composer so the human
 * can allow or refuse the paused call with one click.
 */
import { useState } from 'react';
import type { ApprovalChoice, PendingApproval } from '../../lib/api/sessions';
import { shortPath } from '../../lib/shortPath';
import { Button } from '../ui/Button';
import { Card } from '../ui/Card';
import { Select } from '../ui/Field';
import { useLang, useT } from '../../i18n';
import { t, type Lang } from '../../i18n/messages';

function approvalAction(lang: Lang, approval: PendingApproval): string {
  if (approval.kind === 'command') return t(lang, 'runAction');
  if (approval.kind === 'tool') return t(lang, 'useAction');
  if (approval.access === 'write') return t(lang, 'writeAction');
  if (approval.access === 'readWrite') return t(lang, 'readWriteAction');
  return t(lang, 'readAction');
}

function sessionScope(
  lang: Lang,
  folder: string,
  kind: PendingApproval['kind']
): { pattern: string; suffix: string | null } {
  const pattern = folder;
  const suffix = kind === 'path' && pattern.endsWith('/') ? t(lang, 'andSubfolders') : null;
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
  const t = useT();
  const lang = useLang();
  const [folder, setFolder] = useState(approval.pattern);
  const [forCall, setForCall] = useState(approval.callId);
  if (forCall !== approval.callId) {
    setForCall(approval.callId);
    setFolder(approval.pattern);
  }

  const action = approvalAction(lang, approval);
  const scope = sessionScope(lang, folder, approval.kind);

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
                aria-label={t('folderScopeLabel')}
                title={t('folderScopeLabel')}
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
              title={t('onceTitle')}
              onClick={() => onAnswer('once')}
            >
              {t('once')}
            </Button>
            <Button
              type="button"
              variant="secondary"
              size="sm"
              disabled={disabled}
              title={t('sessionTitle', { scope: scope.pattern + (scope.suffix ?? '') })}
              onClick={() =>
                onAnswer('session', folder !== approval.pattern ? folder : undefined)
              }
            >
              {t('session')}
            </Button>
            <Button
              type="button"
              variant="secondary"
              size="sm"
              disabled={disabled}
              title={t('alwaysTitle')}
              onClick={() =>
                onAnswer('always', folder !== approval.pattern ? folder : undefined)
              }
            >
              {t('always')}
            </Button>
            <Button
              type="button"
              variant="danger"
              size="sm"
              disabled={disabled}
              title={t('neverTitle')}
              onClick={() =>
                onAnswer('never', folder !== approval.pattern ? folder : undefined)
              }
            >
              {t('never')}
            </Button>
          </div>
        </div>
      </Card>
    </div>
  );
}
