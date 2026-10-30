/**
 * The project's module map: the arrows read from the imports of the code, and
 * the human's click that writes `docs/map.md`. The tab's content.
 */
import { useState } from 'react';
import { Badge } from '../ui/Badge';
import { Button } from '../ui/Button';
import { ProjectMap } from './ProjectMap';
import { writeMapFile, type MapFileState, type ModuleMap } from '../../lib/api';

export function MapSection({
  projectId,
  map,
  mapFile,
  onWritten,
}: {
  projectId: string;
  map: ModuleMap;
  mapFile: MapFileState;
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
      <div className="mt-3">
        <ProjectMap map={map} />
      </div>
    </section>
  );
}
