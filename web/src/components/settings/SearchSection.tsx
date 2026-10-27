import { Card } from '../ui/Card';
import { FieldRow, TextInput } from '../ui/Field';

export interface SearchForm {
  url: string;
  apiKey: string;
}

export function SearchSection({ value, onChange }: { value: SearchForm; onChange: (value: SearchForm) => void }) {
  return (
    <Card className="p-4">
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
      </div>
    </Card>
  );
}
