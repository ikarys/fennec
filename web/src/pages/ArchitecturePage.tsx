import { useEffect, useState, type ChangeEvent } from 'react';
import { Badge } from '../components/ui/Badge';
import { Card } from '../components/ui/Card';
import { Select } from '../components/ui/Field';
import { SectionLabel } from '../components/ui/SectionLabel';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import { markdownComponents } from '../components/session/markdownComponents';
import { getArchitecture, type Adr, type Architecture, type Project } from '../lib/api';

const REMARK_PLUGINS = [remarkGfm];

function statusTone(status: string): 'neutral' | 'ok' | 'err' {
  if (status === 'accepted') return 'ok';
  if (status === 'rejected' || status === 'deprecated' || status.startsWith('superseded')) return 'err';
  return 'neutral';
}

function AdrRow({ adr, className }: { adr: Adr; className?: string }) {
  return (
    <details className={className}>
      <summary className="flex cursor-pointer list-none items-center gap-3 px-4 py-3 text-sm hover:bg-raised select-none">
        <span className="tabular-nums text-xs text-muted">{String(adr.number).padStart(4, '0')}</span>
        <span className="grow truncate">{adr.title}</span>
        {adr.date && <span className="text-xs text-muted">{adr.date}</span>}
        {adr.status && <Badge tone={statusTone(adr.status)}>{adr.status}</Badge>}
        <Badge tone="neutral" title="Lu depuis le fichier">vérifié</Badge>
      </summary>
      <div className="border-t border-border px-4 py-3 text-sm">
        <p className="text-xs text-muted">{adr.file}</p>
        <ReactMarkdown remarkPlugins={REMARK_PLUGINS} components={markdownComponents}>{adr.body}</ReactMarkdown>
      </div>
    </details>
  );
}

export function ArchitecturePage({ projects, projectId }: { projects: Project[]; projectId: string | null }) {
  const firstProjectId = projects[0]?.id ?? null;
  const [selectedProjectId, setSelectedProjectId] = useState<string | null>(projectId ?? firstProjectId);
  const [data, setData] = useState<Architecture | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    setSelectedProjectId(projectId ?? firstProjectId);
  }, [projectId, firstProjectId]);

  useEffect(() => {
    if (!selectedProjectId) {
      setData(null);
      setError(null);
      setLoading(false);
      return;
    }
    let cancelled = false;
    setLoading(true);
    setError(null);
    setData(null);
    getArchitecture(selectedProjectId)
      .then((architecture) => { if (!cancelled) setData(architecture); })
      .catch((e: unknown) => { if (!cancelled) setError(e instanceof Error ? e.message : String(e)); })
      .finally(() => { if (!cancelled) setLoading(false); });
    return () => { cancelled = true; };
  }, [selectedProjectId]);

  const handleProjectChange = (e: ChangeEvent<HTMLSelectElement>) => {
    const id = e.target.value || null;
    setSelectedProjectId(id);
    if (id) window.location.hash = '#/architecture/' + encodeURIComponent(id);
  };

  return (
    <main className="grow overflow-y-auto px-6 py-8">
      <div className="mx-auto max-w-[720px]">
        <h1 className="text-[1.375rem] font-semibold tracking-[-0.02em]">Architecture</h1>
        <p className="mt-0.5 text-muted">Les décisions qui engagent l'architecture du projet, consignées dans docs/adr/.</p>
        <div className="mt-6 flex items-center gap-3">
          <label htmlFor="arch-project" className="text-xs text-muted">Projet</label>
          <Select id="arch-project" value={selectedProjectId ?? ''} onChange={handleProjectChange} className="w-40">
            {projects.map((project) => <option key={project.id} value={project.id}>{project.name}</option>)}
          </Select>
        </div>

        {projects.length === 0 ? (
          <p className="mt-6 text-muted">Ouvrez d'abord un projet.</p>
        ) : (
          <>
            {loading && <p className="mt-6 text-muted">Chargement…</p>}
            {error && <p className="mt-6 text-err">{error}</p>}
            {!loading && !error && data && (
              <div className="mt-6 flex flex-col gap-6">
                <section>
                  <SectionLabel>ADR ({data.adrs.length})</SectionLabel>
                  {data.adrs.length === 0 ? (
                    <p className="mt-2 text-xs text-muted">Aucun ADR pour l'instant.</p>
                  ) : (
                    <Card className="mt-2">
                      {data.adrs.map((adr, index) => (
                        <AdrRow key={adr.file + ':' + adr.number} adr={adr} className={index > 0 ? 'border-t border-border' : undefined} />
                      ))}
                    </Card>
                  )}
                </section>
              </div>
            )}
          </>
        )}
      </div>
    </main>
  );
}
