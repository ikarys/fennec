import type { SkillSummary } from '../../lib/api';
import { Section, type SectionSave } from '../ui/Section';
import { OptInList } from './OptInList';
import { useT } from '../../i18n';

interface SkillsSectionProps {
  skills: SkillSummary[];
  value: string[];
  onChange: (value: string[]) => void;
  /** The group's own save: its dot and its "Save". */
  save?: SectionSave;
}

export function SkillsSection({ skills, value, onChange, save }: SkillsSectionProps) {
  const t = useT();

  return (
    <Section
      title={t('alwaysActiveSkillsTitle')}
      description={t('alwaysActiveSkillsHint')}
      save={save}
    >
      <OptInList
        title={t('optInSkillsTitle')}
        items={skills.map((skill) => ({
          id: skill.id,
          label: skill.id,
          mono: true,
          description: skill.description,
        }))}
        value={value}
        onChange={onChange}
        empty={t('noSkillsFound')}
      />
    </Section>
  );
}
