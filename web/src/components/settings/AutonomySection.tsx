import type { AutonomyPrefs } from '../../lib/api';
import { Section, type SectionSave } from '../ui/Section';
import { FieldRow, TextInput } from '../ui/Field';

interface AutonomySectionProps {
  value: AutonomyPrefs;
  onChange: (value: AutonomyPrefs) => void;
  /** The group's own save: its dot and its "Enregistrer". */
  save?: SectionSave;
}

export function AutonomySection({ value, onChange, save }: AutonomySectionProps) {
  return (
    <Section title="Mode nuit" save={save}>
      <p className="text-xs text-muted">
        Lorsque la file tourne en mode autonome, l'agent n'attend pas ta réponse : il tranche ses questions lui-même et
        les note comme décisions. Une idée qui dépasse un plafond est arrêtée et la file passe à la suivante.
      </p>

      <div className="grid gap-2 md:grid-cols-2">
        <FieldRow label="Durée max par idée (minutes)" hint="0 : pas de limite.">
          <TextInput
            type="number"
            min={0}
            step={1}
            className="w-full"
            value={value.maxMinutesPerIdea}
            onChange={(event) =>
              onChange({
                ...value,
                maxMinutesPerIdea: Math.max(0, Math.floor(Number(event.target.value) || 0)),
              })
            }
          />
        </FieldRow>

        <FieldRow label="Tokens max par idée" hint="Tokens non cachés + générés. 0 : pas de limite.">
          <TextInput
            type="number"
            min={0}
            step={1}
            className="w-full"
            value={value.maxTokensPerIdea}
            onChange={(event) =>
              onChange({
                ...value,
                maxTokensPerIdea: Math.max(0, Math.floor(Number(event.target.value) || 0)),
              })
            }
          />
        </FieldRow>
      </div>
    </Section>
  );
}
