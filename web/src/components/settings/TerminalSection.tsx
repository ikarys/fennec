import { Section, type SectionSave } from '../ui/Section';
import { FieldRow, TextInput } from '../ui/Field';
import { useT } from '../../i18n';

interface TerminalSectionProps {
  value: string;
  onChange: (value: string) => void;
  /** The group's own save: its dot and its "Save". */
  save?: SectionSave;
}

export function TerminalSection({ value, onChange, save }: TerminalSectionProps) {
  const t = useT();
  return (
    <Section title={t('terminalTitle')} save={save}>
      <FieldRow label={t('terminalCommandLabel')} hint={t('terminalCommandHint')}>
        <TextInput
          type="text"
          value={value}
          onChange={(event) => onChange(event.target.value)}
          placeholder="alacritty"
          className="w-full"
        />
      </FieldRow>
    </Section>
  );
}
