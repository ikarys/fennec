/**
 * The project's module map: the arrows read from the imports of the code, and
 * the human's click that writes `docs/map.md`. It also shows the gap with the
 * intended architecture. The tab's content.
 */
import { useState } from 'react';
import { Badge } from '../ui/Badge';
import { Button } from '../ui/Button';
import { ProjectMap } from './ProjectMap';
import { writeMapFile, type ArchitectureDrift, type MapFileState, type ModuleMap } from '../../lib/api';

export function MapSection({
  projectId,
  map,
  mapFile,
  drift,
  onWritten,
}: {
  projectId: string;
  map: ModuleMap;
  mapFile: MapFileState;
  drift: ArchitectureDrift;
  onWritten: (state: MapFileState) => void;
}) {
  const [writing, setWriting] = useState(false);
  const [writeError, setWriteError] = useState<string | null>(null);

  const handleWrite = async () => {
    setWriting(true);
    setWriteError(null);
    try {
      const result = await writeMapFile(projectId);
      onWritten(result.mapFile);
    } catch (err) {
      setWriteError(err instanceof Error ? err.message : 'Erreur inconnue');
    } finally {
      setWriting(false);
    }
  };

  return (
    <section>
      <div className="flex items-center gap-2">
        <Badge tone="neutral" title="Extraite des imports du code, sans modèle">
          vérifié
        </Badge>
        <div className="grow" />
        {mapFile === 'current' ? (
          <Badge tone="ok">docs/map.md à jour</Badge>
        ) : mapFile === 'stale' ? (
          <Badge tone="accent">docs/map.md périmé</Badge>
        ) : (
          <Badge tone="neutral">docs/map.md absent</Badge>
        )}
        {mapFile !== 'current' ? (
          <Button
            variant="secondary"
            size="sm"
            disabled={writing}
            onClick={handleWrite}
          >
            {writing
              ? 'Écriture…'
              : mapFile === 'missing'
                ? 'Écrire docs/map.md'
                : 'Mettre à jour docs/map.md'}
          </Button>
        ) : null}
      </div>
      {writeError ? <p className="mt-1 text-xs text-err">{writeError}</p> : null}
      <p className="mt-2 text-xs text-muted">
        Une flèche A → B : du code de A importe du code de B (nombre de
        fichiers). docs/map.md, commité, donne cette carte aux agents et aux
        développeurs qui arrivent sur le projet.
      </p>
      <div className="mt-3 text-xs">
        {drift.rules === 0 ? (
          <p className="text-muted">
            Aucune règle de modules dans <code>.agents/architecture</code> (<code>A never imports B</code>) : rien à comparer.
          </p>
        ) : drift.violations.length === 0 ? (
          <Badge tone="ok">
            {drift.rules === 1
              ? 'Conforme à la règle de modules'
              : `Conforme aux ${drift.rules} règles de modules`}
          </Badge>
        ) : (
          <>
            <Badge tone="err">
              {drift.violations.length} dépendance(s) contre l'architecture voulue
            </Badge>
            <ul className="mt-1 space-y-0.5">
              {drift.violations.map((violation) => (
                <li
                  key={`${violation.from}->${violation.to}:${violation.rule}`}
                  className="font-mono"
                >
                  {violation.from} → {violation.to}
                  <span className="text-muted">
                    {' '}
                    ({violation.imports} fichier(s)) : {violation.rule}
                  </span>
                </li>
              ))}
            </ul>
          </>
        )}
        {drift.sinceMap !== null &&
        drift.sinceMap.added.length + drift.sinceMap.removed.length > 0 ? (
          <div className="mt-2">
            <p>
              Depuis docs/map.md : +{drift.sinceMap.added.length} / −{drift.sinceMap.removed.length} dépendance(s)
            </p>
            <details className="mt-1">
              <summary>Voir</summary>
              <ul className="mt-1 space-y-0.5 font-mono">
                {drift.sinceMap.added.map((edge) => (
                  <li key={`+${edge.from}->${edge.to}`} className="text-ok">
                    + {edge.from} → {edge.to}
                  </li>
                ))}
                {drift.sinceMap.removed.map((edge) => (
                  <li key={`-${edge.from}->${edge.to}`} className="text-err">
                    − {edge.from} → {edge.to}
                  </li>
                ))}
              </ul>
            </details>
          </div>
        ) : null}
      </div>
      <div className="mt-3">
        <ProjectMap map={map} />
      </div>
    </section>
  );
}
