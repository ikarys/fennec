import type { ScannerStatus } from '../../lib/api';
import { CopyButton } from '../session/CopyButton';
import { Badge } from '../ui/Badge';
import { ListCard } from '../ui/ListCard';
import { Section } from '../ui/Section';
import { useT } from '../../i18n';

interface ScannersSectionProps {
  scanners: ScannerStatus[];
}

export function ScannersSection({ scanners }: ScannersSectionProps) {
  const t = useT();
  const installed = scanners.filter((scanner) => scanner.path).length;
  const total = scanners.length;

  return (
    <Section title={t('securityTitle')}>
      <p className="text-xs text-muted">{t('scannersHint')}</p>
      <ListCard title={t('scannersListTitle')} count={t('scannersInstalledCount', { installed, total })}>
        <ul className="divide-y divide-border">
          {scanners.map((scanner) => (
            <li key={scanner.id} className="flex flex-col gap-1 px-4 py-3">
              <div className="flex items-center gap-2">
                <a
                  href={scanner.url}
                  target="_blank"
                  rel="noreferrer"
                  className="text-sm font-medium text-accent hover:underline"
                >
                  {scanner.name}
                </a>
                {scanner.path ? (
                  <Badge tone="ok" title={scanner.path}>
                    {t('scannerInstalled')}
                  </Badge>
                ) : (
                  <Badge tone="neutral">{t('scannerAbsent')}</Badge>
                )}
              </div>
              <p className="text-xs text-muted">{scanner.covers}</p>
              {!scanner.path && (
                <div className="flex items-center gap-2">
                  <code
                    className="min-w-0 grow truncate rounded bg-surface px-2 py-1 font-mono text-xs"
                    title={scanner.install}
                  >
                    {scanner.install}
                  </code>
                  <CopyButton text={scanner.install} />
                </div>
              )}
            </li>
          ))}
        </ul>
      </ListCard>
    </Section>
  );
}
