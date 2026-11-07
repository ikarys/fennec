import type { DelegationPrefs } from '../../lib/api';
import { Section, type SectionSave } from '../ui/Section';
import { FieldRow, TextInput } from '../ui/Field';
import { useT } from '../../i18n';

interface DelegationSectionProps {
  value: DelegationPrefs;
  onChange: (value: DelegationPrefs) => void;
  /** The group's own save: its dot and its "Save". */
  save?: SectionSave;
}

export function DelegationSection({ value, onChange, save }: DelegationSectionProps) {
  const t = useT();
  return (
    <Section title={t('delegationTitle')} save={save}>
      <p className="text-xs text-muted">{t('delegationHint')}</p>

      <label className="flex items-center gap-2 text-sm">
        <input
          type="checkbox"
          checked={value.enabled}
          onChange={(event) => onChange({ ...value, enabled: event.target.checked })}
        />
        {t('allowDelegation')}
      </label>

      {value.enabled && (
        <FieldRow label={t('maxDelegationsPerTurnLabel')} hint={t('maxDelegationsPerTurnHint')}>
          <TextInput
            type="number"
            min={0}
            step={1}
            className="w-full"
            value={value.maxPerTurn}
            onChange={(event) =>
              onChange({
                ...value,
                maxPerTurn: Math.max(0, Math.floor(Number(event.target.value) || 0)),
              })
            }
          />
        </FieldRow>
      )}
    </Section>
  );
}
