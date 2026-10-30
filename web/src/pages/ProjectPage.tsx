import { useEffect, useState, type ReactNode } from 'react';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import { Badge } from '../components/ui/Badge';
import { Button } from '../components/ui/Button';
import { Card } from '../components/ui/Card';
import { SectionLabel } from '../components/ui/SectionLabel';
import { markdownComponents } from '../components/session/markdownComponents';
import { ProjectMap } from '../components/project/ProjectMap';
import { getProjectOverview, writeMapFile, type Adr, type Project, type ProjectOverview } from '../lib/api';

const REMARK_PLUGINS = [remarkGfm];

function statusTone(status: string) {
  if (status === 'accepted') return 'ok' as const;
  if (status === 'rejected' || status === 'deprecated' || status.startsWith('superseded')) return 'err' as const;
  return 'neutral' as const;
}

function AdrRow({ adr, first }: { adr: Adr; first: boolean }) {
  return (
    <details className={first ? undefined : 'border-t border-border'}>
      <summary className="flex cursor-pointer list-none items-center gap-3 px-4 py-3 text-sm hover:bg-raised select-none">
        <span className="tabular-nums text-xs text-muted">{String(adr.number).padStart(4, '0')}</span>
        <span className="grow truncate">{adr.title}</span>
        {adr.date ? <span className="text-xs text-muted">{adr.date}</span> : null}
        {adr.status ? <Badge tone={statusTone(adr.status)}>{adr.status}</Badge> : null}
        <Badge tone="neutral" title="Lu depuis le fichier">vérifié</Badge>
      </summary>
      <div className="border-t border-border px-4 py-3 text-sm">
        <p className="text-xs text-muted">{adr.file}</p>
        <ReactMarkdown remarkPlugins={REMARK_PLUGINS} components={markdownComponents}>{adr.body}</ReactMarkdown>
      </div>
    </details>
  );
}

export default function ProjectPage({ projects, projectId }: { projects: Project[]; projectId: string }) {
  const project = projects.find((p) => p.id === projectId) ?? null;
  const [data, setData] = useState<ProjectOverview | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [writing, setWriting] = useState(false);
  const [writeError, setWriteError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setError(null);
    setWriteError(null);
    setData(null);
    getProjectOverview(projectId)
      .then((overview) => {
        if (!cancelled) {
          setData(overview);
          setLoading(false);
        }
      })
      .catch((err: unknown) => {
        if (!cancelled) {
          setError(err instanceof Error ? err.message : 'Erreur inconnue');
          setLoading(false);
        }
      });
    return () => {
      cancelled = true;
    };
  }, [projectId]);

  const handleWrite = async () => {
    setWriting(true);
    setWriteError(null);
    try {
      const result = await writeMapFile(projectId);
      setData((prev) => (prev ? { ...prev, mapFile: result.mapFile } : prev));
    } catch (err) {
      setWriteError(err instanceof Error ? err.message : 'Erreur inconnue');
    } finally {
      setWriting(false);
    }
  };

  const shell = (children: ReactNode) => (
    <main className="grow overflow-y-auto px-6 py-8">
      <div className="mx-auto max-w-[1100px]">{children}</div>
    </main>
  );

  if (!project) {
    return shell(<p className="text-muted">Projet introuvable.</p>);
  }

  const header = (
    <>
      <h1 className="text-[1.375rem] font-semibold tracking-[-0.02em]">{project.name}</h1>
      <p className="mt-0.5 font-mono text-xs text-muted">
        {project.workdir} · ⎇ {project.branch ?? 'sans branche'}
      </p>
    </>
  );

  if (loading) {
    return shell(<>{header}<p className="mt-6 text-muted">Chargement…</p></>);
  }

  if (error) {
    return shell(<>{header}<p className="mt-6 text-err">{error}</p></>);
  }

  if (!data) {
    return shell(<>{header}<p className="mt-6 text-muted">Chargement…</p></>);
  }

  const mapFile = data.mapFile;

  return shell(
    <>
      {header}
      <div className="mt-6 flex flex-col gap-8">
        <section>
          <div className="flex items-center gap-2">
            <SectionLabel>Carte des modules</SectionLabel>
            <Badge tone="neutral" title="Extraite des imports du code, sans modèle">vérifié</Badge>
            <div className="grow" />
            {mapFile === 'current' ? (
              <Badge tone="ok">docs/map.md à jour</Badge>
            ) : mapFile === 'stale' ? (
              <Badge tone="accent">docs/map.md périmé</Badge>
            ) : (
              <Badge tone="neutral">docs/map.md absent</Badge>
            )}
            {mapFile !== 'current' ? (
              <Button variant="secondary" size="sm" disabled={writing} onClick={handleWrite}>
                {writing ? 'Écriture…' : mapFile === 'missing' ? 'Écrire docs/map.md' : 'Mettre à jour docs/map.md'}
              </Button>
            ) : null}
          </div>
          {writeError ? <p className="mt-1 text-xs text-err">{writeError}</p> : null}
          <p className="mt-2 text-xs text-muted">
            Une flèche A → B : du code de A importe du code de B (nombre de fichiers). docs/map.md, commité, donne cette carte aux agents et aux développeurs qui arrivent sur le projet.
          </p>
          <div className="mt-3">
            <ProjectMap map={data.map} />
          </div>
        </section>
        <section>
          <SectionLabel>ADR ({data.adrs.length})</SectionLabel>
          {data.adrs.length === 0 ? (
            <p className="mt-3 text-xs text-muted">Aucun ADR dans docs/adr/.</p>
          ) : (
            <Card className="mt-3">
              {data.adrs.map((adr, index) => (
                <AdrRow key={adr.file} adr={adr} first={index === 0} />
              ))}
            </Card>
          )}
        </section>
      </div>
    </>
  );
}
