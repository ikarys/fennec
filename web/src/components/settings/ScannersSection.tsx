import type { ScannerStatus } from '../../lib/api';
import { CopyButton } from '../session/CopyButton';
import { Badge } from '../ui/Badge';
import { Section } from '../ui/Section';

interface ScannersSectionProps {
  scanners: ScannerStatus[];
}

export function ScannersSection({ scanners }: ScannersSectionProps) {
  return (
    <Section title="Sécurité">
      <p className="text-xs text-muted">
        L&apos;agent « Audit sécurité » lance les scanners qu&apos;il trouve sur ta machine, puis lit
        le code. fennec ne les installe pas : copie la commande et lance-la dans ton terminal, puis
        recharge cette page. Ceux qui interrogent une base en ligne (cargo audit, OSV-Scanner,
        Semgrep) ont besoin du réseau dans le bac à sable.
      </p>
      <ul className="flex flex-col gap-2">
        {scanners.map((scanner) => (
          <li key={scanner.id} className="flex flex-col gap-1 rounded-md border border-border p-3">
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
                  installé
                </Badge>
              ) : (
                <Badge tone="neutral">absent</Badge>
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
    </Section>
  );
}
