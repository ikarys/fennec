import { Section, type SectionSave } from '../ui/Section';
import { FieldRow, TextArea } from '../ui/Field';
import { useT } from '../../i18n';

export function InstructionsSection({
  value,
  onChange,
  save,
}: {
  value: string;
  onChange: (value: string) => void;
  /** The group's own save: its dot and its "Save". */
  save?: SectionSave;
}) {
  const t = useT();
  return (
    <Section title={t('globalInstructionsTitle')} save={save}>
      <FieldRow label={t('instructionsLabel')} hint={t('instructionsHint')}>
        <TextArea
          rows={16}
          value={value}
          onChange={(event) => onChange(event.target.value)}
          placeholder={t('instructionsPlaceholder')}
          className="w-full"
        />
      </FieldRow>
    </Section>
  );
}
