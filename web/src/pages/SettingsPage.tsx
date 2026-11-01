import { useEffect, useState, type FormEvent } from 'react';
import type {
  ApiKey,
  DisplayPrefs,
  ModelPrefs,
  ModelRef,
  Preferences,
  Provider,
  ProviderListing,
  ReasoningEffort,
  SandboxPrefs,
  AutonomyPrefs,
  SettingsResponse
} from '../lib/api';
import { listProvider, saveSettings } from '../lib/api';
import { type ThemeId } from '../lib/theme';
import { Button } from '../components/ui/Button';
import { Tabs } from '../components/ui/Tabs';
import { ModelSection } from '../components/settings/ModelSection';
import { ProvidersSection, type ProviderForm } from '../components/settings/ProvidersSection';
import { ModelPrefsSection, percentFromFraction, type ModelForm } from '../components/settings/ModelPrefsSection';
import { DisplaySection } from '../components/settings/DisplaySection';
import { PanelPagesSection } from '../components/settings/PanelPagesSection';
import { InstructionsSection } from '../components/settings/InstructionsSection';
import { SearchSection, type SearchForm } from '../components/settings/SearchSection';
import { SandboxSection } from '../components/settings/SandboxSection';
import { RulesSection } from '../components/rules/RulesSection';
import { AutonomySection } from '../components/settings/AutonomySection';
import { ScannersSection } from '../components/settings/ScannersSection';
import { SkillsSection } from '../components/settings/SkillsSection';
import { WorkflowsSection } from '../components/settings/WorkflowsSection';

interface SettingsPageProps {
  /** The tab named in the address (`#/settings/rules`), when it names one. */
  initialTab?: string | null;
  settings: SettingsResponse | null;
  theme: ThemeId;
  onThemeChange: (theme: ThemeId) => void;
  onSaved: (next: SettingsResponse) => void;
  onListing: (listing: ProviderListing) => void;
}

interface SettingsForm {
  model: string;
  favorites: ModelRef[];
  providers: ProviderForm[];
  models: Record<string, ModelForm>;
  display: DisplayPrefs;
  instructions: string;
  search: SearchForm;
  sandbox: SandboxPrefs;
  autonomy: AutonomyPrefs;
  alwaysOnSkills: string[];
  enabledWorkflows: string[];
}

type SettingsTab = 'models' | 'instructions' | 'skills' | 'workflows' | 'search' | 'sandbox' | 'rules' | 'security' | 'autonomy' | 'display';

const TABS: { id: SettingsTab; label: string }[] = [
  { id: 'models', label: 'Modèles' },
  { id: 'instructions', label: 'Instructions' },
  { id: 'skills', label: 'Skills' },
  { id: 'workflows', label: 'Workflows' },
  { id: 'search', label: 'Recherche web' },
  { id: 'sandbox', label: 'Bac à sable' },
  { id: 'rules', label: 'Règles' },
  { id: 'security', label: 'Sécurité' },
  { id: 'autonomy', label: 'Mode nuit' },
  { id: 'display', label: 'Affichage' }
];

const emptyModelForm: ModelForm = {
  maxTokens: '',
  contextWindow: '',
  compactionThreshold: '',
  reasoningEffort: '',
  priceInput: '',
  priceCached: '',
  priceOutput: ''
};

function localProviderForm(): ProviderForm {
  return {
    id: 'local',
    name: '',
    baseUrl: '',
    keyKind: 'none',
    envVar: '',
    newKey: '',
    sealed: null
  };
}

function providerToForm(provider: Provider): ProviderForm {
  let keyKind: ProviderForm['keyKind'] = 'none';
  let envVar = '';
  let newKey = '';
  let sealed: string | null = null;

  if (provider.apiKey) {
    if ('env' in provider.apiKey) {
      keyKind = 'env';
      envVar = provider.apiKey.env;
    } else if ('sealed' in provider.apiKey) {
      keyKind = 'stored';
      sealed = provider.apiKey.sealed;
    } else {
      // A plain key should not come back from the server; keep it editable if it does.
      keyKind = 'stored';
      newKey = provider.apiKey.plain;
    }
  }

  return {
    id: provider.id,
    name: provider.name,
    baseUrl: provider.baseUrl,
    keyKind,
    envVar,
    newKey,
    sealed
  };
}

/** The API key a provider form carries, as the wire value (null when none). */
function providerFormToApiKey(provider: ProviderForm): ApiKey | null {
  if (provider.keyKind === 'env') {
    const envVar = provider.envVar.trim();
    return envVar === '' ? null : { env: envVar };
  }
  if (provider.keyKind === 'stored') {
    const newKey = provider.newKey.trim();
    if (newKey !== '') return { plain: newKey };
    return provider.sealed ? { sealed: provider.sealed } : null;
  }
  return null;
}

function toForm(preferences: Preferences, listings: ProviderListing[]): SettingsForm {
  const providerForms = preferences.providers.map(providerToForm);

  if (!providerForms.some((provider) => provider.id === 'local')) {
    providerForms.unshift(localProviderForm());
  } else {
    const localIndex = providerForms.findIndex((provider) => provider.id === 'local');
    if (localIndex > 0) {
      const [local] = providerForms.splice(localIndex, 1);
      providerForms.unshift(local);
    }
  }

  const models: Record<string, ModelForm> = {};
  for (const listing of listings) {
    const storedProvider = preferences.providers.find((provider) => provider.id === listing.provider);
    for (const model of listing.models) {
      const key = `${listing.provider}/${model.id}`;
      const prefs = storedProvider?.models[model.id] ?? {};
      models[key] = {
        maxTokens: prefs.maxTokens == null ? '' : String(prefs.maxTokens),
        contextWindow: prefs.contextWindow == null ? '' : String(prefs.contextWindow),
        compactionThreshold:
          prefs.compactionThreshold == null ? '' : percentFromFraction(prefs.compactionThreshold),
        reasoningEffort: prefs.reasoningEffort ?? '',
        priceInput: prefs.price ? String(prefs.price.inputPerM) : '',
        priceCached: prefs.price ? String(prefs.price.cachedPerM) : '',
        priceOutput: prefs.price ? String(prefs.price.outputPerM) : ''
      };
    }
  }
  return {
    model: preferences.selected ? `${preferences.selected.provider}/${preferences.selected.model}` : '',
    favorites: preferences.favorites ?? [],
    providers: providerForms,
    models,
    display: preferences.display,
    instructions: preferences.globalInstructions ?? '',
    search: {
      url: preferences.search?.searxngUrl ?? '',
      apiKey: preferences.search?.searxngApiKey ?? '',
      webFetch: preferences.search?.webFetch ?? true
    },
    sandbox: preferences.sandbox ?? { enabled: true, network: false },
    autonomy: preferences.autonomy ?? { maxMinutesPerIdea: 120, maxTokensPerIdea: 2_000_000 },
    alwaysOnSkills: preferences.alwaysOnSkills ?? [],
    enabledWorkflows: preferences.enabledWorkflows ?? []
  };
}

function parseNumber(value: string): number | null {
  const trimmed = value.trim();
  if (trimmed === '') return null;
  const parsed = Number(trimmed);
  return Number.isFinite(parsed) ? parsed : null;
}

function parsePercent(value: string): number | null {
  const trimmed = value.trim();
  if (trimmed === '') return null;
  const parsed = Number(trimmed);
  if (!Number.isFinite(parsed)) return null;
  return parsed / 100;
}

function buildPreferences(form: SettingsForm, settings: SettingsResponse): Preferences {
  const selected: ModelRef | null = form.model === '' ? null : (() => {
    const slash = form.model.indexOf('/');
    if (slash === -1) return null;
    return {
      provider: form.model.slice(0, slash),
      model: form.model.slice(slash + 1)
    };
  })();

  const providers: Provider[] = form.providers.map((providerForm) => {
    const listing = settings.listings.find((item) => item.provider === providerForm.id);
    const listedIds = new Set(listing?.models.map((model) => model.id) ?? []);
    const models: Record<string, ModelPrefs> = {};

    // Every model of the form is written from the form, listed or added by hand.
    // A listed one with nothing set is dropped; a hand-added one is kept even
    // empty: its entry is what makes it exist for the provider.
    const prefix = `${providerForm.id}/`;
    for (const [key, entry] of Object.entries(form.models)) {
      if (!key.startsWith(prefix)) continue;
      const modelId = key.slice(prefix.length);
      const maxTokens = parseNumber(entry.maxTokens);
      const contextWindow = parseNumber(entry.contextWindow);
      const compactionThreshold = parsePercent(entry.compactionThreshold);
      const priceInput = parseNumber(entry.priceInput);
      const priceCached = parseNumber(entry.priceCached);
      const priceOutput = parseNumber(entry.priceOutput);
      const prefs: ModelPrefs = {};
      if (maxTokens != null) prefs.maxTokens = maxTokens;
      if (contextWindow != null) prefs.contextWindow = contextWindow;
      if (compactionThreshold != null) prefs.compactionThreshold = compactionThreshold;
      if (entry.reasoningEffort !== '') prefs.reasoningEffort = entry.reasoningEffort as ReasoningEffort;
      if (entry.priceInput !== '' || entry.priceCached !== '' || entry.priceOutput !== '') {
        prefs.price = {
          inputPerM: priceInput ?? 0,
          cachedPerM: priceCached ?? 0,
          outputPerM: priceOutput ?? 0
        };
      }
      if (Object.keys(prefs).length === 0 && listedIds.has(modelId)) continue;
      models[modelId] = prefs;
    }

    const apiKey = providerFormToApiKey(providerForm);

    return {
      id: providerForm.id,
      name: providerForm.name,
      baseUrl: providerForm.baseUrl,
      apiKey,
      models
    };
  });

  const searxngUrl = form.search.url.trim();
  const searxngApiKey = form.search.apiKey.trim();

  return {
    selected,
    favorites: form.favorites,
    providers,
    display: form.display,
    globalInstructions: form.instructions.trim() === '' ? null : form.instructions,
    search: {
      searxngUrl: searxngUrl === '' ? null : searxngUrl,
      searxngApiKey: searxngApiKey === '' ? null : searxngApiKey,
      webFetch: form.search.webFetch
    },
    sandbox: form.sandbox,
    autonomy: form.autonomy,
    alwaysOnSkills: form.alwaysOnSkills,
    enabledWorkflows: form.enabledWorkflows
  };
}

export function SettingsPage({ initialTab, settings, theme, onThemeChange, onSaved, onListing }: SettingsPageProps) {
  const [form, setForm] = useState<SettingsForm | null>(null);
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [tab, setTab] = useState<SettingsTab>(() => TABS.find((t) => t.id === initialTab)?.id ?? 'models');
  const [dirty, setDirty] = useState(false);

  useEffect(() => {
    if (settings && form === null) {
      setForm(toForm(settings.preferences, settings.listings));
    }
  }, [settings, form]);

  function markDirty() {
    setSaved(false);
    setError(null);
    setDirty(true);
  }

  function updateModel(value: string) {
    markDirty();
    setForm((previous) => (previous ? { ...previous, model: value } : previous));
  }

  function toggleFavorite(ref: ModelRef) {
    markDirty();
    setForm((previous) => {
      if (!previous) return previous;
      const exists = previous.favorites.some(
        (favorite) => favorite.provider === ref.provider && favorite.model === ref.model
      );
      const favorites = exists
        ? previous.favorites.filter(
            (favorite) => !(favorite.provider === ref.provider && favorite.model === ref.model)
          )
        : [...previous.favorites, ref];
      return { ...previous, favorites };
    });
  }

  function updateModelField(key: string, field: keyof ModelForm, value: string) {
    markDirty();
    setForm((previous) => {
      if (!previous) return previous;
      const current = previous.models[key] ?? emptyModelForm;
      return {
        ...previous,
        models: {
          ...previous.models,
          [key]: { ...current, [field]: value }
        }
      };
    });
  }

  function updateProvider(index: number, patch: Partial<ProviderForm>) {
    markDirty();
    setForm((previous) => {
      if (!previous) return previous;
      return {
        ...previous,
        providers: previous.providers.map((provider, itemIndex) =>
          itemIndex === index ? { ...provider, ...patch } : provider
        )
      };
    });
  }

  function addProvider(provider: ProviderForm) {
    markDirty();
    setForm((previous) =>
      previous ? { ...previous, providers: [...previous.providers, provider] } : previous
    );
  }

  /** Adds a model by hand: an entry with no listed source, saved as an override. */
  function addModel(providerId: string, modelId: string) {
    markDirty();
    setForm((previous) => {
      if (!previous) return previous;
      const key = `${providerId}/${modelId}`;
      if (key in previous.models) return previous;
      return { ...previous, models: { ...previous.models, [key]: emptyModelForm } };
    });
  }

  /** Removes a hand-added model: its entry and everything that pointed at it. */
  function removeModel(providerId: string, modelId: string) {
    markDirty();
    setForm((previous) => {
      if (!previous) return previous;
      const key = `${providerId}/${modelId}`;
      const models = { ...previous.models };
      delete models[key];
      const favorites = previous.favorites.filter(
        (favorite) => !(favorite.provider === providerId && favorite.model === modelId)
      );
      const model =
        previous.model === key ? '' : previous.model;
      return { ...previous, models, favorites, model };
    });
  }

  function removeProvider(index: number) {
    markDirty();
    setForm((previous) => {
      if (!previous) return previous;
      const provider = previous.providers[index];
      const providers = previous.providers.filter((_, itemIndex) => itemIndex !== index);
      const model = provider && previous.model.startsWith(`${provider.id}/`) ? '' : previous.model;

      const models: Record<string, ModelForm> = {};
      for (const [key, value] of Object.entries(previous.models)) {
        if (!provider || !key.startsWith(`${provider.id}/`)) {
          models[key] = value;
        }
      }

      return { ...previous, providers, model, models };
    });
  }

  /** Adds empty per-model entries for models a fresh listing surfaced. */
  function mergeFormModels(listing: ProviderListing) {
    setForm((previous) => {
      if (!previous) return previous;
      const models = { ...previous.models };
      for (const model of listing.models) {
        const key = `${listing.provider}/${model.id}`;
        if (!(key in models)) {
          models[key] = emptyModelForm;
        }
      }
      return { ...previous, models };
    });
  }

  /** Re-lists a provider now (its baseUrl/key may not be stored yet). */
  async function handleFetch(provider: ProviderForm): Promise<void> {
    const listing = await listProvider({
      provider: provider.id,
      baseUrl: provider.baseUrl,
      apiKey: providerFormToApiKey(provider)
    });
    onListing(listing);
    mergeFormModels(listing);
  }

  function updateDisplay(display: DisplayPrefs) {
    markDirty();
    setForm((previous) => (previous ? { ...previous, display } : previous));
  }

  function updateInstructions(instructions: string) {
    markDirty();
    setForm((previous) => (previous ? { ...previous, instructions } : previous));
  }

  function updateSearch(search: SearchForm) {
    markDirty();
    setForm((previous) => (previous ? { ...previous, search } : previous));
  }

  function updateSandbox(sandbox: SandboxPrefs) {
    markDirty();
    setForm((previous) => (previous ? { ...previous, sandbox } : previous));
  }

  function updateAutonomy(autonomy: AutonomyPrefs) {
    markDirty();
    setForm((previous) => (previous ? { ...previous, autonomy } : previous));
  }

  function updateAlwaysOnSkills(alwaysOnSkills: string[]) {
    markDirty();
    setForm((previous) => (previous ? { ...previous, alwaysOnSkills } : previous));
  }

  function updateEnabledWorkflows(enabledWorkflows: string[]) {
    markDirty();
    setForm((previous) => (previous ? { ...previous, enabledWorkflows } : previous));
  }

  /** Saves one change at once on top of the stored preferences: the form's other unsaved edits stay pending. */
  async function savePatch(patch: Partial<Preferences>) {
    if (!settings) return;
    setError(null);
    try {
      onSaved(await saveSettings({ ...settings.preferences, ...patch }));
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Impossible d’enregistrer les réglages');
    }
  }

  function setDefaultModel(key: string) {
    updateModel(key);
    const slash = key.indexOf('/');
    void savePatch({
      selected: slash === -1 ? null : { provider: key.slice(0, slash), model: key.slice(slash + 1) }
    });
  }

  async function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!settings || !form || saving) return;
    setSaving(true);
    setSaved(false);
    setError(null);
    try {
      const response = await saveSettings(buildPreferences(form, settings));
      setForm(toForm(response.preferences, response.listings));
      setSaved(true);
      setDirty(false);
      onSaved(response);
    } catch (err) {
      setSaved(false);
      setError(err instanceof Error ? err.message : 'Impossible d’enregistrer les réglages');
    } finally {
      setSaving(false);
    }
  }

  if (!settings || !form) {
    return (
      <main className="grow overflow-y-auto px-4 pt-6 md:px-6 md:pt-8">
        <div className="mx-auto max-w-[960px]">
          <h1 className="text-2xl font-semibold">Réglages</h1>
          <p className="mt-0.5 text-muted">S'applique au prochain tour, sans redémarrage.</p>
          <div className="mt-6 text-sm text-muted">Chargement…</div>
        </div>
      </main>
    );
  }

  return (
    <main className="grow overflow-y-auto px-4 pt-6 md:px-6 md:pt-8">
      <div className="mx-auto flex max-w-[960px] flex-col gap-6">
        <div>
          <h1 className="text-2xl font-semibold">Réglages</h1>
          <p className="mt-0.5 text-muted">S'applique au prochain tour, sans redémarrage.</p>
        </div>

        <div className="flex flex-col gap-6 md:flex-row">
          {/* One form across the tabs: an edit on one tab is kept when switching. */}
          <Tabs label="Sections des réglages" tabs={TABS} active={tab} onSelect={setTab} />

          <form onSubmit={handleSubmit} className="flex min-w-0 grow flex-col gap-6">
            {tab === 'models' && (
              <>
                <ModelSection
                  value={form.model}
                  listings={settings.listings}
                  stored={settings.preferences.selected}
                  defaultModel={settings.defaults.model}
                  onChange={updateModel}
                />
                <ProvidersSection
                  providers={form.providers}
                  listings={settings.listings}
                  values={form.models}
                  onChange={updateProvider}
                  onAdd={addProvider}
                  onRemove={removeProvider}
                  onFetch={handleFetch}
                  onAddModel={addModel}
                />
                <ModelPrefsSection
                  listings={settings.listings}
                  values={form.models}
                  defaults={settings.defaults}
                  favorites={form.favorites}
                  onToggleFavorite={toggleFavorite}
                  defaultModel={form.model}
                  onSetDefault={setDefaultModel}
                  onChange={updateModelField}
                  onRemoveModel={removeModel}
                />
              </>
            )}

            {tab === 'instructions' && (
              <InstructionsSection value={form.instructions} onChange={updateInstructions} />
            )}

            {tab === 'skills' && (
              <SkillsSection
                skills={settings.skills}
                value={form.alwaysOnSkills}
                onChange={updateAlwaysOnSkills}
              />
            )}

            {tab === 'workflows' && (
              <WorkflowsSection
                workflows={settings.optInWorkflows}
                value={form.enabledWorkflows}
                onChange={updateEnabledWorkflows}
              />
            )}

            {tab === 'search' && (
              <SearchSection value={form.search} onChange={updateSearch} />
            )}

            {tab === 'sandbox' && (
              <SandboxSection value={form.sandbox} onChange={updateSandbox} health={settings?.sandboxHealth} />
            )}
            {tab === 'rules' && <RulesSection />}
            {tab === 'security' && settings && <ScannersSection scanners={settings.scanners ?? []} />}

            {tab === 'autonomy' && (
              <AutonomySection value={form.autonomy} onChange={updateAutonomy} />
            )}

            {tab === 'display' && (
              <div className="flex flex-col gap-4">
                <DisplaySection
                  value={form.display}
                  onChange={updateDisplay}
                  theme={theme}
                  onThemeChange={onThemeChange}
                />
                <PanelPagesSection
                  value={form.display.panelPages ?? null}
                  onChange={(panelPages) => updateDisplay({ ...form.display, panelPages })}
                />
              </div>
            )}

            {tab !== 'security' && (
              <div className="sticky bottom-0 -mx-1 flex items-center gap-3 border-t border-border bg-bg px-1 py-3">
                <Button type="submit" variant="primary" size="md" disabled={saving}>
                  {saving ? 'Enregistrement…' : 'Enregistrer'}
                </Button>
                {dirty && !saving && <span className="text-xs text-muted">Modifications non enregistrées</span>}
                {saved && <span className="text-xs text-ok">Enregistré</span>}
                {error && <span className="text-xs text-err">{error}</span>}
              </div>
            )}
          </form>
        </div>
      </div>
    </main>
  );
}
