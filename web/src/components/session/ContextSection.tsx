/**
 * The Session drawer's Context section: the gauge's detail, collapsed by default.
 * Every figure and sentence comes from the server.
 */
import type { ContextView } from '../../lib/api';
import type { BadgeTone } from '../ui/Badge';
import { TokensIcon } from '../ui/Icons';
import { Panel } from '../ui/Panel';
import { useT } from '../../i18n';
import { ContextComposition } from './ContextComposition';

const TONE: Record<ContextView['tone'], BadgeTone> = { ok: 'ok', warn: 'attention', err: 'err' };

export function ContextSection({ context, revealKey }: { context: ContextView | null; revealKey: number }) {
  const t = useT();

  return (
    <Panel
      id="context"
      title={t('contextOfModel')}
      defaultOpen={false}
      icon={<TokensIcon className="h-3.5 w-3.5" />}
      iconTone={context ? TONE[context.tone] : 'neutral'}
      summary={context?.summary}
      revealKey={revealKey}
    >
      {context === null ? (
        <p className="m-0 text-xs text-muted">{t('loading')}</p>
      ) : (
        <>
          <p className="m-0 text-xs text-muted">{context.title}</p>
          {context.composition && <ContextComposition composition={context.composition} />}
        </>
      )}
    </Panel>
  );
}
