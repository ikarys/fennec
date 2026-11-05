import { useEffect, useMemo, useRef, useState, type FormEvent } from 'react';
import type {
  AutonomyPrefs,
  DisplayPrefs,
  McpServer,
  ModelRef,
  Preferences,
  ProviderListing,
  SandboxPrefs,
  SettingsResponse
} from '../lib/api';
import { listProvider, saveSettings } from '../lib/api';
import {
  SAVE_GROUPS,
  buildPreferences,
  emptyModelForm,
  providerFormToApiKey,
  toForm,
  type SaveGroup,
  type SettingsForm,
} from '../lib/settingsForm';
import { type ThemeId } from '../lib/theme';
import { Tabs } from '../components/ui/Tabs';
import { GroupJumps } from '../components/settings/GroupJumps';
import { Card } from '../components/ui/Card';
import type { SectionSave } from '../components/ui/Section';
import { ModelSection } from '../components/settings/ModelSection';
import { ProvidersSection, type ProviderForm } from '../components/settings/ProvidersSection';
import { ModelPrefsSection, type ModelForm } from '../components/settings/ModelPrefsSection';
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
import { PluginsSection } from '../components/settings/PluginsSection';
import { SkillsSection } from '../components/settings/SkillsSection';
import { WorkflowsSection } from '../components/settings/WorkflowsSection';
import { useT } from '../i18n';
import type { MsgKey } from '../i18n/messages';

interface SettingsPageProps {
  /** The tab named in the address (`#/settings/security`), when it names one. */
  initialTab?: string | null;
  settings: SettingsResponse | null;
  theme: ThemeId;
  onThemeChange: (theme: ThemeId) => void;
  onSaved: (next: SettingsResponse) => void;
  onListing: (listing: ProviderListing) => void;
}

type SettingsTab = 'models' | 'behaviour' | 'security' | 'plugins' | 'library' | 'display';

// Six groups, not twelve tabs: what tunes the model, how an agent behaves, what it is allowed to touch
// (sandbox, scanners, the human's rules), the human's plugins, the catalogues of content (skills, workflows, agents, MCP) and the display.
const TABS: { id: SettingsTab; labelKey: MsgKey }[] = [
  { id: 'models', labelKey: 'settingsTabModels' },
  { id: 'behaviour', labelKey: 'settingsTabBehaviour' },
  { id: 'security', labelKey: 'settingsTabSecurity' },
  { id: 'plugins', labelKey: 'settingsTabPlugins' },
  { id: 'library', labelKey: 'settingsTabLibrary' },
  { id: 'display', labelKey: 'settingsTabDisplay' }
];

/** The old per-section addresses (`#/settings/skills`) still open the group that holds the section. */
const LEGACY_TABS: Record<string, SettingsTab> = {
  instructions: 'behaviour',
  search: 'behaviour',
  sandbox: 'security',
  security: 'security',
  rules: 'security',
  autonomy: 'behaviour',
  skills: 'library',
  workflows: 'library',
  agents: 'library',
  mcp: 'library'
};

export function SettingsPage({ initialTab, settings, theme, onThemeChange, onSaved, onListing }: SettingsPageProps) {
  const t = useT();
  const [form, setForm] = useState<SettingsForm | null>(null);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const formRef = useRef<HTMLFormElement>(null);
  const [tab, setTab] = useState<SettingsTab>(
    () => TABS.find((item) => item.id === initialTab)?.id ?? (initialTab ? LEGACY_TABS[initialTab] : undefined) ?? 'models'
  );
  const tabs = TABS.map((item) => ({ id: item.id, label: t(item.labelKey) }));

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
      setError(err instanceof Error ? err.message : t('settingsSaveError'));
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
      setError(err instanceof Error ? err.message : t('settingsSaveError'));
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
          <h1 className="text-2xl font-semibold">{t('settingsTitle')}</h1>
          <p className="mt-0.5 text-muted">{t('settingsSubtle')}</p>
          <div className="mt-6 text-sm text-muted">{t('loading')}</div>
        </div>
      </main>
    );
  }

  return (
    <main className="grow overflow-y-auto px-4 md:px-6">
      {/* The top padding sits inside the scroller, so the jump bar sticks at its very top. */}
      <div className="mx-auto flex max-w-[960px] flex-col gap-6 pt-6 md:pt-8">
        <div>
          <h1 className="text-2xl font-semibold">{t('settingsTitle')}</h1>
          <p className="mt-0.5 text-muted">{t('settingsSubtle')}</p>
        </div>

        <Card className="flex flex-col md:flex-row">
          {/* One form across the tabs: an edit on one tab is kept when switching. */}
          <div className="shrink-0 border-b border-border p-2 md:border-r md:border-b-0">
            <Tabs label={t('settingsSectionsLabel')} tabs={tabs} active={tab} onSelect={setTab} />
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
                <AutonomySection save={groupSave('autonomy')} value={form.autonomy} onChange={updateAutonomy} />
              </>
            )}

            {tab === 'security' && (
              <>
                <SandboxSection save={groupSave('sandbox')} value={form.sandbox} onChange={updateSandbox} health={settings?.sandboxHealth} />
                <ScannersSection scanners={settings.scanners ?? []} />
                <RulesSection />
              </>
            )}

            {tab === 'plugins' && <PluginsSection />}

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
