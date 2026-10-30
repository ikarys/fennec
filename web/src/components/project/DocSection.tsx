/**
 * The project's documentation tab: its markdown files (AGENTS.md, README,
 * docs/) rendered, and its ADRs. The server reads them from disk (verified).
 */
import { useEffect, useState } from 'react';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import { Badge } from '../ui/Badge';
import { Card } from '../ui/Card';
import { SectionLabel } from '../ui/SectionLabel';
import { Select } from '../ui/Field';
import { markdownComponents } from '../session/markdownComponents';
import { getProjectDocs, type Adr, type DocFile } from '../../lib/api';

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

function AdrRow({ adr, first }: { adr: Adr; first: boolean }) {
  return (
    <details className={first ? undefined : 'border-t border-border'}>
      <summary className="flex cursor-pointer list-none items-center gap-3 px-4 py-3 text-sm hover:bg-raised select-none">
        <span className="tabular-nums text-xs text-muted">
          {String(adr.number).padStart(4, '0')}
        </span>
        <span className="grow truncate">{adr.title}</span>
        {adr.date ? <span className="text-xs text-muted">{adr.date}</span> : null}
        {adr.status ? (
          <Badge tone={statusTone(adr.status)}>{adr.status}</Badge>
        ) : null}
        <Badge tone="neutral" title="Lu depuis le fichier">
          vérifié
        </Badge>
      </summary>
      <div className="border-t border-border px-4 py-3 text-sm">
        <p className="text-xs text-muted">{adr.file}</p>
        <ReactMarkdown
          remarkPlugins={REMARK_PLUGINS}
          components={markdownComponents}
        >
          {adr.body}
        </ReactMarkdown>
      </div>
    </details>
  );
}

export function DocSection({ projectId }: { projectId: string }) {
  const [files, setFiles] = useState<DocFile[]>([]);
  const [adrs, setAdrs] = useState<Adr[]>([]);
  const [selected, setSelected] = useState('');
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setError(null);

    getProjectDocs(projectId)
      .then((docs) => {
        if (cancelled) return;
        setFiles(docs.files);
        setAdrs(docs.adrs);
        setSelected(docs.files[0]?.path ?? '');
        setLoading(false);
      })
      .catch((err: unknown) => {
        if (cancelled) return;
        setError(err instanceof Error ? err.message : 'Erreur inconnue');
        setLoading(false);
      });

    return () => {
      cancelled = true;
    };
  }, [projectId]);

  const file =
    files.find((candidate) => candidate.path === selected) ?? files[0] ?? null;

  return (
    <section>
      {loading ? (
        <p className="text-muted">Chargement…</p>
      ) : error ? (
        <p className="text-err">{error}</p>
      ) : (
        <>
          {file ? (
            <>
              <div className="flex items-center gap-2">
                <SectionLabel>Documentation</SectionLabel>
                {files.length > 1 ? (
                  <Select
                    aria-label="Fichier de doc"
                    value={file.path}
                    onChange={(event) => setSelected(event.target.value)}
                    className="min-w-0"
                  >
                    {files.map((candidate) => (
                      <option key={candidate.path} value={candidate.path}>
                        {candidate.name}
                      </option>
                    ))}
                  </Select>
                ) : null}
              </div>
              <Card className="mt-3">
                <div className="px-4 py-3 text-sm">
                  <p className="text-xs text-muted">{file.path}</p>
                  <div className="mt-2">
                    <ReactMarkdown
                      remarkPlugins={REMARK_PLUGINS}
                      components={markdownComponents}
                    >
                      {file.content}
                    </ReactMarkdown>
                  </div>
                </div>
              </Card>
            </>
          ) : (
            <p className="text-xs text-muted">
              Aucun fichier de doc (AGENTS.md, README, docs/).
            </p>
          )}

          <div className="mt-6">
            <SectionLabel>ADR ({adrs.length})</SectionLabel>
            {adrs.length === 0 ? (
              <p className="mt-3 text-xs text-muted">
                Aucun ADR dans docs/adr/.
              </p>
            ) : (
              <Card className="mt-3">
                {adrs.map((adr, index) => (
                  <AdrRow key={adr.file} adr={adr} first={index === 0} />
                ))}
              </Card>
            )}
          </div>
        </>
      )}
    </section>
  );
}
