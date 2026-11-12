// The session's plugin blocks as pure renderers: shared blocks for the Plugins tab and
// one panel per block for a plugin's own-panel tab. Empty blocks show a waiting line.
import { useT } from '../../i18n';
import type { ServedPluginBlock } from '../../lib/api';
import { Badge } from '../ui/Badge';
import { Panel } from '../ui/Panel';
import { BlockElements } from '../plugins/BlockElements';
import { PluginBlockCard } from '../plugins/PluginBlockCard';

export function PluginsSection({ blocks }: { blocks: ServedPluginBlock[] }) {
  const t = useT();

  if (blocks.length === 0) {
    return <p className="p-3 text-sm text-muted">{t('pluginWaiting')}</p>;
  }

  return (
    <div className="flex flex-col gap-2 p-3">
      {blocks.map((block) => (
        <PluginBlockCard key={`${block.plugin}:${block.title}`} block={block} />
      ))}
    </div>
  );
}

export function PluginOwnSection({ plugin, blocks }: { plugin: string; blocks: ServedPluginBlock[] }) {
  const t = useT();

  return (
    <>
      <div className="px-3 pt-3">
        <Badge tone="info" appearance="pill" title={t('pluginProvenance', { name: plugin })}>
          {t('pluginProvenance', { name: plugin })}
        </Badge>
      </div>
      {blocks.length === 0 ? (
        <p className="p-3 text-sm text-muted">{t('pluginWaiting')}</p>
      ) : (
        blocks.map((block) => (
          <Panel
            key={`${block.plugin}:${block.title}`}
            id={`plugin-own:${block.plugin}:${block.title}`}
            title={block.title}
          >
            <BlockElements elements={block.content.elements} />
          </Panel>
        ))
      )}
    </>
  );
}
