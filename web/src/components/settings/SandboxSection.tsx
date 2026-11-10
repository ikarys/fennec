// Sandbox settings section.
import type { SandboxHealth, SandboxPrefs } from '../../lib/api';
import { CopyButton } from '../session/CopyButton';
import { Badge } from '../ui/Badge';
import { Section, type SectionSave } from '../ui/Section';
import { SettingRow } from '../ui/SettingRow';
import { TerminalIcon } from '../ui/Icons';
import { Switch } from '../ui/Switch';
import { useT } from '../../i18n';

const FIX =
  "echo 'kernel.apparmor_restrict_unprivileged_userns=0' | sudo tee /etc/sysctl.d/60-bwrap-userns.conf && sudo sysctl --system";

interface SandboxSectionProps {
  value: SandboxPrefs;
  onChange: (value: SandboxPrefs) => void;
  health?: SandboxHealth;
  /** The group's own save: its dot and its "Save". */
  save?: SectionSave;
}

export function SandboxSection({ value, onChange, health, save }: SandboxSectionProps) {
  const t = useT();
  return (
    <Section Icon={TerminalIcon} flush title={t('sandboxTitle')} save={save}>
      {value.enabled && health && health.status !== 'ready' && (
        <div className="border-b border-border p-4">
          <div className="flex flex-col gap-1 rounded-md border border-err p-3">
            <Badge tone="err" className="self-start">
              {t('bwrapUnavailable')}
            </Badge>
            {health.status === 'missing' && (
              <p className="text-xs text-muted">{t('bwrapMissingHint')}</p>
            )}
            {health.status === 'blocked' && (
              <>
                <p className="text-xs text-muted">{t('bwrapBlockedHint')}</p>
                {health.detail && <code className="font-mono text-xs text-err">{health.detail}</code>}
                <div className="flex items-center gap-2">
                  <code
                    className="min-w-0 grow truncate rounded-md bg-surface px-2 py-1 font-mono text-xs"
                    title={FIX}
                  >
                    {FIX}
                  </code>
                  <CopyButton text={FIX} />
                </div>
              </>
            )}
          </div>
        </div>
      )}
      <SettingRow label={t('sandboxEnabledLabel')} hint={t('sandboxEnabledHint')}>
        <Switch
          checked={value.enabled}
          onChange={(checked) => onChange({ ...value, enabled: checked })}
          label={t('sandboxEnabledLabel')}
        />
      </SettingRow>
      <SettingRow label={t('sandboxNetworkLabel')} hint={t('sandboxNetworkHint')}>
        <Switch
          checked={value.network}
          onChange={(checked) => onChange({ ...value, network: checked })}
          label={t('sandboxNetworkLabel')}
          disabled={!value.enabled}
        />
      </SettingRow>
    </Section>
  );
}
