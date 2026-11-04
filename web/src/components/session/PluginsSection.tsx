// The session's plugin blocks (left bar): typed data the active plugins
// emitted, each with its provenance badge. The core renders it, never HTML.
import { useCallback, useEffect, useRef, useState } from 'react';
import { useT } from '../../i18n';
import { getSessionPluginBlocks, type PluginTone, type ServedPluginBlock } from '../../lib/api';
import { Badge, type BadgeTone } from '../ui/Badge';
import { Panel } from '../ui/Panel';
import { panelBlock } from '../../lib/panelBlocks';
import { PuzzleIcon } from '../Icons';

const TONE_TO_BADGE: Record<PluginTone, BadgeTone> = {
  none: 'neutral',
  info: 'info',
  success: 'ok',
  warning: 'attention',
  danger: 'err',
};

export function PluginsSection({ sessionId }: { sessionId: string | null }) {
  const t = useT();
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
  }, [sessionId]);

  useEffect(() => {
    void fetchBlocks();
  }, [fetchBlocks]);

  if (sessionId === null) {
    return null;
  }

  return (
    <Panel
      id="plugins"
      title={t(panelBlock('plugins').titleKey)}
      icon={<PuzzleIcon className="h-3.5 w-3.5" />}
      iconTone={blocks !== null && blocks.length > 0 ? 'accent' : 'neutral'}
      summary={blocks !== null ? String(blocks.length) : undefined}
    >
      <div className="flex flex-col gap-2">
        {blocks === null && <div className="text-xs text-muted">{t('loading')}</div>}
        {blocks !== null && blocks.length === 0 && (
          <div className="text-xs text-muted">{t('noPluginBlocks')}</div>
        )}
        {blocks?.map((block) => (
          <div key={`${block.plugin}:${block.title}`} className="rounded-md border border-border bg-raised/40 px-2 py-1.5">
            <div className="flex items-center gap-1.5">
              <span className="min-w-0 truncate text-xs font-medium text-text">{block.title}</span>
              <Badge tone="info" pill className="ml-auto shrink-0" title={t('pluginProvenance', { name: block.plugin })}>
                {t('pluginProvenance', { name: block.plugin })}
              </Badge>
            </div>
            <div className="mt-1 flex flex-col gap-1">
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
          </div>
        ))}
      </div>
    </Panel>
  );
}
