// The app's status bar: the blocks a plugin placed there, in the order the
// server stacks the surfaces (global, project, session). Shown only when a
// plugin has placed a block; there is no header toggle.
import { useCallback, useEffect, useRef, useState } from 'react';
import { useT } from '../../i18n';
import { getStatusBarPluginBlocks, type ServedPluginBlock } from '../../lib/api';
import { usePluginsVersion } from '../../hooks/usePluginsVersion';
import { PuzzleIcon } from '../ui/Icons';
import { BlockElements } from '../plugins/BlockElements';

export function StatusBar({ sessionId, projectId }: { sessionId: string | null; projectId: string | null }) {
  const t = useT();
  const pluginsVersion = usePluginsVersion();
  const [blocks, setBlocks] = useState<ServedPluginBlock[]>([]);
  const fetchId = useRef(0);

  const fetchBlocks = useCallback(async () => {
    const id = ++fetchId.current;
    try {
      const data = await getStatusBarPluginBlocks({
        sessionId: sessionId ?? undefined,
        projectId: projectId ?? undefined
      });
      if (id === fetchId.current) setBlocks(data.blocks);
    } catch {
      if (id === fetchId.current) setBlocks([]);
    }
  }, [sessionId, projectId, pluginsVersion]);

  useEffect(() => {
    void fetchBlocks();
  }, [fetchBlocks]);

  if (blocks.length === 0) return null;

  return (
    <div
      role="region"
      aria-label={t('statusBarLabel')}
      className="flex shrink-0 items-center gap-x-3 overflow-x-auto border-t border-border bg-raised px-3 py-1"
    >
      {blocks.map((block) => {
        const provenance = t('pluginProvenance', { name: block.plugin });
        return (
          <span key={`${block.plugin}:${block.title}`} title={provenance} className="flex shrink-0 items-center gap-1.5">
            <PuzzleIcon className="h-3.5 w-3.5 shrink-0 text-faint" />
            <BlockElements elements={block.content.elements} compact />
          </span>
        );
      })}
    </div>
  );
}
