// Global instructions and agent style settings section.
import type { AgentStyle } from '../../lib/api';
import { Section, type SectionSave } from '../ui/Section';
import { TextArea } from '../ui/Field';
import { FileIcon } from '../ui/Icons';
import { Segmented } from '../ui/Segmented';
import { SettingRow } from '../ui/SettingRow';
import { useT } from '../../i18n';
import type { MsgKey } from '../../i18n/messages';

const AGENT_STYLES: { value: AgentStyle; label: MsgKey; title: MsgKey }[] = [
  { value: 'concise', label: 'agentStyleConcise', title: 'agentStyleConciseTitle' },
  { value: 'normal', label: 'agentStyleNormal', title: 'agentStyleNormalTitle' },
  { value: 'explanatory', label: 'agentStyleExplanatory', title: 'agentStyleExplanatoryTitle' },
  { value: 'technical', label: 'agentStyleTechnical', title: 'agentStyleTechnicalTitle' }
];

export function InstructionsSection({
  value,
  onChange,
  style,
  onStyleChange,
  save,
}: {
  value: string;
  onChange: (value: string) => void;
  style: AgentStyle;
  onStyleChange: (style: AgentStyle) => void;
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
      <SettingRow label={t('agentStyleLabel')} hint={t('agentStyleHint')}>
        <Segmented
          label={t('agentStyleLabel')}
          value={style}
          onChange={onStyleChange}
          segments={AGENT_STYLES.map((item) => ({
            value: item.value,
            label: t(item.label),
            title: t(item.title)
          }))}
        />
      </SettingRow>
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
