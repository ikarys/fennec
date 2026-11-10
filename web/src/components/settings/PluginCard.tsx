// A framed card for one global plugin: its status, toggles, env, and emitted blocks.
import type { GlobalPlugin, ServedPluginBlock } from '../../lib/api';
import { Badge } from '../ui/Badge';
import { Button } from '../ui/Button';
import { GlobeIcon, PlayIcon, PuzzleIcon, StopIcon } from '../ui/Icons';
import { Section } from '../ui/Section';
import { PluginBlockCard } from '../plugins/PluginBlockCard';
import { useT } from '../../i18n';

interface PluginCardProps {
  plugin: GlobalPlugin;
  blocks: ServedPluginBlock[];
  busy: boolean;
  onToggle: (patch: { enabled?: boolean; network?: boolean }) => void;
}

export function PluginCard({ plugin, blocks, busy, onToggle }: PluginCardProps) {
  const t = useT();
  const commandArgs = `${plugin.command} ${plugin.args.join(' ')}`;
  const envEntries = Object.entries(plugin.env);
  const envLine = envEntries.map(([key, value]) => `${key}=${value}`).join(' ');

  return (
    <Section
      Icon={PuzzleIcon}
      flush
      title={<span className="font-mono">{plugin.name}</span>}
      description={<span className="font-mono" title={commandArgs}>{commandArgs}</span>}
      actions={
        <>
          {plugin.active ? (
            <Badge tone="ok" appearance="pill" dot pulse={plugin.enabled}>
              {t('pluginEnabled')}
            </Badge>
          ) : plugin.enabled ? (
            <Badge tone="attention" appearance="pill" title={t('pluginCodeChanged')}>
              {t('pluginCodeChanged')}
            </Badge>
          ) : (
            <Badge tone="neutral" appearance="pill">
              {t('pluginDisabled')}
            </Badge>
          )}
          <Button
            variant="ghost"
            size="sm"
            iconOnly
            aria-pressed={plugin.network}
            aria-label={t('pluginNetwork')}
            title={t('pluginNetwork')}
            disabled={busy}
            onClick={() => onToggle({ network: !plugin.network })}
            className={plugin.network ? 'text-accent-strong' : ''}
          >
            <GlobeIcon className="h-3.5 w-3.5" />
          </Button>
          <Button
            variant={plugin.enabled ? 'soft' : 'secondary'}
            size="sm"
            iconOnly
            disabled={busy}
            aria-label={plugin.enabled ? t('pluginDisable') : t('pluginEnable')}
            title={plugin.enabled ? t('pluginDisable') : t('pluginEnable')}
            onClick={() => onToggle({ enabled: !plugin.enabled })}
          >
            {plugin.enabled ? <StopIcon className="h-4 w-4" /> : <PlayIcon className="h-4 w-4" />}
          </Button>
        </>
      }
    >
      {envLine ? (
        <div className="px-4 py-2.5 border-b border-border font-mono text-xs text-faint truncate" title={envLine}>
          {t('pluginEnv')}: {envLine}
        </div>
      ) : null}
      {blocks.length === 0 ? (
        <p className="m-0 px-4 py-3 text-xs text-faint">{t('noPluginBlocks')}</p>
      ) : (
        <div className="grid grid-cols-1 gap-2 p-4 sm:grid-cols-2">
          {blocks.map((block) => (
            <PluginBlockCard key={`${block.plugin}:${block.title}`} block={block} />
          ))}
        </div>
      )}
    </Section>
  );
}
