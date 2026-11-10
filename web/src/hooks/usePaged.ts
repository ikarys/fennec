// Slices an already-loaded list into pages; the page is clamped when the list or the page size changes.
import { useState } from 'react';

export const PAGE_SIZES = [25, 50, 100] as const;
export type PageSize = (typeof PAGE_SIZES)[number];

export interface Paged<T> {
  page: number;
  pageCount: number;
  size: PageSize;
  visible: T[];
  from: number;
  to: number;
  total: number;
  setPage: (page: number) => void;
  setSize: (size: PageSize) => void;
}

export function usePaged<T>(items: T[]): Paged<T> {
  const [page, setPage] = useState(0);
  const [size, setSize] = useState<PageSize>(PAGE_SIZES[0]);
  const pageCount = Math.max(1, Math.ceil(items.length / size));
  const effective = Math.min(page, pageCount - 1);

  return {
    page: effective,
    pageCount,
    size,
    visible: items.slice(effective * size, (effective + 1) * size),
    from: items.length === 0 ? 0 : effective * size + 1,
    to: Math.min(items.length, (effective + 1) * size),
    total: items.length,
    setPage,
    setSize: (next) => {
      setSize(next);
      setPage(0);
    },
  };
}
