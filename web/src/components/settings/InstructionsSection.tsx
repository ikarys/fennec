export function InstructionsSection({ value, onChange }: { value: string; onChange: (value: string) => void }) {
  return (
    <section className="rounded-xl border border-border bg-panel px-5 py-4">
      <h2 className="text-[15px] font-semibold">Instructions globales</h2>
      <textarea
        rows={6}
        value={value}
        onChange={(event) => onChange(event.target.value)}
        placeholder="Ex. : Réponds en français, en quelques lignes, sans récapitulatif."
        className="mt-3 w-full resize-y rounded-lg border border-border bg-panel px-3 py-2 font-mono text-[13px] text-text"
      />
      <p className="mt-2 text-xs text-muted">
        Ajoutées au prompt système de chaque nouvelle session, avant les fichiers AGENTS.md du projet. Une session en cours garde le prompt de son premier tour.
      </p>
    </section>
  );
}
