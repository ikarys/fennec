import type { TariffPrefs } from '../../lib/api';
import { Section } from '../ui/Section';
import { FieldRow, TextInput } from '../ui/Field';

interface TariffSectionProps {
  value: TariffPrefs;
  onChange: (value: TariffPrefs) => void;
}

export function TariffSection({ value, onChange }: TariffSectionProps) {
  return (
    <Section title="Coût">
      <p className="text-xs text-muted">
        Le tarif de référence d'un modèle payant, pour estimer sur la page Insight ce que tes tokens auraient coûté. Le
        modèle local ne coûte rien : c'est une estimation.
      </p>

      <div className="grid gap-2 md:grid-cols-3">
        <FieldRow label="Entrée ($ / M tokens)" hint="Tokens d'entrée hors cache.">
          <TextInput
            type="number"
            min={0}
            step={0.01}
            className="w-full"
            value={value.inputPerM}
            onChange={(event) =>
              onChange({
                ...value,
                inputPerM: Math.max(0, Number(event.target.value) || 0),
              })
            }
          />
        </FieldRow>

        <FieldRow label="Cache ($ / M tokens)" hint="Tokens relus du cache.">
          <TextInput
            type="number"
            min={0}
            step={0.01}
            className="w-full"
            value={value.cachedPerM}
            onChange={(event) =>
              onChange({
                ...value,
                cachedPerM: Math.max(0, Number(event.target.value) || 0),
              })
            }
          />
        </FieldRow>

        <FieldRow label="Sortie ($ / M tokens)" hint="Tokens générés.">
          <TextInput
            type="number"
            min={0}
            step={0.01}
            className="w-full"
            value={value.outputPerM}
            onChange={(event) =>
              onChange({
                ...value,
                outputPerM: Math.max(0, Number(event.target.value) || 0),
              })
            }
          />
        </FieldRow>
      </div>
    </Section>
  );
}
