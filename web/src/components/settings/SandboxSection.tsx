import type { SandboxHealth, SandboxPrefs } from '../../lib/api';
import { CopyButton } from '../session/CopyButton';
import { Badge } from '../ui/Badge';
import { Section, type SectionSave } from '../ui/Section';
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
    <Section title={t('sandboxTitle')} save={save}>
      {value.enabled && health && health.status !== 'ready' && (
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
                  className="min-w-0 grow truncate rounded bg-surface px-2 py-1 font-mono text-xs"
                  title={FIX}
                >
                  {FIX}
                </code>
                <CopyButton text={FIX} />
              </div>
            </>
          )}
        </div>
      )}
      <div className="flex flex-col gap-1">
        <label className="flex items-center gap-2 text-sm text-text">
          <input
            type="checkbox"
            checked={value.enabled}
            onChange={(event) => onChange({ ...value, enabled: event.target.checked })}
            className="h-4 w-4"
          />
          {t('sandboxEnabledLabel')}
        </label>
        <p className="text-xs text-muted">{t('sandboxEnabledHint')}</p>
      </div>
      <div className="flex flex-col gap-1">
        <label className={`flex items-center gap-2 text-sm ${value.enabled ? 'text-text' : 'text-muted'}`}>
          <input
            type="checkbox"
            checked={value.network}
            disabled={!value.enabled}
            onChange={(event) => onChange({ ...value, network: event.target.checked })}
            className="h-4 w-4 disabled:opacity-60"
          />
          {t('sandboxNetworkLabel')}
        </label>
        <p className="text-xs text-muted">{t('sandboxNetworkHint')}</p>
      </div>
    </Section>
  );
}
