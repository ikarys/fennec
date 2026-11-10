// Settings' Plugins tab: global plugin toggles and each plugin's emitted blocks.
import { useCallback, useEffect, useRef, useState } from 'react';
import { useT } from '../../i18n';
import {
  getGlobalPlugins,
  setGlobalPlugin,
  type GlobalPlugin,
  type ServedPluginBlock,
} from '../../lib/api';
import { usePluginsVersion } from '../../hooks/usePluginsVersion';
import { Section } from '../ui/Section';
import { AlertIcon } from '../ui/Icons';
import { InfoBar } from '../ui/InfoBar';
import { PluginCard } from './PluginCard';

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
    <div className="flex flex-col gap-6">
      <InfoBar>{t('pluginsDescription')}</InfoBar>
      {error ? <p className="m-0 text-xs text-err">{error}</p> : null}
      {plugins === null && !error && <p className="m-0 text-sm text-muted">{t('loading')}</p>}
      {problems.length > 0 && (
        <Section Icon={AlertIcon} title={t('pluginProblems')} flush>
          <div className="px-4 py-3">
            <ul className="m-0 space-y-1 p-0">
              {problems.map((problem) => (
                <li key={problem} className="font-mono text-xs text-text">
                  {problem}
                </li>
              ))}
            </ul>
          </div>
        </Section>
      )}
      {plugins !== null && plugins.length === 0 && problems.length === 0 && (
        <div className="rounded-lg border border-dashed border-border px-4 py-3 text-sm text-muted">
          {t('noPluginsDefined')}
        </div>
      )}
      {plugins !== null &&
        plugins.map((plugin) => (
          <PluginCard
            key={plugin.name}
            plugin={plugin}
            blocks={blocks.filter((block) => block.plugin === plugin.name)}
            busy={busyName !== null}
            onToggle={(patch) => void toggle(plugin, patch)}
          />
        ))}
    </div>
  );
}
