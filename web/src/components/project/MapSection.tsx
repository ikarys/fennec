/**
 * The project's module map: the arrows read from the imports of the code, and
 * the human's click that writes `docs/map.md`. It also shows the gap with the
 * intended architecture. The tab's content.
 */
import { useState } from 'react';
import {
  AlertIcon,
  BranchIcon,
  ClockIcon,
  LayersIcon,
  PinIcon,
  RefreshIcon,
  SpinnerIcon,
} from '../Icons';
import {
  writeMapFile,
  type ArchitectureDrift,
  type MapFileState,
  type ModuleMap,
} from '../../lib/api';
import { Badge } from '../ui/Badge';
import { Button } from '../ui/Button';
import { Card } from '../ui/Card';
import { Section } from '../ui/Section';
import { Tile } from '../ui/Tile';
import { ProjectMap } from './ProjectMap';

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

  const entries = map.modules.filter((m) => m.entry).length;
  const added = drift.sinceMap?.added.length ?? 0;
  const removed = drift.sinceMap?.removed.length ?? 0;
  const hasChanges = added + removed > 0;

  return (
    <Section
      title="Carte des modules"
      description="Une flèche A → B : du code de A importe du code de B (nombre de fichiers). docs/map.md, commité, donne cette carte aux agents et aux développeurs qui arrivent sur le projet."
      actions={
        <div className="flex items-center gap-2">
          <Badge tone="ok" pill title="Extraite des imports du code, sans modèle">
            vérifié
          </Badge>
          {mapFile === 'current' ? (
            <Badge tone="ok" pill>
              docs/map.md à jour
            </Badge>
          ) : mapFile === 'stale' ? (
            <Badge tone="attention" pill>
              docs/map.md périmé
            </Badge>
          ) : (
            <Badge tone="neutral" pill>
              docs/map.md absent
            </Badge>
          )}
          {mapFile !== 'current' ? (
            <Button
              variant="primary"
              size="sm"
              disabled={writing}
              onClick={handleWrite}
            >
              <span className="flex items-center gap-1.5">
                {writing ? (
                  <SpinnerIcon className="h-3.5 w-3.5" />
                ) : (
                  <RefreshIcon className="h-3.5 w-3.5" />
                )}
                {writing
                  ? 'Écriture…'
                  : mapFile === 'missing'
                    ? 'Écrire docs/map.md'
                    : 'Mettre à jour docs/map.md'}
              </span>
            </Button>
          ) : null}
        </div>
      }
    >
      <div className="flex flex-col gap-4">
        {writeError ? <p className="text-xs text-err">{writeError}</p> : null}

        {/* Summarize the map and drift before the detailed lists. */}
        <div className="grid grid-cols-2 gap-3 lg:grid-cols-4">
          <Tile
            icon={<LayersIcon />}
            label="Modules"
            figure={map.modules.length}
            detail={`${entries} point(s) d'entrée`}
            tone="accent"
            active
          />
          <Tile
            icon={<BranchIcon />}
            label="Dépendances"
            figure={map.dependencies.length}
            detail="entre modules, lues des imports"
            tone="info"
            active
          />
          <Tile
            icon={<PinIcon />}
            label="Règles de modules"
            figure={drift.rules}
            detail={
              drift.rules === 0
                ? "aucune dans .agents/architecture"
                : drift.violations.length === 0
                  ? "toutes respectées"
                  : `${drift.violations.length} dépendance(s) contre l'architecture voulue`
            }
            tone={
              drift.rules === 0
                ? 'neutral'
                : drift.violations.length === 0
                  ? 'ok'
                  : 'err'
            }
            active={drift.rules !== 0}
          />
          <Tile
            icon={<ClockIcon />}
            label="Depuis docs/map.md"
            figure={
              drift.sinceMap === null ? '—' : `+${added} / −${removed}`
            }
            detail={
              drift.sinceMap === null
                ? "pas de docs/map.md commité"
                : hasChanges
                  ? "dépendances ajoutées / retirées"
                  : "aucune dépendance changée"
            }
            tone={
              drift.sinceMap === null
                ? 'neutral'
                : hasChanges
                  ? 'attention'
                  : 'ok'
            }
            active={drift.sinceMap !== null}
          />
        </div>

        {drift.rules === 0 ? (
          <p className="text-xs text-muted">
            Aucune règle de modules dans <code>.agents/architecture</code> (
            <code>A never imports B</code>) : rien à comparer.
          </p>
        ) : null}

        {/* Only render the violations table when there is at least one rule breach. */}
        {drift.violations.length > 0 ? (
          <Card className="overflow-hidden">
            <div className="flex items-center gap-3 border-b border-border bg-raised px-4 py-2 text-xs uppercase tracking-wide text-faint">
              <span className="w-3.5" aria-hidden />
              <span className="grow">Dépendance interdite</span>
              <span className="w-20 text-right">Fichiers</span>
              <span className="w-64">Règle</span>
            </div>
            <ul className="divide-y divide-border">
              {drift.violations.map((violation) => (
                <li
                  key={`${violation.from}->${violation.to}:${violation.rule}`}
                  className="flex items-center gap-3 px-4 py-2.5 text-sm"
                >
                  <AlertIcon className="h-3.5 w-3.5 shrink-0 text-err" />
                  <span className="grow font-mono text-xs text-text">
                    {violation.from} → {violation.to}
                  </span>
                  <span className="w-20 text-right tabular-nums">
                    {violation.imports}
                  </span>
                  <span
                    className="w-64 truncate text-xs text-muted"
                    title={violation.rule}
                  >
                    {violation.rule}
                  </span>
                </li>
              ))}
            </ul>
          </Card>
        ) : null}

        {/* Cap the diff list so a large change set cannot push the map out of view. */}
        {drift.sinceMap !== null && hasChanges ? (
          <Card className="overflow-hidden">
            <div className="flex items-center gap-3 border-b border-border bg-raised px-4 py-2 text-xs uppercase tracking-wide text-faint">
              <span className="grow">Écarts depuis docs/map.md</span>
              <Badge tone="ok" pill>
                +{added}
              </Badge>
              <Badge tone="err" pill>
                −{removed}
              </Badge>
            </div>
            <ul className="max-h-64 divide-y divide-border overflow-y-auto">
              {drift.sinceMap.added.map((edge) => (
                <li
                  key={`+${edge.from}->${edge.to}`}
                  className="px-4 py-2 font-mono text-xs text-ok"
                >
                  + {edge.from} → {edge.to}
                </li>
              ))}
              {drift.sinceMap.removed.map((edge) => (
                <li
                  key={`-${edge.from}->${edge.to}`}
                  className="px-4 py-2 font-mono text-xs text-err"
                >
                  − {edge.from} → {edge.to}
                </li>
              ))}
            </ul>
          </Card>
        ) : null}

        <ProjectMap map={map} />
      </div>
    </Section>
  );
}
