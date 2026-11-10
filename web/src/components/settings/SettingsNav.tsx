// The settings side navigation: the sub-pages under their group headings, each one an address.
import { useT } from '../../i18n';
import type { MsgKey } from '../../i18n/messages';
import {
  EyeIcon,
  CpuIcon,
  FileIcon,
  LayersIcon,
  WandIcon,
  WorkflowsIcon,
  MoonIcon,
  HardDriveIcon,
  DecisionIcon,
  SearchIcon,
  PuzzleIcon,
  ServerIcon,
  GlobeIcon,
  InfoIcon,
  KeyboardIcon
} from '../ui/Icons';
import { SectionLabel } from '../ui/SectionLabel';
import { SETTINGS_GROUPS, type SettingsPageId, type SettingsGroupId } from '../../lib/settingsPages';

export const SETTINGS_PAGE_META: Record<
  SettingsPageId,
  { Icon: typeof PuzzleIcon; labelKey: MsgKey; hintKey: MsgKey }
> = {
  display: { Icon: EyeIcon, labelKey: 'settingsPageDisplay', hintKey: 'settingsPageDisplayHint' },
  shortcuts: { Icon: KeyboardIcon, labelKey: 'settingsPageShortcuts', hintKey: 'settingsPageShortcutsHint' },
  providers: { Icon: CpuIcon, labelKey: 'settingsPageProviders', hintKey: 'settingsPageProvidersHint' },
  instructions: {
    Icon: FileIcon,
    labelKey: 'settingsPageInstructions',
    hintKey: 'settingsPageInstructionsHint'
  },
  agents: { Icon: LayersIcon, labelKey: 'settingsPageAgents', hintKey: 'settingsPageAgentsHint' },
  skills: { Icon: WandIcon, labelKey: 'settingsPageSkills', hintKey: 'settingsPageSkillsHint' },
  workflows: {
    Icon: WorkflowsIcon,
    labelKey: 'settingsPageWorkflows',
    hintKey: 'settingsPageWorkflowsHint'
  },
  autonomy: { Icon: MoonIcon, labelKey: 'settingsPageAutonomy', hintKey: 'settingsPageAutonomyHint' },
  sandbox: { Icon: HardDriveIcon, labelKey: 'settingsPageSandbox', hintKey: 'settingsPageSandboxHint' },
  rules: { Icon: DecisionIcon, labelKey: 'settingsPageRules', hintKey: 'settingsPageRulesHint' },
  scanners: { Icon: SearchIcon, labelKey: 'settingsPageScanners', hintKey: 'settingsPageScannersHint' },
  plugins: { Icon: PuzzleIcon, labelKey: 'settingsPagePlugins', hintKey: 'settingsPagePluginsHint' },
  mcp: { Icon: ServerIcon, labelKey: 'settingsPageMcp', hintKey: 'settingsPageMcpHint' },
  search: { Icon: GlobeIcon, labelKey: 'settingsPageSearch', hintKey: 'settingsPageSearchHint' },
  about: { Icon: InfoIcon, labelKey: 'settingsPageAbout', hintKey: 'settingsPageAboutHint' }
};

const GROUP_HEADINGS: Record<SettingsGroupId, MsgKey> = {
  general: 'settingsGroupGeneral',
  models: 'settingsGroupModels',
  agents: 'settingsGroupAgents',
  security: 'settingsGroupSecurity',
  extensions: 'settingsGroupExtensions',
  about: 'settingsGroupAbout'
};

export function SettingsNav({ active }: { active: SettingsPageId }) {
  const t = useT();

  return (
    <nav aria-label={t('settingsSectionsLabel')} className="flex shrink-0 flex-col md:sticky md:top-6 md:w-56">
      {SETTINGS_GROUPS.map((group, index) => (
        <div key={group.id}>
          <SectionLabel className={`px-2.5 pb-1 ${index === 0 ? 'pt-0.5' : 'pt-4'}`}>
            {t(GROUP_HEADINGS[group.id])}
          </SectionLabel>
          {group.pages.map((id) => {
            const meta = SETTINGS_PAGE_META[id];
            const Icon = meta.Icon;
            const isActive = id === active;
            return (
              <a
                key={id}
                href={`#/settings/${id}`}
                aria-current={isActive ? 'page' : undefined}
                className={
                  'relative mb-px flex h-[34px] items-center gap-2.5 rounded-md pl-2 pr-2.5 text-sm transition-colors ' +
                  (isActive
                    ? 'bg-accent-soft font-semibold text-accent before:absolute before:-left-0.5 before:top-[7px] before:bottom-[7px] before:w-[2.5px] before:rounded-sm before:bg-accent'
                    : 'text-muted hover:bg-raised hover:text-text')
                }
              >
                <Icon className="h-4 w-4 shrink-0" />
                {t(meta.labelKey)}
              </a>
            );
          })}
        </div>
      ))}
    </nav>
  );
}
