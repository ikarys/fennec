import { useEffect, useState, type FormEvent } from 'react';
import type {
  DisplayPrefs,
  ModelPrefs,
  Preferences,
  ReasoningEffort,
  SandboxPrefs,
  SettingsModel,
  SettingsResponse
} from '../lib/api';
import { saveSettings } from '../lib/api';
import { ModelSection } from '../components/settings/ModelSection';
import { ModelPrefsSection, percentFromFraction, type ModelForm } from '../components/settings/ModelPrefsSection';
import { DisplaySection } from '../components/settings/DisplaySection';
import { InstructionsSection } from '../components/settings/InstructionsSection';
import { SearchSection, type SearchForm } from '../components/settings/SearchSection';
import { SandboxSection } from '../components/settings/SandboxSection';

interface SettingsPageProps {
  settings: SettingsResponse | null;
  onSaved: (next: SettingsResponse) => void;
}

interface SettingsForm {
  model: string;
  models: Record<string, ModelForm>;
  display: DisplayPrefs;
  instructions: string;
  search: SearchForm;
  sandbox: SandboxPrefs;
}

type SettingsTab = 'models' | 'instructions' | 'search' | 'sandbox' | 'display';

const TABS: { id: SettingsTab; label: string }[] = [
  { id: 'models', label: 'Modèles' },
  { id: 'instructions', label: 'Instructions' },
  { id: 'search', label: 'Recherche web' },
  { id: 'sandbox', label: 'Bac à sable' },
  { id: 'display', label: 'Affichage' }
];

const emptyModelForm: ModelForm = {
  maxTokens: '',
  contextWindow: '',
  compactionThreshold: '',
  reasoningEffort: ''
};

function toForm(preferences: Preferences, models: SettingsModel[]): SettingsForm {
  const modelForms: Record<string, ModelForm> = {};
  for (const model of models) {
    const prefs = preferences.models[model.id] ?? {};
    modelForms[model.id] = {
      maxTokens: prefs.maxTokens == null ? '' : String(prefs.maxTokens),
      contextWindow: prefs.contextWindow == null ? '' : String(prefs.contextWindow),
      compactionThreshold: prefs.compactionThreshold == null ? '' : percentFromFraction(prefs.compactionThreshold),
      reasoningEffort: prefs.reasoningEffort ?? ''
    };
  }
  return {
    model: preferences.model ?? '',
    models: modelForms,
    display: preferences.display,
    instructions: preferences.globalInstructions ?? '',
    search: {
      url: preferences.search?.searxngUrl ?? '',
      apiKey: preferences.search?.searxngApiKey ?? ''
    },
    sandbox: preferences.sandbox ?? { enabled: true, network: false }
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
  const listedIds = new Set(settings.models.map((model) => model.id));
  const models: Record<string, ModelPrefs> = {};

  // Keep overrides for models that are no longer listed; the page does not edit them.
  for (const [id, prefs] of Object.entries(settings.preferences.models)) {
    if (!listedIds.has(id)) {
      models[id] = prefs;
    }
  }

  for (const model of settings.models) {
    const entry = form.models[model.id] ?? emptyModelForm;
    const maxTokens = parseNumber(entry.maxTokens);
    const contextWindow = parseNumber(entry.contextWindow);
    const compactionThreshold = parsePercent(entry.compactionThreshold);
    if (maxTokens == null && contextWindow == null && compactionThreshold == null && entry.reasoningEffort === '') continue;
    const prefs: ModelPrefs = {};
    if (maxTokens != null) prefs.maxTokens = maxTokens;
    if (contextWindow != null) prefs.contextWindow = contextWindow;
    if (compactionThreshold != null) prefs.compactionThreshold = compactionThreshold;
    if (entry.reasoningEffort !== '') prefs.reasoningEffort = entry.reasoningEffort as ReasoningEffort;
    models[model.id] = prefs;
  }

  const searxngUrl = form.search.url.trim();
  const searxngApiKey = form.search.apiKey.trim();

  return {
    model: form.model === '' ? null : form.model,
    models,
    display: form.display,
    globalInstructions: form.instructions.trim() === '' ? null : form.instructions,
    search: {
      searxngUrl: searxngUrl === '' ? null : searxngUrl,
      searxngApiKey: searxngApiKey === '' ? null : searxngApiKey
    },
    sandbox: form.sandbox
  };
}

export function SettingsPage({ settings, onSaved }: SettingsPageProps) {
  const [form, setForm] = useState<SettingsForm | null>(null);
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [tab, setTab] = useState<SettingsTab>('models');

  useEffect(() => {
    if (settings && form === null) {
      setForm(toForm(settings.preferences, settings.models));
    }
  }, [settings, form]);

  function markDirty() {
    setSaved(false);
    setError(null);
  }

  function updateModel(value: string) {
    markDirty();
    setForm((previous) => (previous ? { ...previous, model: value } : previous));
  }

  function updateModelField(modelId: string, field: keyof ModelForm, value: string) {
    markDirty();
    setForm((previous) => {
      if (!previous) return previous;
      const current = previous.models[modelId] ?? emptyModelForm;
      return {
        ...previous,
        models: {
          ...previous.models,
          [modelId]: { ...current, [field]: value }
        }
      };
    });
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

  async function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!settings || !form || saving) return;
    setSaving(true);
    setSaved(false);
    setError(null);
    try {
      const response = await saveSettings(buildPreferences(form, settings));
      setForm(toForm(response.preferences, response.models));
      setSaved(true);
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
      <main className="grow overflow-y-auto px-6 py-8">
        <div className="mx-auto max-w-[720px]">
          <h1 className="text-[22px] font-semibold tracking-[-0.02em]">Réglages</h1>
          <p className="mt-0.5 text-muted">S'applique au prochain tour, sans redémarrage.</p>
          <div className="mt-6 text-sm text-muted">Chargement…</div>
        </div>
      </main>
    );
  }

  return (
    <main className="grow overflow-y-auto px-6 py-8">
      <div className="mx-auto flex max-w-[960px] flex-col gap-6">
        <div>
          <h1 className="text-[22px] font-semibold tracking-[-0.02em]">Réglages</h1>
          <p className="mt-0.5 text-muted">S'applique au prochain tour, sans redémarrage.</p>
        </div>

        <div className="flex gap-6">
          {/* One form across the tabs: an edit on one tab is kept when switching. */}
          <nav aria-label="Sections des réglages" className="flex w-44 shrink-0 flex-col gap-0.5">
            {TABS.map(({ id, label }) => (
              <button
                key={id}
                type="button"
                onClick={() => setTab(id)}
                aria-current={tab === id ? 'page' : undefined}
                className={`rounded-lg px-3 py-1.5 text-left text-[13px] transition-colors ${
                  tab === id ? 'bg-raised text-text font-medium' : 'text-muted hover:bg-raised hover:text-text'
                }`}
              >
                {label}
              </button>
            ))}
          </nav>

          <form onSubmit={handleSubmit} className="flex min-w-0 grow flex-col gap-6">
            {tab === 'models' && (
              <>
                <ModelSection
                  value={form.model}
                  models={settings.models}
                  storedModel={settings.preferences.model}
                  defaultModel={settings.defaults.model}
                  onChange={updateModel}
                />
                <ModelPrefsSection
                  models={settings.models}
                  values={form.models}
                  defaults={settings.defaults}
                  onChange={updateModelField}
                />
              </>
            )}

            {tab === 'instructions' && (
              <InstructionsSection value={form.instructions} onChange={updateInstructions} />
            )}

            {tab === 'search' && (
              <SearchSection value={form.search} onChange={updateSearch} />
            )}

            {tab === 'sandbox' && (
              <SandboxSection value={form.sandbox} onChange={updateSandbox} />
            )}

            {tab === 'display' && (
              <DisplaySection value={form.display} onChange={updateDisplay} />
            )}

            <div className="flex items-center gap-3">
              <button
                type="submit"
                disabled={saving}
                className="flex min-h-9 items-center gap-2 rounded-lg bg-accent px-4 text-[13px] font-medium text-on-accent disabled:opacity-60"
              >
                {saving ? 'Enregistrement…' : 'Enregistrer'}
              </button>
              {saved && <span className="text-[13px] text-ok">Enregistré</span>}
              {error && <span className="text-[13px] text-err">{error}</span>}
            </div>
          </form>
        </div>
      </div>
    </main>
  );
}
