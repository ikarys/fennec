/**
 * The project's decisions tab: its ADRs (docs/adr/) in a side navigation
 * beside the selected one rendered. The server reads them from disk (verified).
 */
import { useEffect, useState } from 'react';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import { Badge } from '../ui/Badge';
import { Section } from '../ui/Section';
import { SectionLabel } from '../ui/SectionLabel';
import { DecisionIcon } from '../ui/Icons';
import { markdownComponents } from '../session/markdownComponents';
import { getProjectAdrs, type Adr } from '../../lib/api';
import { useT } from '../../i18n';

const REMARK_PLUGINS = [remarkGfm];

function statusTone(status: string) {
  if (status === 'accepted') return 'ok' as const;
  if (
    status === 'rejected' ||
    status === 'deprecated' ||
    status.startsWith('superseded')
  ) {
    return 'err' as const;
  }
  return 'neutral' as const;
}

function navItemClass(active: boolean) {
  return (
    'relative mb-px flex h-[34px] w-full items-center gap-2.5 rounded-md pl-2 pr-2.5 text-left text-sm transition-colors ' +
    (active
      ? 'bg-accent-soft font-semibold text-accent before:absolute before:-left-0.5 before:top-[7px] before:bottom-[7px] before:w-[2.5px] before:rounded-sm before:bg-accent'
      : 'text-muted hover:bg-raised hover:text-text')
  );
}

function AdrNav({
  adrs,
  activeFile,
  onSelect,
}: {
  adrs: Adr[];
  activeFile: string;
  onSelect: (file: string) => void;
}) {
  const t = useT();

  return (
    <nav
      aria-label={t('projectPageDecisions')}
      className="flex shrink-0 flex-col md:sticky md:top-6 md:w-64 md:max-h-[calc(100vh-8rem)] md:overflow-y-auto"
    >
      <SectionLabel className="px-2.5 pb-1 pt-0.5">
        {`ADR (${adrs.length})`}
      </SectionLabel>
      {adrs.map((adr) => {
        const active = activeFile === adr.file;
        return (
          <button
            key={adr.file}
            type="button"
            aria-current={active ? 'page' : undefined}
            title={adr.title}
            className={navItemClass(active)}
            onClick={() => onSelect(adr.file)}
          >
            <span className="tabular-nums text-xs">
              {String(adr.number).padStart(4, '0')}
            </span>
            <span className="grow truncate">{adr.title}</span>
            {adr.status ? (
              <Badge tone={statusTone(adr.status)}>{adr.status}</Badge>
            ) : null}
          </button>
        );
      })}
    </nav>
  );
}

function AdrReader({ adr }: { adr: Adr }) {
  const t = useT();

  const verifiedBadge = (
    <Badge tone="neutral" title={t('docVerifiedTitle')}>
      {t('verified')}
    </Badge>
  );

  return (
    <Section
      Icon={DecisionIcon}
      title={`${String(adr.number).padStart(4, '0')} · ${adr.title}`}
      flush
      actions={
        <div className="flex items-center gap-2">
          {adr.date ? (
            <span className="text-xs text-muted">{adr.date}</span>
          ) : null}
          {adr.status ? (
            <Badge tone={statusTone(adr.status)}>{adr.status}</Badge>
          ) : null}
          {verifiedBadge}
        </div>
      }
    >
      <div className="px-5 py-4 text-sm">
        <p className="text-xs text-muted">{adr.file}</p>
        <div className="mt-2">
          <ReactMarkdown
            remarkPlugins={REMARK_PLUGINS}
            components={markdownComponents}
          >
            {adr.body}
          </ReactMarkdown>
        </div>
      </div>
    </Section>
  );
}

export function DecisionsSection({ projectId }: { projectId: string }) {
  const [adrs, setAdrs] = useState<Adr[]>([]);
  const [selected, setSelected] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const t = useT();

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setError(null);

    getProjectAdrs(projectId)
      .then((data) => {
        if (cancelled) return;
        setAdrs(data.adrs);
        setSelected(data.adrs[0]?.file ?? null);
        setLoading(false);
      })
      .catch((err: unknown) => {
        if (cancelled) return;
        setError(err instanceof Error ? err.message : t('unknownError'));
        setLoading(false);
      });

    return () => {
      cancelled = true;
    };
  }, [projectId]);

  const shownAdr = adrs.find((adr) => adr.file === selected) ?? adrs[0];

  return (
    <section>
      {loading ? (
        <p className="text-muted">{t('loading')}</p>
      ) : error ? (
        <p className="text-err">{error}</p>
      ) : !shownAdr ? (
        <p className="text-xs text-muted">{t('noAdrs')}</p>
      ) : (
        <div className="flex flex-col gap-6 md:flex-row md:items-start">
          <AdrNav
            adrs={adrs}
            activeFile={shownAdr.file}
            onSelect={setSelected}
          />
          <div className="min-w-0 flex-1">
            <AdrReader adr={shownAdr} />
          </div>
        </div>
      )}
    </section>
  );
}
