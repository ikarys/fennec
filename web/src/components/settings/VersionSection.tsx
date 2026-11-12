// Version information and update check.
import { useState } from 'react';
import { Section } from '../ui/Section';
import { Badge } from '../ui/Badge';
import { Button } from '../ui/Button';
import { ExternalIcon, InfoIcon, RefreshIcon, SpinnerIcon } from '../ui/Icons';
import { useT } from '../../i18n';
import { getVersion, getUpdate, type UpdateCheck } from '../../lib/api';
import { useLoaded } from '../../hooks/useLoaded';
import { useInstallUpdate } from '../../hooks/useInstallUpdate';

type CheckResult =
  | { kind: 'checked'; check: UpdateCheck }
  | { kind: 'error'; message: string };

export function VersionSection() {
  const t = useT();
  const [checking, setChecking] = useState(false);
  const [check, setCheck] = useState<CheckResult | null>(null);
  const { installing, install } = useInstallUpdate();

  const { data: info, error: loadError } = useLoaded(getVersion, []);

  const onCheck = () => {
    if (checking) return;
    setChecking(true);
    getUpdate(true)
      .then((updateCheck) => {
        setCheck({ kind: 'checked', check: updateCheck });
      })
      .catch((error) => {
        setCheck({ kind: 'error', message: error instanceof Error ? error.message : String(error) });
      })
      .finally(() => {
        setChecking(false);
      });
  };

  if (!info) {
    return loadError ? <p className="text-xs text-danger">{loadError}</p> : null;
  }

  return (
    <Section
      Icon={InfoIcon}
      title={t('versionTitle')}
      description={t('versionDescription')}
      actions={
        // A local build updates from its checkout: the releases do not concern it.
        !info.build && (
          <Button variant="primary" size="sm" disabled={checking} onClick={onCheck}>
            {checking ? <SpinnerIcon className="h-3.5 w-3.5 animate-spin" /> : <RefreshIcon className="h-3.5 w-3.5" />}
            {t('versionCheckTitle')}
          </Button>
        )
      }
    >
      {info.build && <p className="mb-2 text-xs text-muted">{t('versionLocalHint')}</p>}
      <div className="flex flex-wrap items-center gap-2">
        <span className="font-mono text-sm">{info.version}</span>
        {info.build && (
          <Badge tone="accent" title={info.build}>
            {t('versionBuild', { build: info.build })}
          </Badge>
        )}
        <a
          href={info.changelogUrl}
          target="_blank"
          rel="noreferrer"
          className="ml-auto inline-flex items-center gap-1 text-xs text-accent-strong hover:underline"
        >
          {t('aboutAllReleases')}
          <ExternalIcon className="h-3 w-3" />
        </a>
      </div>
      {check && check.kind === 'checked' && check.check.newer && (
        <div className="mt-2 flex flex-wrap items-center gap-2 text-xs">
          <Badge tone="attention">{t('versionNewer', { latest: check.check.latest })}</Badge>
          <a
            href={check.check.releaseUrl}
            target="_blank"
            rel="noreferrer"
            className="text-xs text-accent-strong underline"
          >
            {t('versionReleaseLink')}
          </a>
          <Button variant="primary" size="sm" disabled={installing} onClick={() => void install(check.check)}>
            {installing ? t('updateInstalling') : t('versionInstall', { latest: check.check.latest })}
          </Button>
        </div>
      )}
      {check && check.kind === 'checked' && !check.check.newer && (
        <div className="mt-2 text-xs">
          <Badge tone="ok">{t('versionUpToDate', { latest: check.check.latest })}</Badge>
        </div>
      )}
      {check && check.kind === 'error' && <p className="mt-2 text-xs text-danger">{check.message}</p>}
    </Section>
  );
}
