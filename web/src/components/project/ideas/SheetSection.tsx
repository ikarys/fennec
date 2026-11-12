/**
 * One field of the task sheet: a tinted header (label, required and model
 * badges, suggest button), the field itself, and a pending model draft to
 * accept or reject.
 */
import type { ReactNode } from 'react';
import { Badge } from '../../ui/Badge';
import { Button } from '../../ui/Button';
import { Card } from '../../ui/Card';
import { CheckIcon, CloseIcon, SpinnerIcon, WandIcon } from '../../ui/Icons';
import type { SheetField } from '../../../lib/ideas';
import type { MsgKey } from '../../../i18n/messages';
import { useT } from '../../../i18n';

export const FIELD_LABELS: Record<SheetField, MsgKey> = {
  objective: 'sheetObjective',
  context: 'sheetContext',
  constraints: 'sheetConstraints',
  outOfScope: 'sheetOutOfScope',
  deliverable: 'sheetDeliverable',
  approach: 'sheetApproach',
  doneWhen: 'sheetDoneWhen',
};

export const FIELD_HELP: Record<SheetField, MsgKey> = {
  objective: 'sheetObjectiveHelp',
  context: 'sheetContextHelp',
  constraints: 'sheetConstraintsHelp',
  outOfScope: 'sheetOutOfScopeHelp',
  deliverable: 'sheetDeliverableHelp',
  approach: 'sheetApproachHelp',
  doneWhen: 'sheetDoneWhenHelp',
};

export function SheetSection({
  field,
  required,
  model,
  editable,
  suggesting,
  onSuggest,
  draft,
  onAccept,
  onReject,
  children,
}: {
  field: SheetField;
  required: boolean;
  /** A model wrote this field last: indicative. */
  model: boolean;
  editable: boolean;
  /** A suggestion is on its way: the button spins and waits. */
  suggesting: boolean;
  onSuggest: () => void;
  /** The model's pending proposal, rendered; null when none. */
  draft: ReactNode | null;
  onAccept: () => void;
  onReject: () => void;
  children: ReactNode;
}) {
  const t = useT();
  return (
    <Card className="overflow-hidden">
      <div className="flex items-center gap-2 border-b border-border bg-raised px-3 py-1.5">
        <span className="text-sm font-medium">{t(FIELD_LABELS[field])}</span>
        {required ? <Badge tone="attention">{t('sheetRequired')}</Badge> : null}
        {model ? (
          <Badge tone="info" title={t('sheetModelTitle')}>
            {t('sheetModel')}
          </Badge>
        ) : null}
        {editable ? (
          <Button
            variant="ghost"
            size="sm"
            iconOnly
            className="ml-auto"
            title={t('sheetSuggest')}
            aria-label={t('sheetSuggest')}
            aria-busy={suggesting}
            disabled={suggesting}
            onClick={onSuggest}
          >
            {suggesting ? <SpinnerIcon className="h-4 w-4" /> : <WandIcon className="h-4 w-4" />}
          </Button>
        ) : null}
      </div>
      <div className="flex flex-col gap-2 p-3">
        {children}
        {draft !== null ? (
          <div className="rounded-md border border-dashed border-accent bg-accent-soft/30 p-2 text-sm">
            {draft}
            <div className="mt-2 flex items-center gap-1">
              <Button variant="soft" size="sm" iconOnly title={t('sheetAccept')} aria-label={t('sheetAccept')} onClick={onAccept}>
                <CheckIcon className="h-4 w-4" />
              </Button>
              <Button variant="ghost" size="sm" iconOnly title={t('sheetReject')} aria-label={t('sheetReject')} onClick={onReject}>
                <CloseIcon className="h-4 w-4" />
              </Button>
            </div>
          </div>
        ) : null}
      </div>
    </Card>
  );
}
