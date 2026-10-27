export interface SearchForm {
  url: string;
  apiKey: string;
}

const inputClass = 'mt-1 w-full rounded-lg border border-border bg-panel px-3 py-2 font-mono text-[0.8125rem] text-text';

export function SearchSection({ value, onChange }: { value: SearchForm; onChange: (value: SearchForm) => void }) {
  return (
    <section className="rounded-xl border border-border bg-panel px-5 py-4">
      <h2 className="text-[0.9375rem] font-semibold">Recherche web</h2>
      <label className="mt-3 block text-xs text-muted">
        URL de l’instance SearXNG
        <input
          type="url"
          value={value.url}
          onChange={(event) => onChange({ ...value, url: event.target.value })}
          placeholder="http://127.0.0.1:8080"
          className={inputClass}
        />
      </label>
      <label className="mt-3 block text-xs text-muted">
        Clé d’API (facultative)
        <input
          type="password"
          value={value.apiKey}
          onChange={(event) => onChange({ ...value, apiKey: event.target.value })}
          className={inputClass}
        />
      </label>
      <p className="mt-2 text-xs text-muted">
        Utilisée par l’outil web_search. L’instance doit accepter le format JSON (search.formats: json dans settings.yml).
      </p>
    </section>
  );
}
