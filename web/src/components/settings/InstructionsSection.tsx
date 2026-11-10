// Global instructions settings section.
import { Section, type SectionSave } from '../ui/Section';
import { TextArea } from '../ui/Field';
import { FileIcon } from '../ui/Icons';
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
    <Section
      Icon={FileIcon}
      title={t('globalInstructionsTitle')}
      description={t('instructionsHint')}
      save={save}
    >
      <TextArea
        aria-label={t('instructionsLabel')}
        rows={16}
        value={value}
        onChange={(event) => onChange(event.target.value)}
        placeholder={t('instructionsPlaceholder')}
        className="w-full"
      />
    </Section>
  );
}
