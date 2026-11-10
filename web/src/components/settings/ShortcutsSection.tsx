// The Settings page's keyboard shortcut editor: capture a combo per action and reset overrides.
import { useState } from 'react';
import type { KeyboardEvent as ReactKeyboardEvent } from 'react';
import { Section, type SectionSave } from '../ui/Section';
import { SettingRow } from '../ui/SettingRow';
import { Button } from '../ui/Button';
import { KeyboardIcon, RefreshIcon } from '../ui/Icons';
import { useT } from '../../i18n';
import type { MsgKey } from '../../i18n/messages';
import { comboLabel, comboOf, type ShortcutAction, type ShortcutBinding } from '../../lib/shortcuts';

interface ShortcutsSectionProps {
  /** Every binding as stored (the defaults and the catalog order come from here). */
  bindings: ShortcutBinding[];
  /** The overrides being edited. */
  value: Record<string, string>;
  onChange: (value: Record<string, string>) => void;
  save?: SectionSave;
}

const LABELS: Record<ShortcutAction, MsgKey> = {
  newChat: 'shortcutNewChat',
  agentPlan: 'shortcutAgentPlan',
  agentBuilder: 'shortcutAgentBuilder',
  agentAsk: 'shortcutAgentAsk',
  newSession: 'shortcutNewSession',
  goHome: 'shortcutGoHome',
  goProjects: 'shortcutGoProjects',
  goChats: 'shortcutGoChats',
  goTasks: 'shortcutGoTasks',
  goWorkflows: 'shortcutGoWorkflows',
  goInsight: 'shortcutGoInsight',
  goSettings: 'shortcutGoSettings',
  nextTab: 'shortcutNextTab',
  prevTab: 'shortcutPrevTab',
  closeTab: 'shortcutCloseTab',
  palette: 'shortcutPalette',
  split: 'shortcutSplit'
};

export function ShortcutsSection({ bindings, value, onChange, save }: ShortcutsSectionProps) {
  const t = useT();
  const [capturing, setCapturing] = useState<ShortcutAction | null>(null);

  const hasOverrides = Object.keys(value).length > 0;

  const removeOverride = (action: ShortcutAction) => {
    const next = { ...value };
    delete next[action];
    onChange(next);
  };

  const stopCapturing = () => setCapturing(null);

  return (
    <Section
      Icon={KeyboardIcon}
      flush
      title={t('shortcutsTitle')}
      save={save}
      actions={
        <Button
          variant="ghost"
          size="sm"
          iconOnly
          aria-label={t('shortcutsResetAll')}
          title={t('shortcutsResetAll')}
          disabled={!hasOverrides}
          onClick={() => onChange({})}
        >
          <RefreshIcon className="h-4 w-4" />
        </Button>
      }
    >
      {bindings.map((binding) => {
        const current = Object.hasOwn(value, binding.action)
          ? value[binding.action] ?? binding.default
          : binding.default;
        const isCapturing = capturing === binding.action;
        const display = isCapturing ? t('shortcutsPress') : current ? comboLabel(current) : t('shortcutsNone');

        return (
          <SettingRow key={binding.action} label={t(LABELS[binding.action])}>
            <div className="flex items-center gap-2">
              <Button
                variant="secondary"
                size="sm"
                className="min-w-[8rem] justify-center font-mono"
                aria-label={t('shortcutsChange')}
                onClick={() => setCapturing(binding.action)}
                onKeyDown={(event: ReactKeyboardEvent<HTMLButtonElement>) => {
                  if (!isCapturing) return;
                  event.preventDefault();
                  event.stopPropagation();
                  if (event.key === 'Escape') {
                    setCapturing(null);
                    return;
                  }
                  if (event.key === 'Backspace' || event.key === 'Delete') {
                    const next = { ...value };
                    next[binding.action] = '';
                    onChange(next);
                    setCapturing(null);
                    return;
                  }
                  const combo = comboOf(event.nativeEvent);
                  if (!combo) return;
                  const next = { ...value };
                  if (combo === binding.default) {
                    delete next[binding.action];
                  } else {
                    next[binding.action] = combo;
                  }
                  onChange(next);
                  setCapturing(null);
                }}
                onBlur={stopCapturing}
              >
                {display}
              </Button>
              {Object.hasOwn(value, binding.action) && (
                <Button
                  variant="ghost"
                  size="sm"
                  iconOnly
                  aria-label={t('shortcutsReset')}
                  title={t('shortcutsReset')}
                  onClick={() => removeOverride(binding.action)}
                >
                  <RefreshIcon className="h-4 w-4" />
                </Button>
              )}
            </div>
          </SettingRow>
        );
      })}
    </Section>
  );
}
