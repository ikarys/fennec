// Project page plugins tab: the human's activations (enabled, network) and
// the project-scope blocks the active plugins emitted, each with its
// provenance badge. The core renders the blocks, never HTML.
import { useCallback, useEffect, useRef, useState } from 'react';
import { useT } from '../../i18n';
import {
  getProjectPlugins,
  setProjectPlugin,
  type PluginTone,
  type ProjectPlugin,
  type ServedPluginBlock,
} from '../../lib/api';
import { Badge, type BadgeTone } from '../ui/Badge';
import { Button } from '../ui/Button';
import { Card } from '../ui/Card';
import { Section } from '../ui/Section';
import { GlobeIcon, PlayIcon, PuzzleIcon, StopIcon } from '../Icons';

const TONE_TO_BADGE: Record<PluginTone, BadgeTone> = {
  none: 'neutral',
  info: 'info',
  success: 'ok',
  warning: 'attention',
  danger: 'err',
};

/** One block's typed elements, rendered by the core's primitives. */
function BlockContent({ block }: { block: ServedPluginBlock }) {
  return (
    <div className="flex flex-col gap-1">
      {block.content.elements.map((element, index) => {
        if (element.kind === 'text') {
          return (
            <span key={index} className="text-xs text-muted">
              {element.text}
            </span>
          );
        }
        if (element.kind === 'figure') {
          return (
            <div key={index} className="flex items-baseline gap-2">
              <span className="font-mono text-lg text-text tabular-nums">{element.value}</span>
              <span className="text-xs text-muted">{element.label}</span>
              {element.tone && element.tone !== 'none' ? (
                <Badge tone={TONE_TO_BADGE[element.tone]} pill>
                  {element.tone}
                </Badge>
              ) : null}
            </div>
          );
        }
        return (
          <Badge key={index} tone={element.tone ? TONE_TO_BADGE[element.tone] : 'neutral'} pill>
            {element.label}
          </Badge>
        );
      })}
    </div>
  );
}

export function PluginsSection({ projectId }: { projectId: string }) {
  const t = useT();
  const [plugins, setPlugins] = useState<ProjectPlugin[] | null>(null);
  const [blocks, setBlocks] = useState<ServedPluginBlock[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [busyName, setBusyName] = useState<string | null>(null);
  const fetchId = useRef(0);

  const fetchAll = useCallback(async () => {
    const id = ++fetchId.current;
    try {
      const data = await getProjectPlugins(projectId);
      if (id === fetchId.current) {
        setPlugins(data.plugins);
        setBlocks(data.blocks);
        setError(null);
      }
    } catch (loadError) {
      if (id === fetchId.current) {
        setError(loadError instanceof Error ? loadError.message : String(loadError));
      }
    }
  }, [projectId]);

  useEffect(() => {
    setPlugins(null);
    setBlocks([]);
    void fetchAll();
  }, [fetchAll]);

  const toggle = async (plugin: ProjectPlugin, patch: { enabled?: boolean; network?: boolean }) => {
    if (busyName !== null) return;
    setBusyName(plugin.name);
    try {
      await setProjectPlugin(
        projectId,
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
        {plugins !== null && plugins.length === 0 && (
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
                  aria-label={plugin.enabled ? t('pluginDisabled') : t('pluginEnabled')}
                  onClick={() => void toggle(plugin, { enabled: !plugin.enabled })}
                >
                  {plugin.enabled ? <StopIcon className="h-3.5 w-3.5" /> : <PlayIcon className="h-3.5 w-3.5" />}
                  {plugin.enabled ? t('pluginDisabled') : t('pluginEnabled')}
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
              <div key={`${block.plugin}:${block.title}`} className="rounded-md border border-border bg-raised/40 px-3 py-2">
                <div className="flex items-center gap-1.5">
                  <span className="min-w-0 truncate text-xs font-medium text-text">{block.title}</span>
                  <Badge tone="info" pill className="ml-auto shrink-0">
                    {t('pluginProvenance', { name: block.plugin })}
                  </Badge>
                </div>
                <div className="mt-1.5">
                  <BlockContent block={block} />
                </div>
              </div>
            ))}
          </div>
        )}
      </Section>
    </div>
  );
}
