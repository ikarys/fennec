// The settings sub-pages, their groups in the side navigation, and the old addresses that still lead to them.
export type SettingsPageId =
  | 'display'
  | 'shortcuts'
  | 'providers'
  | 'instructions'
  | 'agents'
  | 'skills'
  | 'workflows'
  | 'autonomy'
  | 'sandbox'
  | 'rules'
  | 'scanners'
  | 'plugins'
  | 'mcp'
  | 'search'
  | 'about';

export type SettingsGroupId = 'models' | 'security' | 'extensions' | 'display';

export const SETTINGS_GROUPS: readonly { id: SettingsGroupId; pages: readonly SettingsPageId[] }[] = [
  { id: 'models', pages: ['providers'] },
  { id: 'security', pages: ['sandbox', 'rules', 'scanners', 'autonomy'] },
  { id: 'extensions', pages: ['agents', 'skills', 'workflows', 'plugins', 'mcp', 'search', 'instructions'] },
  { id: 'display', pages: ['display', 'shortcuts', 'about'] }
];

const LEGACY_PAGES: Record<string, SettingsPageId> = {
  models: 'providers',
  behaviour: 'instructions',
  security: 'sandbox',
  extensions: 'plugins',
  library: 'skills'
};

function isSettingsPageId(value: string): value is SettingsPageId {
  return SETTINGS_GROUPS.some((group) => group.pages.some((page) => page === value));
}

export function resolveSettingsPage(id: string | null | undefined): SettingsPageId {
  if (id === null || id === undefined) return 'providers';
  if (isSettingsPageId(id)) return id;
  if (Object.hasOwn(LEGACY_PAGES, id)) {
    const mapped = LEGACY_PAGES[id];
    if (mapped !== undefined) return mapped;
  }
  return 'providers';
}
