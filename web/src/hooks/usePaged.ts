// Slices an already-loaded list into pages; the page is clamped when the list shrinks.
import { useState } from "react";

export const PAGE_SIZE = 20;

export function usePaged<T>(
  items: T[],
  size = PAGE_SIZE,
): {
  page: number;
  pageCount: number;
  visible: T[];
  setPage: (page: number) => void;
  from: number;
  to: number;
} {
  const [page, setPage] = useState(0);
  const pageCount = Math.max(1, Math.ceil(items.length / size));
  const effective = Math.min(page, pageCount - 1);
  const visible = items.slice(effective * size, effective * size + size);
  const from = items.length === 0 ? 0 : effective * size + 1;
  const to = Math.min(items.length, (effective + 1) * size);

  return {
    page: effective,
    pageCount,
    visible,
    setPage,
    from,
    to,
  };
}
