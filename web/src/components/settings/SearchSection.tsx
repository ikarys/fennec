import { Section } from '../ui/Section';
import { FieldRow, TextInput } from '../ui/Field';

export interface SearchForm {
  url: string;
  apiKey: string;
  webFetch: boolean;
}

export function SearchSection({ value, onChange }: { value: SearchForm; onChange: (value: SearchForm) => void }) {
  return (
    <Section>
      <div className="flex flex-col gap-4">
        <h2 className="text-sm font-semibold">Recherche web</h2>
        <FieldRow label="URL de l’instance SearXNG">
          <TextInput
            type="url"
            value={value.url}
            onChange={(event) => onChange({ ...value, url: event.target.value })}
            placeholder="http://127.0.0.1:8080"
            className="w-full"
          />
        </FieldRow>
        <FieldRow
          label="Clé d’API (facultative)"
          hint="Utilisée par l’outil web_search. L’instance doit accepter le format JSON (search.formats: json dans settings.yml)."
        >
          <TextInput
            type="password"
            value={value.apiKey}
            onChange={(event) => onChange({ ...value, apiKey: event.target.value })}
            className="w-full"
          />
        </FieldRow>
        <div className="flex flex-col gap-1">
          <label className="flex items-center gap-2 text-sm text-text">
            <input
              type="checkbox"
              checked={value.webFetch}
              onChange={(event) => onChange({ ...value, webFetch: event.target.checked })}
              className="h-4 w-4 accent-[var(--accent)]"
            />
            Autoriser web_fetch
          </label>
          <p className="text-xs text-muted">
            Le modèle télécharge les pages dont il choisit l’URL (adresses locales et privées refusées) ; fennec les garde en cache 7 jours. Indépendant du réseau du bac à sable : fennec l’exécute lui-même.
          </p>
        </div>
      </div>
    </Section>
  );
}
