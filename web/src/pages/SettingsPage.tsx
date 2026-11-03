import { useEffect, useMemo, useRef, useState, type FormEvent } from 'react';
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
  McpServer,
  SettingsResponse
} from '../lib/api';
import { listProvider, saveSettings } from '../lib/api';
import { type ThemeId } from '../lib/theme';
import { Tabs } from '../components/ui/Tabs';
import { GroupJumps } from '../components/settings/GroupJumps';
import { Card } from '../components/ui/Card';
import type { SectionSave } from '../components/ui/Section';
import { ModelSection } from '../components/settings/ModelSection';
import { ProvidersSection, type ProviderForm } from '../components/settings/ProvidersSection';
import { ModelPrefsSection, percentFromFraction, type ModelForm } from '../components/settings/ModelPrefsSection';
import { DisplaySection } from '../components/settings/DisplaySection';
import { PanelPagesSection } from '../components/settings/PanelPagesSection';
import { InstructionsSection } from '../components/settings/InstructionsSection';
import { SearchSection, type SearchForm } from '../components/settings/SearchSection';
import { SandboxSection } from '../components/settings/SandboxSection';
import { McpSection } from '../components/settings/McpSection';
import { AgentsSection } from '../components/agents/AgentsSection';
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
  mcp: McpServer[];
}

type SettingsTab = 'models' | 'behaviour' | 'rules' | 'library' | 'display';

// Five groups, not twelve tabs: what tunes the model, how an agent behaves, the human's rules,
// the catalogues of content (skills, workflows, agents, MCP) and the display.
const TABS: { id: SettingsTab; label: string }[] = [
  { id: 'models', label: 'Modèles' },
  { id: 'behaviour', label: 'Comportement' },
  { id: 'rules', label: 'Règles' },
  { id: 'library', label: 'Bibliothèque' },
  { id: 'display', label: 'Affichage' }
];

/** The old per-section addresses (`#/settings/skills`) still open the group that holds the section. */
const LEGACY_TABS: Record<string, SettingsTab> = {
  instructions: 'behaviour',
  search: 'behaviour',
  sandbox: 'behaviour',
  security: 'behaviour',
  autonomy: 'behaviour',
  skills: 'library',
  workflows: 'library',
  agents: 'library',
  mcp: 'library'
};

type SaveGroup =
  | 'model'
  | 'providers'
  | 'modelPrefs'
  | 'instructions'
  | 'search'
  | 'sandbox'
  | 'autonomy'
  | 'skills'
  | 'workflows'
  | 'mcp'
  | 'display';

/**
 * What each group saves: its preference keys (stored on top of the others, so the other groups'
 * unsaved edits stay pending) and the form fields it reloads from the answer (a typed API key
 * comes back sealed). Providers and per-model parameters share `providers`.
 */
const SAVE_GROUPS: Record<SaveGroup, { prefs: (keyof Preferences)[]; form: (keyof SettingsForm)[] }> = {
  model: { prefs: ['selected'], form: ['model'] },
  providers: { prefs: ['providers'], form: ['providers', 'models'] },
  modelPrefs: { prefs: ['providers', 'favorites'], form: ['providers', 'models', 'favorites'] },
  instructions: { prefs: ['globalInstructions'], form: ['instructions'] },
  search: { prefs: ['search'], form: ['search'] },
  sandbox: { prefs: ['sandbox'], form: ['sandbox'] },
  autonomy: { prefs: ['autonomy'], form: ['autonomy'] },
  skills: { prefs: ['alwaysOnSkills'], form: ['alwaysOnSkills'] },
  workflows: { prefs: ['enabledWorkflows'], form: ['enabledWorkflows'] },
  mcp: { prefs: ['mcp'], form: ['mcp'] },
  display: { prefs: ['display'], form: ['display'] }
};

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
    enabledWorkflows: preferences.enabledWorkflows ?? [],
    mcp: preferences.mcp ?? []
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
    enabledWorkflows: form.enabledWorkflows,
    mcp: form.mcp
  };
}

export function SettingsPage({ initialTab, settings, theme, onThemeChange, onSaved, onListing }: SettingsPageProps) {
  const [form, setForm] = useState<SettingsForm | null>(null);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const formRef = useRef<HTMLFormElement>(null);
  const [tab, setTab] = useState<SettingsTab>(
    () => TABS.find((t) => t.id === initialTab)?.id ?? (initialTab ? LEGACY_TABS[initialTab] : undefined) ?? 'models'
  );

  useEffect(() => {
    if (settings && form === null) {
      setForm(toForm(settings.preferences, settings.listings));
    }
  }, [settings, form]);

  function updateModel(value: string) {
    setForm((previous) => (previous ? { ...previous, model: value } : previous));
  }

  function toggleFavorite(ref: ModelRef) {
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
    setForm((previous) =>
      previous ? { ...previous, providers: [...previous.providers, provider] } : previous
    );
  }

  /** Adds a model by hand: an entry with no listed source, saved as an override. */
  function addModel(providerId: string, modelId: string) {
    setForm((previous) => {
      if (!previous) return previous;
      const key = `${providerId}/${modelId}`;
      if (key in previous.models) return previous;
      return { ...previous, models: { ...previous.models, [key]: emptyModelForm } };
    });
  }

  /** Removes a hand-added model: its entry and everything that pointed at it. */
  function removeModel(providerId: string, modelId: string) {
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
    setForm((previous) => (previous ? { ...previous, display } : previous));
  }

  function updateInstructions(instructions: string) {
    setForm((previous) => (previous ? { ...previous, instructions } : previous));
  }

  function updateSearch(search: SearchForm) {
    setForm((previous) => (previous ? { ...previous, search } : previous));
  }

  function updateSandbox(sandbox: SandboxPrefs) {
    setForm((previous) => (previous ? { ...previous, sandbox } : previous));
  }

  function updateMcp(mcp: McpServer[]) {
    setForm((previous) => (previous ? { ...previous, mcp } : previous));
  }

  function updateAutonomy(autonomy: AutonomyPrefs) {
    setForm((previous) => (previous ? { ...previous, autonomy } : previous));
  }

  function updateAlwaysOnSkills(alwaysOnSkills: string[]) {
    setForm((previous) => (previous ? { ...previous, alwaysOnSkills } : previous));
  }

  function updateEnabledWorkflows(enabledWorkflows: string[]) {
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
    setError(null);
    try {
      const response = await saveSettings(buildPreferences(form, settings));
      setForm(toForm(response.preferences, response.listings));
      onSaved(response);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Impossible d’enregistrer les réglages');
    } finally {
      setSaving(false);
    }
  }

  // Dirty is computed, not tracked: the form as it would be stored against the stored preferences
  // passed through the same form, so a value the form only normalises never reads as an edit.
  const baseline = useMemo(
    () => (settings ? buildPreferences(toForm(settings.preferences, settings.listings), settings) : null),
    [settings]
  );
  const current = useMemo(() => (settings && form ? buildPreferences(form, settings) : null), [settings, form]);

  function groupSave(group: SaveGroup): SectionSave {
    const { prefs, form: fields } = SAVE_GROUPS[group];
    return {
      dirty:
        baseline !== null &&
        current !== null &&
        prefs.some((key) => JSON.stringify(current[key]) !== JSON.stringify(baseline[key])),
      onSave: async () => {
        if (!settings || !current) return;
        const patch = Object.fromEntries(prefs.map((key) => [key, current[key]])) as Partial<Preferences>;
        const response = await saveSettings({ ...settings.preferences, ...patch });
        const fresh = toForm(response.preferences, response.listings);
        setForm((previous) =>
          previous ? { ...previous, ...Object.fromEntries(fields.map((key) => [key, fresh[key]])) } : previous
        );
        onSaved(response);
      }
    };
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
    <main className="grow overflow-y-auto px-4 md:px-6">
      {/* The top padding sits inside the scroller, so the jump bar sticks at its very top. */}
      <div className="mx-auto flex max-w-[960px] flex-col gap-6 pt-6 md:pt-8">
        <div>
          <h1 className="text-2xl font-semibold">Réglages</h1>
          <p className="mt-0.5 text-muted">S'applique au prochain tour, sans redémarrage.</p>
        </div>

        <Card className="flex flex-col md:flex-row">
          {/* One form across the tabs: an edit on one tab is kept when switching. */}
          <div className="shrink-0 border-b border-border p-2 md:border-r md:border-b-0">
            <Tabs label="Sections des réglages" tabs={TABS} active={tab} onSelect={setTab} />
          </div>

          <form ref={formRef} onSubmit={handleSubmit} className="flex min-w-0 grow flex-col gap-8 p-5">
            <GroupJumps container={formRef} deps={tab} />
            {error && <p className="m-0 text-xs text-err">{error}</p>}
            {tab === 'models' && (
              <>
                <ModelSection
                  save={groupSave('model')}
                  value={form.model}
                  listings={settings.listings}
                  stored={settings.preferences.selected}
                  defaultModel={settings.defaults.model}
                  onChange={updateModel}
                />
                <ProvidersSection
                  save={groupSave('providers')}
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
                  save={groupSave('modelPrefs')}
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

            {tab === 'behaviour' && (
              <>
                <InstructionsSection save={groupSave('instructions')} value={form.instructions} onChange={updateInstructions} />
                <SearchSection save={groupSave('search')} value={form.search} onChange={updateSearch} />
                <SandboxSection save={groupSave('sandbox')} value={form.sandbox} onChange={updateSandbox} health={settings?.sandboxHealth} />
                <ScannersSection scanners={settings.scanners ?? []} />
                <AutonomySection save={groupSave('autonomy')} value={form.autonomy} onChange={updateAutonomy} />
              </>
            )}

            {tab === 'rules' && <RulesSection />}

            {tab === 'library' && (
              <>
                <SkillsSection
                  save={groupSave('skills')}
                  skills={settings.skills}
                  value={form.alwaysOnSkills}
                  onChange={updateAlwaysOnSkills}
                />
                <WorkflowsSection
                  save={groupSave('workflows')}
                  workflows={settings.optInWorkflows}
                  value={form.enabledWorkflows}
                  onChange={updateEnabledWorkflows}
                />
                <AgentsSection />
                <McpSection
                  save={groupSave('mcp')}
                  value={form.mcp}
                  onChange={updateMcp}
                  savedNames={new Set((settings.preferences.mcp ?? []).map((server) => server.name))}
                />
              </>
            )}

            {tab === 'display' && (
              <>
                <DisplaySection
                  save={groupSave('display')}
                  value={form.display}
                  onChange={updateDisplay}
                  theme={theme}
                  onThemeChange={onThemeChange}
                />
                <PanelPagesSection
                  save={groupSave('display')}
                  value={form.display.panelPages ?? null}
                  onChange={(panelPages) => updateDisplay({ ...form.display, panelPages })}
                />
              </>
            )}

          </form>
        </Card>
      </div>
    </main>
  );
}
