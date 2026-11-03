import { Section, type SectionSave } from '../ui/Section';
import { FieldRow, TextArea } from '../ui/Field';

export function InstructionsSection({
  value,
  onChange,
  save,
}: {
  value: string;
  onChange: (value: string) => void;
  /** The group's own save: its dot and its "Enregistrer". */
  save?: SectionSave;
}) {
  return (
    <Section title="Instructions globales" save={save}>
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
