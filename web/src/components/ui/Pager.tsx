// A list's footer pager: the range shown and previous / next; nothing when one page is enough.
import { useT } from "../../i18n";
import { Button } from "./Button";
import { ChevronRightIcon } from "./Icons";

export function Pager({
  page,
  pageCount,
  from,
  to,
  total,
  onPage,
}: {
  page: number;
  pageCount: number;
  from: number;
  to: number;
  total: number;
  onPage: (page: number) => void;
}) {
  const t = useT();

  if (pageCount <= 1) return null;

  return (
    <div className="flex items-center justify-end gap-2 border-t border-border px-4 py-2">
      <span className="font-mono text-xs text-muted tabular-nums">
        {t("pagerRange", { from, to, total })}
      </span>
      <Button
        variant="ghost"
        size="sm"
        iconOnly
        title={t("pagerPrevious")}
        aria-label={t("pagerPrevious")}
        disabled={page === 0}
        onClick={() => onPage(page - 1)}
      >
        <ChevronRightIcon className="h-3.5 w-3.5 rotate-180" />
      </Button>
      <Button
        variant="ghost"
        size="sm"
        iconOnly
        title={t("pagerNext")}
        aria-label={t("pagerNext")}
        disabled={page >= pageCount - 1}
        onClick={() => onPage(page + 1)}
      >
        <ChevronRightIcon className="h-3.5 w-3.5" />
      </Button>
    </div>
  );
}
