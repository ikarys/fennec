// Search settings: SearXNG URL, API key, and web fetch toggle.
import { Section, type SectionSave } from '../ui/Section';
import { GlobeIcon } from '../ui/Icons';
import { SettingRow } from '../ui/SettingRow';
import { TextInput } from '../ui/Field';
import { Switch } from '../ui/Switch';
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
    <Section Icon={GlobeIcon} flush title={t('webSearchTitle')} save={save}>
      <SettingRow label={t('searxngUrlLabel')}>
        <TextInput
          type="url"
          value={value.url}
          onChange={(event) => onChange({ ...value, url: event.target.value })}
          placeholder="http://127.0.0.1:8080"
          className="w-72 max-w-full"
          aria-label={t('searxngUrlLabel')}
        />
      </SettingRow>
      <SettingRow label={t('searchApiKeyLabel')} hint={t('searchApiKeyHint')}>
        <TextInput
          type="password"
          value={value.apiKey}
          onChange={(event) => onChange({ ...value, apiKey: event.target.value })}
          className="w-72 max-w-full"
          aria-label={t('searchApiKeyLabel')}
        />
      </SettingRow>
      <SettingRow label={t('allowWebFetch')} hint={t('webFetchHint')}>
        <Switch
          checked={value.webFetch}
          onChange={(webFetch) => onChange({ ...value, webFetch })}
          label={t('allowWebFetch')}
        />
      </SettingRow>
    </Section>
  );
}
