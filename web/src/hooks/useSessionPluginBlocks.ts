// Fetches the session's plugin blocks and the rail tabs its active plugins declare, refreshing when the plugin catalog changes.
import { useCallback, useEffect, useRef, useState } from 'react';
import { getSessionPluginBlocks, type SessionPlugins } from '../lib/api';
import { usePluginsVersion } from './usePluginsVersion';

export function useSessionPluginBlocks(sessionId: string | null): SessionPlugins | null {
  const pluginsVersion = usePluginsVersion();
  const [data, setData] = useState<SessionPlugins | null>(null);
  const fetchId = useRef(0);

  const fetchBlocks = useCallback(async () => {
    if (!sessionId) {
      fetchId.current += 1;
      setData(null);
      return;
    }
    const id = ++fetchId.current;
    try {
      const result = await getSessionPluginBlocks(sessionId);
      if (id === fetchId.current) setData(result);
    } catch {
      if (id === fetchId.current) setData({ blocks: [], tabs: [] });
    }
  }, [sessionId, pluginsVersion]);

  useEffect(() => {
    void fetchBlocks();
  }, [fetchBlocks]);

  return data;
}
