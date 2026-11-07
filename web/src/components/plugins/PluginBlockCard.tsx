/**
 * One served plugin block as a card: its title, its provenance badge (what a
 * plugin shows is indicative, AGENTS.md rule 3), its typed elements. Used by
 * the session panel, the settings' Plugins tab and the project dashboard.
 */
import { useT } from '../../i18n';
import type { ServedPluginBlock } from '../../lib/api';
import { Badge } from '../ui/Badge';
import { BlockElements } from './BlockElements';

export function PluginBlockCard({ block }: { block: ServedPluginBlock }) {
  const t = useT();
  const provenance = t('pluginProvenance', { name: block.plugin });
  return (
    <div className="rounded-md border border-border bg-raised/40 px-2.5 py-2">
      <div className="flex items-center gap-1.5">
        <span className="min-w-0 truncate text-xs font-medium text-text">{block.title}</span>
        <Badge tone="info" appearance="pill" className="ml-auto shrink-0" title={provenance}>
          {provenance}
        </Badge>
      </div>
      <div className="mt-1.5">
        <BlockElements elements={block.content.elements} />
      </div>
    </div>
  );
}
