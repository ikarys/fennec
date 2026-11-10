import { ChoiceMenu } from '../ui/ChoiceMenu';
import { CpuIcon } from '../ui/Icons';
import { REASONING_EFFORTS, type ReasoningEffort } from '../../lib/api';
import { useT } from '../../i18n';

/** The session's reasoning effort in the composer: auto, then the efforts the model accepts (all when unknown). */
export function EffortPicker({
  effort,
  accepted,
  onChange
}: {
  effort: ReasoningEffort | null;
  accepted: ReasoningEffort[];
  onChange: (effort: ReasoningEffort | null) => void;
}) {
  const t = useT();
  const offered = accepted.length > 0 ? accepted : REASONING_EFFORTS;

  return (
    <ChoiceMenu
      label={t('sessionEffort')}
      title={t('sessionEffortTitle')}
      align="right"
      width="w-40"
      groups={[
        {
          choices: [
            { value: 'auto', label: t('effortAutoShort') },
            ...offered.map((value) => ({ value, label: value }))
          ]
        }
      ]}
      value={effort ?? 'auto'}
      onChange={(value) => onChange(value === 'auto' ? null : (value as ReasoningEffort))}
      trigger={
        <>
          <CpuIcon className="h-3.5 w-3.5" />
          {effort ?? t('effortAutoShort')}
        </>
      }
    />
  );
}
