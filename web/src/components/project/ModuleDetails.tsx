import { Badge } from '../ui/Badge';
import { Button } from '../ui/Button';
import { SectionLabel } from '../ui/SectionLabel';
import type { MapLink, MapModule } from '../../lib/api';

export function ModuleDetails({
  module,
  onOpen,
  onClose,
}: {
  module: MapModule;
  onOpen: (id: string) => void;
  onClose: () => void;
}) {
  return (
    <aside className="flex w-80 shrink-0 flex-col gap-3 overflow-y-auto border-l border-border bg-surface p-3 text-xs">
      <div className="flex items-start justify-between gap-2">
        <div className="min-w-0 break-all font-mono text-sm">{module.id}</div>
        <Button variant="ghost" size="sm" iconOnly aria-label="Fermer" onClick={onClose}>
          ×
        </Button>
      </div>

      <div className="flex flex-col gap-1">
        <Badge tone="neutral" title="Tiré des commentaires d'en-tête et des imports du code, sans modèle">
          vérifié
        </Badge>
        <span>
          {module.files} fichiers · {module.tests} testés{module.entry ? ' · entrée' : ''}
        </span>
      </div>

      {module.summary ? (
        <p>{module.summary}</p>
      ) : (
        <p className="text-muted italic">
          Aucun commentaire d'en-tête : le fichier principal du module ne dit pas ce qu'il fait.
        </p>
      )}

      <div className="flex flex-col gap-1">
        <SectionLabel>Importe</SectionLabel>
        <LinkList links={module.uses} onOpen={onOpen} />
      </div>

      <div className="flex flex-col gap-1">
        <SectionLabel>Importé par</SectionLabel>
        <LinkList links={module.usedBy} onOpen={onOpen} />
      </div>

      <div className="flex flex-col gap-1">
        <SectionLabel>Fichiers</SectionLabel>
        {module.sources.length === 0 ? (
          <span className="text-muted">—</span>
        ) : (
          <ul className="flex flex-col gap-2">
            {module.sources.map((source) => (
              <li key={source.path} className="flex flex-col gap-1">
                <span className="break-all font-mono">{source.path}</span>
                {source.summary ? <span className="text-muted">{source.summary}</span> : null}
              </li>
            ))}
          </ul>
        )}
      </div>
    </aside>
  );
}

function LinkList({ links, onOpen }: { links: MapLink[]; onOpen: (id: string) => void }) {
  if (links.length === 0) return <span className="text-muted">—</span>;
  return (
    <ul className="flex flex-col gap-1">
      {links.map((link) => (
        <li key={link.module}>
          <Button variant="ghost" size="sm" onClick={() => onOpen(link.module)}>
            <span className="mr-1 font-mono">{link.module}</span>
            <span className="text-muted" title="fichiers qui importent">
              {link.imports}
            </span>
          </Button>
        </li>
      ))}
    </ul>
  );
}
