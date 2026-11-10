// The Settings page's form model: the stored preferences read into editable strings, and the
// strings written back as preferences. Pure, so the round trip is tested without a browser.
import type {
  ApiKey,
  AutonomyPrefs,
  DelegationPrefs,
  IntegrationPrefs,
  DisplayPrefs,
  McpServer,
  ModelPrefs,
  ModelRef,
  Preferences,
  Provider,
  ProviderListing,
  ReasoningEffort,
  SandboxPrefs,
  SettingsResponse,
} from './api';
import type { ProviderForm } from '../components/settings/ProvidersSection';
import type { SearchForm } from '../components/settings/SearchSection';

/** A model's editable per-field overrides, kept as strings in the form. */
export interface ModelForm {
  maxTokens: string;
  contextWindow: string;
  compactionThreshold: string;
  reasoningEffort: string;
  priceInput: string;
  priceCached: string;
  priceOutput: string;
}

type ListedModel = ProviderListing['models'][number];

/** The model ids of `provider` edited in the form but absent from its listing: added by hand. */
export function formOnlyModels(
  values: Record<string, ModelForm>,
  providerId: string,
  listing: ProviderListing | undefined
): string[] {
  const listedIds = new Set(listing?.models.map((model) => model.id) ?? []);
  const prefix = `${providerId}/`;
  return Object.keys(values)
    .filter((key) => key.startsWith(prefix))
    .map((key) => key.slice(prefix.length))
    .filter((modelId) => !listedIds.has(modelId))
    .sort();
}

/** A token count as a pill shows it: `240000` → `240k`, `1234` → `1.2k`; anything else unchanged. */
function compactTokens(text: string): string {
  const value = Number(text);
  if (!Number.isFinite(value) || value < 1000) return text;
  return `${Number((value / 1000).toFixed(1))}k`;
}

/** Compact spec pills for a model row. */
export function modelSpecs(
  value: ModelForm,
  model: ListedModel | null,
  autoEffort: string
): string[] {
  const specs: string[] = [];

  if (model === null) {
    if (value.contextWindow !== '') {
      specs.push(`ctx ${compactTokens(value.contextWindow)}`);
    }
  } else {
    const window =
      value.contextWindow !== '' ? value.contextWindow : String(model.resolved.contextWindow.value);
    specs.push(`ctx ${compactTokens(window)}`);

    const max =
      value.maxTokens !== '' ? value.maxTokens : String(model.resolved.maxTokens.value);
    specs.push(`max ${compactTokens(max)}`);

    const effort =
      value.reasoningEffort !== ''
        ? value.reasoningEffort
        : (model.resolved.reasoningEffort.value ?? autoEffort);
    specs.push(`effort ${effort}`);
  }

  if (value.priceInput !== '' || value.priceOutput !== '') {
    specs.push(`$${value.priceInput || '0'}/$${value.priceOutput || '0'}`);
  }

  return specs;
}

/** A fraction as the percentage the form shows (`0.85` → `85`). */
export function percentFromFraction(fraction: number): string {
  return String(Number((fraction * 100).toFixed(2)));
}

/** Mirrors `pricing::billed_by_default`: a local model's price is a simulation. */
export function billedByDefault(providerId: string): boolean {
  return providerId !== 'local';
}

export interface SettingsForm {
  model: string;
  favorites: ModelRef[];
  providers: ProviderForm[];
  models: Record<string, ModelForm>;
  billing: Record<string, boolean>;
  display: DisplayPrefs;
  instructions: string;
  search: SearchForm;
  sandbox: SandboxPrefs;
  autonomy: AutonomyPrefs;
  delegation: DelegationPrefs;
  integration: IntegrationPrefs;
  alwaysOnSkills: string[];
  enabledWorkflows: string[];
  mcp: McpServer[];
  shortcuts: Record<string, string>;
}



export type SaveGroup =
  | 'model'
  | 'providers'
  | 'modelPrefs'
  | 'instructions'
  | 'search'
  | 'sandbox'
  | 'autonomy'
  | 'delegation'
  | 'integration'
  | 'skills'
  | 'workflows'
  | 'mcp'
  | 'display'
  | 'shortcuts';

/**
 * What each group saves: its preference keys (stored on top of the others, so the other groups'
 * unsaved edits stay pending) and the form fields it reloads from the answer (a typed API key
 * comes back sealed). Providers and per-model parameters share `providers`.
 */
export const SAVE_GROUPS: Record<SaveGroup, { prefs: (keyof Preferences)[]; form: (keyof SettingsForm)[] }> = {
  model: { prefs: ['selected'], form: ['model'] },
  providers: { prefs: ['providers'], form: ['providers', 'models'] },
  modelPrefs: { prefs: ['providers', 'favorites', 'billing'], form: ['providers', 'models', 'favorites', 'billing'] },
  instructions: { prefs: ['globalInstructions'], form: ['instructions'] },
  search: { prefs: ['search'], form: ['search'] },
  sandbox: { prefs: ['sandbox'], form: ['sandbox'] },
  autonomy: { prefs: ['autonomy'], form: ['autonomy'] },
  delegation: { prefs: ['delegation'], form: ['delegation'] },
  integration: { prefs: ['integration'], form: ['integration'] },
  skills: { prefs: ['alwaysOnSkills'], form: ['alwaysOnSkills'] },
  workflows: { prefs: ['enabledWorkflows'], form: ['enabledWorkflows'] },
  mcp: { prefs: ['mcp'], form: ['mcp'] },
  display: { prefs: ['display'], form: ['display'] },
  shortcuts: { prefs: ['shortcuts'], form: ['shortcuts'] }
};

export const emptyModelForm: ModelForm = {
  maxTokens: '',
  contextWindow: '',
  compactionThreshold: '',
  reasoningEffort: '',
  priceInput: '',
  priceCached: '',
  priceOutput: ''
};

export function localProviderForm(): ProviderForm {
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

export function providerToForm(provider: Provider): ProviderForm {
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
export function providerFormToApiKey(provider: ProviderForm): ApiKey | null {
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

export function toForm(preferences: Preferences, listings: ProviderListing[]): SettingsForm {
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
    billing: preferences.billing ?? {},
    display: preferences.display,
    instructions: preferences.globalInstructions ?? '',
    search: {
      url: preferences.search?.searxngUrl ?? '',
      apiKey: preferences.search?.searxngApiKey ?? '',
      webFetch: preferences.search?.webFetch ?? true
    },
    sandbox: preferences.sandbox ?? { enabled: true, network: false },
    autonomy: preferences.autonomy ?? { maxMinutesPerIdea: 120, maxTokensPerIdea: 2_000_000 },
    delegation: preferences.delegation ?? { enabled: true, maxPerTurn: 5 },
    integration: preferences.integration ?? { modelTrailers: true },
    alwaysOnSkills: preferences.alwaysOnSkills ?? [],
    enabledWorkflows: preferences.enabledWorkflows ?? [],
    mcp: preferences.mcp ?? [],
    shortcuts: preferences.shortcuts ?? {}
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

export function buildPreferences(form: SettingsForm, settings: SettingsResponse): Preferences {
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
    billing: form.billing,
    display: form.display,
    globalInstructions: form.instructions.trim() === '' ? null : form.instructions,
    search: {
      searxngUrl: searxngUrl === '' ? null : searxngUrl,
      searxngApiKey: searxngApiKey === '' ? null : searxngApiKey,
      webFetch: form.search.webFetch
    },
    sandbox: form.sandbox,
    autonomy: form.autonomy,
    delegation: form.delegation,
    integration: form.integration,
    alwaysOnSkills: form.alwaysOnSkills,
    enabledWorkflows: form.enabledWorkflows,
    mcp: form.mcp,
    shortcuts: form.shortcuts
  };
}
