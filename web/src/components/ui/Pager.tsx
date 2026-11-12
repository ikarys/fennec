// A list's footer pager: the page size, the range shown and previous / next; nothing when the list fits the smallest page.
import { useT } from '../../i18n';
import { PAGE_SIZES, type Paged } from '../../hooks/usePaged';
import { Button } from './Button';
import { ChevronRightIcon } from './Icons';
import { Segmented } from './Segmented';

export function Pager<T>({ paged }: { paged: Paged<T> }) {
  const t = useT();

  if (paged.total <= PAGE_SIZES[0]) return null;

  return (
    <div className="flex items-center justify-end gap-2 border-t border-border px-4 py-2">
      <Segmented
        label={t('pagerSize')}
        segments={PAGE_SIZES.map((size) => ({ value: size, label: size, title: t('pagerSizeTitle', { count: size }) }))}
        value={paged.size}
        onChange={paged.setSize}
      />
      <span className="ml-2 font-mono text-xs text-muted tabular-nums">
        {t('pagerRange', { from: paged.from, to: paged.to, total: paged.total })}
      </span>
      <Button
        variant="ghost"
        size="sm"
        iconOnly
        title={t('pagerPrevious')}
        aria-label={t('pagerPrevious')}
        disabled={paged.page === 0}
        onClick={() => paged.setPage(paged.page - 1)}
      >
        <ChevronRightIcon className="h-3.5 w-3.5 rotate-180" />
      </Button>
      <Button
        variant="ghost"
        size="sm"
        iconOnly
        title={t('pagerNext')}
        aria-label={t('pagerNext')}
        disabled={paged.page >= paged.pageCount - 1}
        onClick={() => paged.setPage(paged.page + 1)}
      >
        <ChevronRightIcon className="h-3.5 w-3.5" />
      </Button>
    </div>
  );
}
