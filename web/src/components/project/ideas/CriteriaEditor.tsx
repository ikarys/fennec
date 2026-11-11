/**
 * The sheet's "done when" list: one row per criterion (text and optional check
 * command), saved as a whole on blur. A new row stays local until it is typed.
 */
import { useEffect, useState } from 'react';
import { Button } from '../../ui/Button';
import { TextInput } from '../../ui/Field';
import { PlusIcon, TrashIcon } from '../../ui/Icons';
import type { SheetCriterion } from '../../../lib/ideas';
import { useT } from '../../../i18n';

export function CriteriaEditor({
  criteria,
  editable,
  onSave,
}: {
  criteria: SheetCriterion[];
  editable: boolean;
  onSave: (criteria: SheetCriterion[]) => void;
}) {
  const t = useT();
  const [rows, setRows] = useState(criteria);
  useEffect(() => setRows(criteria), [criteria]);

  // A row added but never typed is not saved.
  const save = (next: SheetCriterion[]) => onSave(next.filter((row) => row.text.trim()));
  const change = (index: number, next: SheetCriterion) => save(rows.map((row, i) => (i === index ? next : row)));

  return (
    <div className="flex flex-col gap-2">
      {rows.map((row, i) => (
        <div key={i} className="flex items-center gap-2">
          <TextInput
            key={`text-${i}-${row.text}`}
            className="grow"
            defaultValue={row.text}
            placeholder={t('criterionText')}
            disabled={!editable}
            onBlur={(e) => {
              if (e.target.value !== row.text) change(i, { ...row, text: e.target.value });
            }}
          />
          <TextInput
            key={`check-${i}-${row.check ?? ''}`}
            className="w-56 font-mono text-xs"
            defaultValue={row.check ?? ''}
            placeholder={t('criterionCommand')}
            title={t('criterionCommandTitle')}
            disabled={!editable}
            onBlur={(e) => {
              if (e.target.value !== (row.check ?? '')) change(i, { ...row, check: e.target.value || undefined });
            }}
          />
          <Button
            variant="danger"
            size="sm"
            iconOnly
            title={t('criterionRemove')}
            aria-label={t('criterionRemove')}
            disabled={!editable}
            onClick={() => save(rows.filter((_, j) => j !== i))}
          >
            <TrashIcon className="h-3.5 w-3.5" />
          </Button>
        </div>
      ))}
      <Button
        variant="ghost"
        size="sm"
        className="self-start"
        disabled={!editable}
        onClick={() => setRows([...rows, { text: '' }])}
      >
        <PlusIcon className="h-4 w-4" />
        {t('criterionAdd')}
      </Button>
    </div>
  );
}
