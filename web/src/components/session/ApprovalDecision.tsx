/**
 * The pending rule approval. It is a row of the decision bar so the human
 * can allow or refuse the paused call with one click.
 */
import { useState } from 'react';
import type { ApprovalChoice, PendingApproval } from '../../lib/api/sessions';
import { shortPath } from '../../lib/shortPath';
import { DecisionRow } from './DecisionBar';
import { KeyIcon } from '../ui/Icons';
import { Button } from '../ui/Button';
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

export function ApprovalDecision({
  approval,
  onAnswer,
  disabled,
  open,
  onToggle
}: {
  approval: PendingApproval;
  onAnswer: (choice: ApprovalChoice, pattern?: string) => void;
  disabled: boolean;
  open: boolean;
  onToggle: () => void;
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
  const onceButton = (
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
  );

  return (
    <DecisionRow
      icon={<KeyIcon className="h-3.5 w-3.5" />}
      title={<span className="font-mono">{approval.kind === 'path' ? shortPath(approval.subject) : approval.subject}</span>}
      meta={`${t('approvalBadge')} · ${action}`}
      primary={onceButton}
      open={open}
      onToggle={onToggle}
      footer={
        <>
          <span className="flex-1" />
          <Button
            type="button"
            variant="danger"
            size="sm"
            disabled={disabled}
            title={t('neverTitle')}
            onClick={() => onAnswer('never', folder !== approval.pattern ? folder : undefined)}
          >
            {t('never')}
          </Button>
          <Button
            type="button"
            variant="secondary"
            size="sm"
            disabled={disabled}
            title={t('alwaysTitle')}
            onClick={() => onAnswer('always', folder !== approval.pattern ? folder : undefined)}
          >
            {t('always')}
          </Button>
          <Button
            type="button"
            variant="secondary"
            size="sm"
            disabled={disabled}
            title={t('sessionTitle', { scope: scope.pattern + (scope.suffix ?? '') })}
            onClick={() => onAnswer('session', folder !== approval.pattern ? folder : undefined)}
          >
            {t('session')}
          </Button>
          {onceButton}
        </>
      }
    >
      <div className="flex flex-wrap items-center gap-2">
        <span className="text-muted">{action}</span>
        <span className="font-mono text-text break-all" title={approval.subject}>
          {approval.subject}
        </span>
      </div>
      {approval.folders.length > 1 && (
        <div className="mt-2 flex items-center gap-2">
          <span className="text-xs text-muted">{t('folderScopeLabel')}</span>
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
        </div>
      )}
    </DecisionRow>
  );
}
