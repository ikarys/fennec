// Filter global plugins by a case-insensitive query and status.
export type PluginStatusFilter = 'all' | 'enabled' | 'disabled';

/** The plugins whose name, command or arguments contain the query (case-insensitive, trimmed), in the given status. */
export function filterPlugins<P extends { name: string; command: string; args: string[]; enabled: boolean }>(
  plugins: P[],
  query: string,
  status: PluginStatusFilter,
): P[] {
  const q = query.trim().toLowerCase();
  return plugins.filter((plugin) => {
    if (status === 'enabled' && !plugin.enabled) return false;
    if (status === 'disabled' && plugin.enabled) return false;
    if (q.length === 0) return true;
    return [plugin.name, plugin.command, ...plugin.args].some((field) => field.toLowerCase().includes(q));
  });
}

/** How many plugins each status filter would show (ignoring the query). */
export function pluginStatusCounts(plugins: { enabled: boolean }[]): Record<PluginStatusFilter, number> {
  const enabled = plugins.filter((plugin) => plugin.enabled).length;
  return {
    all: plugins.length,
    enabled,
    disabled: plugins.length - enabled,
  };
}
