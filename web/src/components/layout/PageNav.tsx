// The header's page navigation: one icon per fixed page, lit when its tab is active.
import { useT } from '../../i18n';
import type { MsgKey } from '../../i18n/messages';
import type { PageId } from '../../lib/route';
import { BacklogIcon, InsightIcon, SettingsIcon, WorkflowsIcon } from '../ui/Icons';
import { buttonClass } from '../ui/Button';

export type { PageId };

/** The fixed pages, in the bar's order; `PageIcon` draws each. */
const PAGES: { page: PageId; title: MsgKey }[] = [
  { page: 'backlog', title: 'tasks' },
  { page: 'workflows', title: 'workflows' },
  { page: 'insight', title: 'insight' },
  { page: 'settings', title: 'settings' }
];

export function PageIcon({ page, className }: { page: PageId; className?: string }) {
  switch (page) {
    case 'settings':
      return <SettingsIcon className={className} />;
    case 'insight':
      return <InsightIcon className={className} />;
    case 'workflows':
      return <WorkflowsIcon className={className} />;
    case 'backlog':
      return <BacklogIcon className={className} />;
  }
}

export function PageNav({ activeTabId, onOpenPage }: { activeTabId: string | null; onOpenPage: (page: PageId) => void }) {
  const t = useT();
  return (
    <div role="group" className="flex items-center gap-0.5">
      {/* The page's icon is lit when its tab is active, so the tab and the icon show one state. */}
      {PAGES.map(({ page, title }) => {
        const active = activeTabId === `page:${page}`;
        return (
          <button
            key={page}
            type="button"
            onClick={() => onOpenPage(page)}
            aria-label={t(title)}
            aria-pressed={active}
            title={t(title)}
            className={buttonClass({ variant: 'icon', size: 'sm', iconOnly: true }, active ? 'bg-accent-soft text-accent' : undefined)}
          >
            <PageIcon page={page} className="h-4 w-4" />
          </button>
        );
      })}
    </div>
  );
}
