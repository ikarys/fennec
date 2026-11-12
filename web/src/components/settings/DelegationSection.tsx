// Delegation settings section.
import type { DelegationPrefs } from '../../lib/api';
import { Section, type SectionSave } from '../ui/Section';
import { TextInput } from '../ui/Field';
import { SettingRow } from '../ui/SettingRow';
import { SplitIcon } from '../ui/Icons';
import { Switch } from '../ui/Switch';
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
    <Section
      Icon={SplitIcon}
      flush
      title={t('delegationTitle')}
      description={t('delegationHint')}
      save={save}
    >
      <SettingRow label={t('allowDelegation')}>
        <Switch
          checked={value.enabled}
          onChange={(checked) => onChange({ ...value, enabled: checked })}
          label={t('allowDelegation')}
        />
      </SettingRow>

      {value.enabled && (
        <SettingRow
          label={t('maxDelegationsPerTurnLabel')}
          hint={t('maxDelegationsPerTurnHint')}
        >
          <TextInput
            type="number"
            min={0}
            step={1}
            className="w-24"
            aria-label={t('maxDelegationsPerTurnLabel')}
            value={value.maxPerTurn}
            onChange={(event) =>
              onChange({
                ...value,
                maxPerTurn: Math.max(0, Math.floor(Number(event.target.value) || 0)),
              })
            }
          />
        </SettingRow>
      )}
    </Section>
  );
}
