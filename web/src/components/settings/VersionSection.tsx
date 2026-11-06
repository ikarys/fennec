import { useEffect, useState } from 'react';
import { Section } from '../ui/Section';
import { Badge } from '../ui/Badge';
import { Button, buttonClass } from '../ui/Button';
import { ExternalIcon, RefreshIcon, SpinnerIcon } from '../Icons';
import { useT } from '../../i18n';
import { getVersion, getUpdate, type UpdateCheck, type VersionInfo } from '../../lib/api';

type CheckResult =
  | { kind: 'checked'; check: UpdateCheck }
  | { kind: 'error'; message: string };

export function VersionSection() {
  const t = useT();
  const [info, setInfo] = useState<VersionInfo | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [checking, setChecking] = useState(false);
  const [check, setCheck] = useState<CheckResult | null>(null);

  useEffect(() => {
    let cancelled = false;
    getVersion()
      .then((versionInfo) => {
        if (!cancelled) setInfo(versionInfo);
      })
      .catch((error) => {
        if (!cancelled) setLoadError(error instanceof Error ? error.message : String(error));
      });
    return () => {
      cancelled = true;
    };
  }, []);

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
      title={t('versionTitle')}
      description={t('versionDescription')}
      actions={
        // A local build updates from its checkout: the releases do not concern it.
        !info.build && <Button
          variant="ghost"
          size="sm"
          iconOnly
          title={t('versionCheckTitle')}
          aria-label={t('versionCheckTitle')}
          disabled={checking}
          onClick={onCheck}
        >
          {checking ? <SpinnerIcon className="h-4 w-4 animate-spin" /> : <RefreshIcon className="h-4 w-4" />}
        </Button>
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
          className={buttonClass({ variant: 'ghost', size: 'sm', iconOnly: true })}
          title={t('versionChangelog')}
          aria-label={t('versionChangelog')}
        >
          <ExternalIcon className="h-4 w-4" />
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
