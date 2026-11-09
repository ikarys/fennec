// Fetches the session's served plugin blocks, refreshing when the plugin catalog changes.
import { useCallback, useEffect, useRef, useState } from 'react';
import { getSessionPluginBlocks, type ServedPluginBlock } from '../lib/api';
import { usePluginsVersion } from './usePluginsVersion';

export function useSessionPluginBlocks(sessionId: string | null): ServedPluginBlock[] | null {
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

  return blocks;
}
