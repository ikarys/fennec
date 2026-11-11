/**
 * A node run's output, opened from the run rail's clickable port: what the
 * agent found (its report, a check's output), without hunting the thread.
 */
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import type { NodeRun } from '../../../lib/workflowRun';
import { portTone } from '../../../lib/workflowRun';
import { ExternalIcon } from '../../ui/Icons';
import { markdownComponents } from '../../session/markdownComponents';
import { Badge } from '../../ui/Badge';
import { buttonClass } from '../../ui/Button';
import { Card } from '../../ui/Card';
import { Modal } from '../../ui/Modal';

const REMARK_PLUGINS = [remarkGfm];

function badgeTone(port: string): 'ok' | 'err' | 'accent' {
  const tone = portTone(port);
  if (tone === 'positive') return 'ok';
  if (tone === 'negative') return 'err';
  return 'accent';
}

export function NodeOutputModal({ nodeRun, onClose }: { nodeRun: NodeRun | null; onClose: () => void }) {
  if (!nodeRun) return null;
  const { output, kind, label, port, childSessionId } = nodeRun;
  return (
    <Modal open onClose={onClose} label={label} width="w-[min(92vw,720px)]">
      <Card className="flex flex-col gap-3 p-4">
        <div className="flex items-center gap-2">
          <h2 className="m-0 min-w-0 truncate text-sm font-semibold" title={label}>
            {label}
          </h2>
          {port ? <Badge tone={badgeTone(port)}>{port}</Badge> : null}
        </div>
        <div className="max-h-[70vh] overflow-y-auto rounded-md border border-border bg-raised px-3 py-2 text-xs">
          {kind === 'shell' || kind === 'criteria' ? (
            <pre className="whitespace-pre-wrap break-words font-mono text-xs text-muted">{output}</pre>
          ) : (
            <ReactMarkdown remarkPlugins={REMARK_PLUGINS} components={markdownComponents}>
              {output}
            </ReactMarkdown>
          )}
        </div>
        {childSessionId ? (
          <div className="flex justify-end">
            <a href={`#/s/${childSessionId}`} className={buttonClass({ variant: 'ghost', size: 'sm' })}>
              <ExternalIcon className="h-3 w-3" />
              Ouvrir la session
            </a>
          </div>
        ) : null}
      </Card>
    </Modal>
  );
}
