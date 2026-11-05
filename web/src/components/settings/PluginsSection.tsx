// Settings' Plugins tab: the human's global activations (enabled, network)
// and the global-scope blocks the active plugins emitted, each with its
// provenance badge. The core renders the blocks, never HTML.
import { useCallback, useEffect, useRef, useState } from 'react';
import { useT } from '../../i18n';
import {
  getGlobalPlugins,
  setGlobalPlugin,
  type GlobalPlugin,
  type ServedPluginBlock,
} from '../../lib/api';
import { usePluginsVersion } from '../../hooks/usePluginsVersion';
import { Badge } from '../ui/Badge';
import { Button } from '../ui/Button';
import { Card } from '../ui/Card';
import { Section } from '../ui/Section';
import { GlobeIcon, PlayIcon, PuzzleIcon, StopIcon } from '../Icons';
import { PluginBlockCard } from '../plugins/PluginBlockCard';

export function PluginsSection() {
  const t = useT();
  const pluginsVersion = usePluginsVersion();
  const [plugins, setPlugins] = useState<GlobalPlugin[] | null>(null);
  const [blocks, setBlocks] = useState<ServedPluginBlock[]>([]);
  const [problems, setProblems] = useState<string[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [busyName, setBusyName] = useState<string | null>(null);
  const fetchId = useRef(0);

  const fetchAll = useCallback(async () => {
    const id = ++fetchId.current;
    try {
      const data = await getGlobalPlugins();
      if (id === fetchId.current) {
        setPlugins(data.plugins);
        setBlocks(data.blocks);
        setProblems(data.problems);
        setError(null);
      }
    } catch (loadError) {
      if (id === fetchId.current) {
        setError(loadError instanceof Error ? loadError.message : String(loadError));
      }
    }
  }, [pluginsVersion]);

  useEffect(() => {
    setPlugins(null);
    setBlocks([]);
    void fetchAll();
  }, [fetchAll]);

  const toggle = async (plugin: GlobalPlugin, patch: { enabled?: boolean; network?: boolean }) => {
    if (busyName !== null) return;
    setBusyName(plugin.name);
    try {
      await setGlobalPlugin(
        plugin.name,
        patch.enabled ?? plugin.enabled,
        patch.network ?? plugin.network,
      );
      await fetchAll();
    } catch {
      // The state is re-read on the next action; a failed write shows the old one.
    } finally {
      setBusyName(null);
    }
  };

  return (
    <div className="space-y-4">
      <Section title={t('panelPlugins')} flush description={t('pluginsDescription')}>
        {error ? <div className="px-4 pb-3 text-err text-xs">{error}</div> : null}
        {plugins === null && !error && <div className="px-4 py-3 text-sm text-muted">{t('loading')}</div>}
        {problems.length > 0 && (
          <Card className="mx-4 mb-3 overflow-hidden">
            <div className="border-b border-border bg-attention/10 px-3 py-1.5 text-xs font-medium text-attention">
              {t('pluginProblems')}
            </div>
            <ul className="space-y-1 px-3 py-2">
              {problems.map((problem) => (
                <li key={problem} className="font-mono text-xs text-text">
                  {problem}
                </li>
              ))}
            </ul>
          </Card>
        )}
        {plugins !== null && plugins.length === 0 && problems.length === 0 && (
          <div className="px-4 py-3 text-sm text-muted">{t('noPluginsDefined')}</div>
        )}
        {plugins !== null && plugins.length > 0 && (
          <Card className="overflow-hidden">
            {plugins.map((plugin, index) => (
              <div
                key={plugin.name}
                className={`flex flex-wrap items-center gap-2 px-4 py-2.5 ${index > 0 ? 'border-t border-border' : ''}`}
              >
                <PuzzleIcon className={`h-4 w-4 shrink-0 ${plugin.active ? 'text-accent' : 'text-faint'}`} />
                <div className="min-w-0 flex-1">
                  <div className="flex items-center gap-2">
                    <span className="truncate font-mono text-xs text-text">{plugin.name}</span>
                    {plugin.active ? (
                      <Badge tone="ok" pill dot pulse={plugin.enabled}>
                        {t('pluginEnabled')}
                      </Badge>
                    ) : plugin.enabled ? (
                      <Badge tone="attention" pill title={t('pluginCodeChanged')}>
                        {t('pluginCodeChanged')}
                      </Badge>
                    ) : (
                      <Badge tone="neutral" pill>
                        {t('pluginDisabled')}
                      </Badge>
                    )}
                  </div>
                  <span className="block truncate text-xs text-faint" title={`${plugin.command} ${plugin.args.join(' ')}`}>
                    {plugin.command} {plugin.args.join(' ')}
                  </span>
                  {Object.keys(plugin.env).length > 0 ? (
                    <span
                      className="block truncate font-mono text-xs text-faint"
                      title={Object.entries(plugin.env)
                        .map(([key, value]) => `${key}=${value}`)
                        .join(' ')}
                    >
                      {t('pluginEnv')}: {Object.entries(plugin.env)
                        .map(([key, value]) => `${key}=${value}`)
                        .join(' ')}
                    </span>
                  ) : null}
                </div>
                <Button
                  variant="ghost"
                  size="sm"
                  iconOnly
                  aria-pressed={plugin.network}
                  aria-label={t('pluginNetwork')}
                  title={t('pluginNetwork')}
                  disabled={busyName !== null}
                  onClick={() => void toggle(plugin, { network: !plugin.network })}
                  className={plugin.network ? 'text-accent-strong' : ''}
                >
                  <GlobeIcon className="h-3.5 w-3.5" />
                </Button>
                <Button
                  variant={plugin.enabled ? 'soft' : 'primary'}
                  size="sm"
                  disabled={busyName !== null}
                  aria-label={plugin.enabled ? t('pluginDisable') : t('pluginEnable')}
                  onClick={() => void toggle(plugin, { enabled: !plugin.enabled })}
                >
                  {plugin.enabled ? <StopIcon className="h-3.5 w-3.5" /> : <PlayIcon className="h-3.5 w-3.5" />}
                  {plugin.enabled ? t('pluginDisable') : t('pluginEnable')}
                </Button>
              </div>
            ))}
          </Card>
        )}
      </Section>
      <Section title={t('pluginBlocksTitle')} flush>
        {blocks.length === 0 ? (
          <div className="px-4 py-3 text-sm text-muted">{t('noProjectBlocks')}</div>
        ) : (
          <div className="grid grid-cols-1 gap-2 sm:grid-cols-2">
            {blocks.map((block) => (
              <PluginBlockCard key={`${block.plugin}:${block.title}`} block={block} />
            ))}
          </div>
        )}
      </Section>
    </div>
  );
}
