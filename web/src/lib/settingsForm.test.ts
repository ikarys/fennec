import { describe, it, expect } from 'vitest';
import {
  buildPreferences,
  emptyModelForm,
  modelSpecs,
  percentFromFraction,
  providerFormToApiKey,
  providerToForm,
  toForm,
} from './settingsForm';
import type { SettingsForm } from './settingsForm';
import type {
  ApiKey,
  ModelPrefs,
  Preferences,
  Provider,
  ProviderListing,
  ReasoningEffort,
  ResolvedValue,
  SettingsResponse,
} from './api';
import type { ProviderForm } from '../components/settings/ProvidersSection';
import type { ModelForm } from './settingsForm';

function resolved<T>(value: T): ResolvedValue<T> {
  return { value, source: 'default', wire: false };
}

function listing(provider: string, modelIds: string[]): ProviderListing {
  return {
    provider,
    name: provider,
    baseUrl: `https://${provider}.example.com`,
    models: modelIds.map((id) => ({
      id,
      contextWindow: null,
      resolved: {
        maxTokens: resolved(0),
        contextWindow: resolved(0),
        compactAt: resolved(0),
        reasoningEffort: resolved<ReasoningEffort | null>(null),
      },
    })),
    error: null,
  };
}

function preferences(overrides: Partial<Preferences> = {}): Preferences {
  return {
    selected: null,
    providers: [],
    display: {
      reasoningOpen: true,
      toolResultsOpen: true,
      fontSize: 14,
    },
    ...overrides,
  };
}

function provider(
  id: string,
  apiKey: ApiKey | null = null,
  models: Record<string, ModelPrefs> = {},
): Provider {
  return {
    id,
    name: id,
    baseUrl: `https://${id}.example.com`,
    apiKey,
    models,
  };
}

function providerForm(overrides: Partial<ProviderForm> = {}): ProviderForm {
  return {
    id: 'p',
    name: 'p',
    baseUrl: 'https://p.example.com',
    keyKind: 'none',
    envVar: '',
    newKey: '',
    sealed: null,
    ...overrides,
  };
}

function modelForm(overrides: Partial<ModelForm> = {}): ModelForm {
  return { ...emptyModelForm, ...overrides };
}

function form(
  preferencesValue: Preferences,
  listings: ProviderListing[],
  overrides: Partial<SettingsForm> = {},
): SettingsForm {
  return { ...toForm(preferencesValue, listings), ...overrides };
}

function response(preferencesValue: Preferences, listings: ProviderListing[]): SettingsResponse {
  return {
    preferences: preferencesValue,
    shortcuts: [],
    listings,
    defaults: {
      provider: 'local',
      model: 'local/model',
      maxTokens: 0,
      contextWindow: 0,
      compactAt: 0,
      reasoningEffort: null,
    },
    skills: [],
    scanners: [],
    optInWorkflows: [],
    mcpTools: {},
    sandboxHealth: {
      status: 'ready',
      detail: null,
    },
  };
}

describe('percent from fraction', () => {
  it('formats 0.85 as 85', () => {
    expect(percentFromFraction(0.85)).toBe('85');
  });

  it('rounds 0.123456 to 12.35', () => {
    expect(percentFromFraction(0.123456)).toBe('12.35');
  });

  it('formats 1 as 100', () => {
    expect(percentFromFraction(1)).toBe('100');
  });
});

describe('provider to form', () => {
  it('maps a missing api key to none', () => {
    const providerValue = provider('p');

    expect(providerToForm(providerValue)).toEqual({
      id: 'p',
      name: 'p',
      baseUrl: 'https://p.example.com',
      keyKind: 'none',
      envVar: '',
      newKey: '',
      sealed: null,
    });
  });

  it('maps an env key to env', () => {
    const providerValue = provider('p', { env: 'X' });
    const formValue = providerToForm(providerValue);

    expect(formValue.keyKind).toBe('env');
    expect(formValue.envVar).toBe('X');
    expect(formValue.newKey).toBe('');
    expect(formValue.sealed).toBeNull();
  });

  it('maps a sealed key to stored', () => {
    const providerValue = provider('p', { sealed: 's' });
    const formValue = providerToForm(providerValue);

    expect(formValue.keyKind).toBe('stored');
    expect(formValue.sealed).toBe('s');
    expect(formValue.newKey).toBe('');
  });

  it('maps a plain key to stored with a new key', () => {
    const providerValue = provider('p', { plain: 'p' });
    const formValue = providerToForm(providerValue);

    expect(formValue.keyKind).toBe('stored');
    expect(formValue.newKey).toBe('p');
    expect(formValue.sealed).toBeNull();
  });
});

describe('provider form to api key', () => {
  it('returns null for an env key with a blank env var', () => {
    expect(providerFormToApiKey(providerForm({ keyKind: 'env', envVar: '  ' }))).toBeNull();
  });

  it('returns an env key for a non blank env var', () => {
    expect(providerFormToApiKey(providerForm({ keyKind: 'env', envVar: 'KEY' }))).toEqual({
      env: 'KEY',
    });
  });

  it('returns a plain key for a trimmed new key', () => {
    expect(providerFormToApiKey(providerForm({ keyKind: 'stored', newKey: ' k ' }))).toEqual({
      plain: 'k',
    });
  });

  it('returns a sealed key when no new key is present', () => {
    expect(providerFormToApiKey(providerForm({ keyKind: 'stored', sealed: 's' }))).toEqual({
      sealed: 's',
    });
  });

  it('returns null for a stored key with no new key or sealed value', () => {
    expect(providerFormToApiKey(providerForm({ keyKind: 'stored' }))).toBeNull();
  });

  it('returns null for a none key', () => {
    expect(providerFormToApiKey(providerForm({ keyKind: 'none' }))).toBeNull();
  });
});

describe('to form', () => {
  it('starts with a local provider when none is stored', () => {
    const prefs = preferences({ providers: [provider('p')] });
    const formValue = toForm(prefs, []);

    expect(formValue.providers[0].id).toBe('local');
  });

  it('moves a stored local provider to the front', () => {
    const prefs = preferences({ providers: [provider('other'), provider('local')] });
    const formValue = toForm(prefs, []);

    expect(formValue.providers.map((item) => item.id)).toEqual(['local', 'other']);
  });

  it('reads stored model prefs as form strings', () => {
    const listings = [listing('p', ['model'])];
    const prefs = preferences({
      providers: [
        provider('p', null, {
          model: {
            maxTokens: 10,
            compactionThreshold: 0.5,
            price: { inputPerM: 1, cachedPerM: 2, outputPerM: 3 },
          },
        }),
      ],
    });
    const formValue = toForm(prefs, listings);

    expect(formValue.models['p/model']).toEqual({
      maxTokens: '10',
      contextWindow: '',
      compactionThreshold: '50',
      reasoningEffort: '',
      priceInput: '1',
      priceCached: '2',
      priceOutput: '3',
    });
  });

  it('reads a listed model without prefs as empty strings', () => {
    const listings = [listing('p', ['model'])];
    const formValue = toForm(preferences(), listings);

    expect(formValue.models['p/model']).toEqual(emptyModelForm);
  });

  it('applies defaults for missing optional preferences', () => {
    const formValue = toForm(preferences(), []);

    expect(formValue.search).toEqual({ url: '', apiKey: '', webFetch: true });
    expect(formValue.sandbox).toEqual({ enabled: true, network: false });
    expect(formValue.autonomy).toEqual({ maxMinutesPerIdea: 120, maxTokensPerIdea: 2_000_000 });
    expect(formValue.favorites).toEqual([]);
    expect(formValue.alwaysOnSkills).toEqual([]);
    expect(formValue.enabledWorkflows).toEqual([]);
    expect(formValue.mcp).toEqual([]);
  });

  it('reads a selected model as provider/model', () => {
    const prefs = preferences({ selected: { provider: 'p', model: 'm' } });

    expect(toForm(prefs, []).model).toBe('p/m');
  });

  it('reads a null selected model as an empty string', () => {
    expect(toForm(preferences(), []).model).toBe('');
  });
});

describe('build preferences', () => {
  const listings = [listing('p', ['listed'])];

  it('returns a null selected model for an empty model', () => {
    const prefs = preferences();
    const formValue = form(prefs, listings, { model: '' });

    expect(buildPreferences(formValue, response(prefs, listings)).selected).toBeNull();
  });

  it('returns a selected model for provider/model', () => {
    const prefs = preferences();
    const formValue = form(prefs, listings, { model: 'p/m' });

    expect(buildPreferences(formValue, response(prefs, listings)).selected).toEqual({
      provider: 'p',
      model: 'm',
    });
  });

  it('returns a null selected model when the model has no slash', () => {
    const prefs = preferences();
    const formValue = form(prefs, listings, { model: 'nomodel' });

    expect(buildPreferences(formValue, response(prefs, listings)).selected).toBeNull();
  });

  it('drops a listed model whose fields are all empty', () => {
    const prefs = preferences({ providers: [provider('p')] });
    const formValue = form(prefs, listings);
    const built = buildPreferences(formValue, response(prefs, listings));

    expect(built.providers.find((item) => item.id === 'p')?.models).toEqual({});
  });

  it('keeps a hand added model with empty fields', () => {
    const prefs = preferences({ providers: [provider('p')] });
    const formValue = form(prefs, listings, { models: { 'p/hand': modelForm() } });
    const built = buildPreferences(formValue, response(prefs, listings));

    expect(built.providers.find((item) => item.id === 'p')?.models).toEqual({ hand: {} });
  });

  it('parses a compaction threshold percentage as a fraction', () => {
    const prefs = preferences({ providers: [provider('p')] });
    const formValue = form(prefs, listings, {
      models: { 'p/listed': modelForm({ compactionThreshold: '50' }) },
    });
    const built = buildPreferences(formValue, response(prefs, listings));

    expect(built.providers.find((item) => item.id === 'p')?.models.listed?.compactionThreshold).toBe(
      0.5,
    );
  });

  it('fills missing price fields with zero when a price is present', () => {
    const prefs = preferences({ providers: [provider('p')] });
    const formValue = form(prefs, listings, {
      models: { 'p/listed': modelForm({ priceInput: '3' }) },
    });
    const built = buildPreferences(formValue, response(prefs, listings));

    expect(built.providers.find((item) => item.id === 'p')?.models.listed?.price).toEqual({
      inputPerM: 3,
      cachedPerM: 0,
      outputPerM: 0,
    });
  });

  it('stores blank instructions as null', () => {
    const prefs = preferences();
    const formValue = form(prefs, listings, { instructions: '   ' });

    expect(buildPreferences(formValue, response(prefs, listings)).globalInstructions).toBeNull();
  });

  it('stores an empty search url as null', () => {
    const prefs = preferences();
    const formValue = form(prefs, listings, { search: { url: '', apiKey: '', webFetch: true } });

    expect(buildPreferences(formValue, response(prefs, listings)).search?.searxngUrl).toBeNull();
  });

  it('trims a non empty search url', () => {
    const prefs = preferences();
    const formValue = form(prefs, listings, {
      search: { url: ' https://searxng.example.com ', apiKey: '', webFetch: true },
    });

    expect(buildPreferences(formValue, response(prefs, listings)).search?.searxngUrl).toBe(
      'https://searxng.example.com',
    );
  });
});

describe('round trip', () => {
  it('keeps the form unchanged after building and reloading preferences', () => {
    const listings = [listing('p', ['listed'])];
    const prefs = preferences({
      selected: { provider: 'p', model: 'listed' },
      providers: [
        provider('p', { env: 'KEY' }, {
          listed: {
            maxTokens: 10,
            compactionThreshold: 0.5,
            price: { inputPerM: 1, cachedPerM: 2, outputPerM: 3 },
          },
          hand: {},
        }),
      ],
    });

    const initial = toForm(prefs, listings);
    const rebuilt = buildPreferences(initial, response(prefs, listings));

    expect(toForm(rebuilt, listings)).toEqual(initial);
  });

  it('round-trips a billing map', () => {
    const listings = [listing('p', ['listed'])];
    const prefs = preferences({
      billing: { 'p/listed': false, 'local/m': true },
      providers: [provider('p')],
    });

    const initial = toForm(prefs, listings);
    const rebuilt = buildPreferences(initial, response(prefs, listings));

    expect(rebuilt.billing).toEqual({ 'p/listed': false, 'local/m': true });
    expect(toForm(rebuilt, listings).billing).toEqual(initial.billing);
  });
});

describe('modelSpecs', () => {

  const listed = {
    id: 'm',
    resolved: {
      maxTokens: resolved(500),
      contextWindow: resolved(1000),
      compactAt: resolved(0.8),
      reasoningEffort: resolved('high')
    }
  } as ProviderListing['models'][number];

  const listedWithoutEffort = {
    id: 'm',
    resolved: {
      maxTokens: resolved(500),
      contextWindow: resolved(1000),
      compactAt: resolved(0.8),
      reasoningEffort: resolved(null)
    }
  } as ProviderListing['models'][number];

  it('uses listed-model overrides when present', () => {
    const value = {
      ...emptyModelForm,
      contextWindow: '1234',
      maxTokens: '567',
      reasoningEffort: 'low'
    };

    expect(modelSpecs(value, listed, 'auto effort')).toEqual([
      'ctx 1.2k',
      'max 567',
      'effort low'
    ]);
  });

  it('falls back to resolved values for a listed model', () => {
    expect(modelSpecs(emptyModelForm, listed, 'auto effort')).toEqual([
      'ctx 1k',
      'max 500',
      'effort high'
    ]);
  });

  it('falls back to the auto effort for a listed model without resolved effort', () => {
    expect(modelSpecs(emptyModelForm, listedWithoutEffort, 'auto effort')).toEqual([
      'ctx 1k',
      'max 500',
      'effort auto effort'
    ]);
  });

  it('returns only context for a hand-added model when set', () => {
    const value = { ...emptyModelForm, contextWindow: '4321' };

    expect(modelSpecs(value, null, 'auto effort')).toEqual(['ctx 4.3k']);
  });

  it('returns no specs for a hand-added model without context', () => {
    expect(modelSpecs(emptyModelForm, null, 'auto effort')).toEqual([]);
  });

  it('appends prices when either price is set', () => {
    const value = {
      ...emptyModelForm,
      priceInput: '0.3',
      priceOutput: '0.6'
    };

    expect(modelSpecs(value, listed, 'auto effort')).toEqual([
      'ctx 1k',
      'max 500',
      'effort high',
      '$0.3/$0.6'
    ]);

    const handAdded = {
      ...emptyModelForm,
      contextWindow: '4321',
      priceInput: '0.3'
    };

    expect(modelSpecs(handAdded, null, 'auto effort')).toEqual([
      'ctx 4.3k',
      '$0.3/$0'
    ]);
  });
});