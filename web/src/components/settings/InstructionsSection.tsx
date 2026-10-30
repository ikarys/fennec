import { Section } from '../ui/Section';
import { FieldRow, TextArea } from '../ui/Field';

export function InstructionsSection({ value, onChange }: { value: string; onChange: (value: string) => void }) {
  return (
    <Section title="Instructions globales">
      <FieldRow
        label="Instructions"
        hint="Ajoutées au prompt système de chaque nouvelle session, avant les fichiers AGENTS.md du projet. Une session en cours garde le prompt de son premier tour."
      >
        <TextArea
          rows={16}
          value={value}
          onChange={(event) => onChange(event.target.value)}
          placeholder="Ex. : Réponds en français, en quelques lignes, sans récapitulatif."
          className="w-full"
        />
      </FieldRow>
    </Section>
  );
}
