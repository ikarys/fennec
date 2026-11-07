import { Section, type SectionSave } from '../ui/Section';
import { FieldRow, TextInput } from '../ui/Field';
import { useT } from '../../i18n';

export interface SearchForm {
  url: string;
  apiKey: string;
  webFetch: boolean;
}

export function SearchSection({
  value,
  onChange,
  save,
}: {
  value: SearchForm;
  onChange: (value: SearchForm) => void;
  /** The group's own save: its dot and its "Save". */
  save?: SectionSave;
}) {
  const t = useT();
  return (
    <Section title={t('webSearchTitle')} save={save}>
      <FieldRow label={t('searxngUrlLabel')}>
        <TextInput
          type="url"
          value={value.url}
          onChange={(event) => onChange({ ...value, url: event.target.value })}
          placeholder="http://127.0.0.1:8080"
          className="w-full"
        />
      </FieldRow>
      <FieldRow label={t('searchApiKeyLabel')} hint={t('searchApiKeyHint')}>
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
            className="h-4 w-4"
          />
          {t('allowWebFetch')}
        </label>
        <p className="text-xs text-muted">{t('webFetchHint')}</p>
      </div>
    </Section>
  );
}
