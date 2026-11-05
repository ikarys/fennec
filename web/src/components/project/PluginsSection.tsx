// The project dashboard's plugin blocks (placement `dashboard`): one card
// per block. Renders nothing when no block is placed.
import { useCallback, useEffect, useRef, useState } from 'react';
import { useT } from '../../i18n';
import { getProjectPluginBlocks, type ServedPluginBlock } from '../../lib/api';
import { usePluginsVersion } from '../../hooks/usePluginsVersion';
import { Section } from '../ui/Section';
import { PluginBlockCard } from '../plugins/PluginBlockCard';

export function PluginsSection({ projectId }: { projectId: string }) {
  const t = useT();
  const pluginsVersion = usePluginsVersion();
  const [blocks, setBlocks] = useState<ServedPluginBlock[]>([]);
  const fetchId = useRef(0);

  const fetchBlocks = useCallback(async () => {
    const id = ++fetchId.current;
    try {
      const data = await getProjectPluginBlocks(projectId);
      if (id === fetchId.current) setBlocks(data.blocks);
    } catch {
      if (id === fetchId.current) setBlocks([]);
    }
  }, [projectId, pluginsVersion]);

  useEffect(() => {
    void fetchBlocks();
  }, [fetchBlocks]);

  if (blocks.length === 0) return null;

  return (
    <Section title={t('panelPlugins')} flush>
      <div className="grid grid-cols-1 gap-2 sm:grid-cols-2">
        {blocks.map((block) => (
          <PluginBlockCard key={`${block.plugin}:${block.title}`} block={block} />
        ))}
      </div>
    </Section>
  );
}
