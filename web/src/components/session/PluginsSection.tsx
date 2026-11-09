// The session's plugin blocks in the right-hand panel: the shared "Plugins"
// panel (placement `panel`) followed by one panel per `own_panel` block. One
// fetch for both; the data is indicative, each block carries its provenance.
import { useCallback, useEffect, useRef, useState } from 'react';
import { useT } from '../../i18n';
import { getSessionPluginBlocks, type ServedPluginBlock } from '../../lib/api';
import { usePluginsVersion } from '../../hooks/usePluginsVersion';
import { Badge } from '../ui/Badge';
import { Panel } from '../ui/Panel';
import { PuzzleIcon } from '../ui/Icons';
import { BlockElements } from '../plugins/BlockElements';
import { PluginBlockCard } from '../plugins/PluginBlockCard';

export function PluginsSection({ sessionId }: { sessionId: string | null }) {
  const t = useT();
  const pluginsVersion = usePluginsVersion();
  const [blocks, setBlocks] = useState<ServedPluginBlock[] | null>(null);
  const fetchId = useRef(0);

  const fetchBlocks = useCallback(async () => {
    if (!sessionId) {
      fetchId.current += 1;
      setBlocks(null);
      return;
    }
    const id = ++fetchId.current;
    try {
      const data = await getSessionPluginBlocks(sessionId);
      if (id === fetchId.current) setBlocks(data.blocks);
    } catch {
      if (id === fetchId.current) setBlocks([]);
    }
  }, [sessionId, pluginsVersion]);

  useEffect(() => {
    void fetchBlocks();
  }, [fetchBlocks]);

  if (sessionId === null) {
    return null;
  }

  const sharedBlocks = blocks?.filter((block) => block.placement !== 'own_panel') ?? [];
  const ownBlocks = blocks?.filter((block) => block.placement === 'own_panel') ?? [];

  return (
    <>
      <Panel
        id="plugins"
        title={t('panelPlugins')}
        icon={<PuzzleIcon className="h-3.5 w-3.5" />}
        iconTone={blocks !== null && blocks.length > 0 ? 'accent' : 'neutral'}
        summary={blocks !== null ? String(sharedBlocks.length) : undefined}
      >
        <div className="flex flex-col gap-2">
          {blocks === null && <div className="text-xs text-muted">{t('loading')}</div>}
          {blocks !== null && sharedBlocks.length === 0 && (
            <div className="text-xs text-muted">{t('noPluginBlocks')}</div>
          )}
          {sharedBlocks.map((block) => (
            <PluginBlockCard key={`${block.plugin}:${block.title}`} block={block} />
          ))}
        </div>
      </Panel>
      {ownBlocks.map((block) => (
        <Panel
          key={`${block.plugin}:${block.title}`}
          id={`plugin-own:${block.plugin}:${block.title}`}
          title={block.title}
        >
          <div className="flex flex-col gap-1.5">
            <Badge tone="info" appearance="pill" title={t('pluginProvenance', { name: block.plugin })}>
              {t('pluginProvenance', { name: block.plugin })}
            </Badge>
            <BlockElements elements={block.content.elements} />
          </div>
        </Panel>
      ))}
    </>
  );
}
