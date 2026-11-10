/**
 * The project's documentation tab: a navigation of its ADRs then its docs
 * (README, docs/) beside the selected one rendered. The server reads them
 * from disk (verified).
 */
import { Fragment, useEffect, useState } from 'react';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import { Badge } from '../ui/Badge';
import { Section } from '../ui/Section';
import { SectionLabel } from '../ui/SectionLabel';
import { DecisionIcon, FileIcon } from '../ui/Icons';
import { markdownComponents } from '../session/markdownComponents';
import { getProjectDocs, type Adr, type DocFile } from '../../lib/api';
import { useT } from '../../i18n';

const REMARK_PLUGINS = [remarkGfm];

type Selected =
  | { kind: 'adr'; file: string }
  | { kind: 'doc'; path: string }
  | null;

type Resolved =
  | { kind: 'doc'; file: DocFile }
  | { kind: 'adr'; adr: Adr }
  | null;

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

function defaultSelection(files: DocFile[], adrs: Adr[]): Selected {
  const readme = files.find((file) => file.path === 'README.md');
  const firstFile = files[0];
  const firstAdr = adrs[0];

  if (readme) return { kind: 'doc', path: readme.path };
  if (firstFile) return { kind: 'doc', path: firstFile.path };
  if (firstAdr) return { kind: 'adr', file: firstAdr.file };
  return null;
}

function find(files: DocFile[], adrs: Adr[], selected: Selected): Resolved {
  if (selected?.kind === 'doc') {
    const file = files.find((candidate) => candidate.path === selected.path);
    return file ? { kind: 'doc', file } : null;
  }
  if (selected?.kind === 'adr') {
    const adr = adrs.find((candidate) => candidate.file === selected.file);
    return adr ? { kind: 'adr', adr } : null;
  }
  return null;
}

function resolveSelection(files: DocFile[], adrs: Adr[], selected: Selected): Resolved {
  return find(files, adrs, selected) ?? find(files, adrs, defaultSelection(files, adrs));
}

function DocNav({
  adrs,
  files,
  activeAdrFile,
  activeDocPath,
  onAdrSelect,
  onDocSelect,
}: {
  adrs: Adr[];
  files: DocFile[];
  activeAdrFile: string | null;
  activeDocPath: string | null;
  onAdrSelect: (file: string) => void;
  onDocSelect: (path: string) => void;
}) {
  const t = useT();

  return (
    <nav
      aria-label={t('documentation')}
      className="flex shrink-0 flex-col md:sticky md:top-6 md:w-64 md:max-h-[calc(100vh-8rem)] md:overflow-y-auto"
    >
      <SectionLabel className="px-2.5 pb-1 pt-0.5">
        {`ADR (${adrs.length})`}
      </SectionLabel>
      {adrs.length === 0 ? (
        <p className="px-2.5 text-xs text-muted">{t('noAdrs')}</p>
      ) : (
        adrs.map((adr) => {
          const active = activeAdrFile === adr.file;
          return (
            <button
              key={adr.file}
              type="button"
              aria-current={active ? 'page' : undefined}
              title={adr.title}
              className={navItemClass(active)}
              onClick={() => onAdrSelect(adr.file)}
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
        })
      )}
      {files.length === 0 ? (
        <p className="px-2.5 text-xs text-muted">{t('noDocFiles')}</p>
      ) : (
        files.map((file, index) => {
          const showHeading = index === 0 || files[index - 1].dir !== file.dir;
          const active = activeDocPath === file.path;

          return (
            <Fragment key={file.path}>
              {showHeading ? (
                <SectionLabel className="px-2.5 pb-1 pt-4">
                  {file.dir === '' ? t('documentation') : file.dir}
                </SectionLabel>
              ) : null}
              <button
                type="button"
                aria-current={active ? 'page' : undefined}
                title={file.path}
                className={navItemClass(active)}
                onClick={() => onDocSelect(file.path)}
              >
                <FileIcon className="h-4 w-4 shrink-0" />
                <span className="grow truncate">{file.name}</span>
              </button>
            </Fragment>
          );
        })
      )}
    </nav>
  );
}

function DocReader({ resolved }: { resolved: Resolved }) {
  const t = useT();

  if (!resolved) {
    return <p className="text-xs text-muted">{t('noDocFiles')}</p>;
  }

  const verifiedBadge = (
    <Badge tone="neutral" title={t('docVerifiedTitle')}>
      {t('verified')}
    </Badge>
  );

  if (resolved.kind === 'doc') {
    return (
      <Section
        Icon={FileIcon}
        title={resolved.file.name}
        flush
        actions={verifiedBadge}
      >
        <div className="px-5 py-4 text-sm">
          <p className="text-xs text-muted">{resolved.file.path}</p>
          <div className="mt-2">
            <ReactMarkdown
              remarkPlugins={REMARK_PLUGINS}
              components={markdownComponents}
            >
              {resolved.file.content}
            </ReactMarkdown>
          </div>
        </div>
      </Section>
    );
  }

  return (
    <Section
      Icon={DecisionIcon}
      title={`${String(resolved.adr.number).padStart(4, '0')} · ${resolved.adr.title}`}
      flush
      actions={
        <div className="flex items-center gap-2">
          {resolved.adr.date ? (
            <span className="text-xs text-muted">{resolved.adr.date}</span>
          ) : null}
          {resolved.adr.status ? (
            <Badge tone={statusTone(resolved.adr.status)}>
              {resolved.adr.status}
            </Badge>
          ) : null}
          {verifiedBadge}
        </div>
      }
    >
      <div className="px-5 py-4 text-sm">
        <p className="text-xs text-muted">{resolved.adr.file}</p>
        <div className="mt-2">
          <ReactMarkdown
            remarkPlugins={REMARK_PLUGINS}
            components={markdownComponents}
          >
            {resolved.adr.body}
          </ReactMarkdown>
        </div>
      </div>
    </Section>
  );
}

export function DocSection({ projectId }: { projectId: string }) {
  const [files, setFiles] = useState<DocFile[]>([]);
  const [adrs, setAdrs] = useState<Adr[]>([]);
  const [selected, setSelected] = useState<Selected>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const t = useT();

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setError(null);

    getProjectDocs(projectId)
      .then((docs) => {
        if (cancelled) return;
        setFiles(docs.files);
        setAdrs(docs.adrs);
        setSelected(defaultSelection(docs.files, docs.adrs));
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

  const resolved = resolveSelection(files, adrs, selected);

  return (
    <section>
      {loading ? (
        <p className="text-muted">{t('loading')}</p>
      ) : error ? (
        <p className="text-err">{error}</p>
      ) : files.length === 0 && adrs.length === 0 ? (
        <p className="text-xs text-muted">{t('noDocFiles')}</p>
      ) : (
        <div className="flex flex-col gap-6 md:flex-row md:items-start">
          <DocNav
            adrs={adrs}
            files={files}
            activeAdrFile={
              resolved && resolved.kind === 'adr' ? resolved.adr.file : null
            }
            activeDocPath={
              resolved && resolved.kind === 'doc' ? resolved.file.path : null
            }
            onAdrSelect={(file) => setSelected({ kind: 'adr', file })}
            onDocSelect={(path) => setSelected({ kind: 'doc', path })}
          />
          <div className="min-w-0 flex-1">
            <DocReader resolved={resolved} />
          </div>
        </div>
      )}
    </section>
  );
}
