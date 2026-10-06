import { IconChevronLeft as ChevronLeft, IconChevronRight as ChevronRight } from "@tabler/icons-react";
import { useEffect } from "react";
import { Button } from "./button";

export const DEFAULT_PAGE_SIZE = 24;

/** Slice of `items` for the zero-based `page`. */
export function paginate<T>(items: T[], page: number, pageSize = DEFAULT_PAGE_SIZE) {
  return items.slice(page * pageSize, (page + 1) * pageSize);
}

/** Previous/next controls; renders nothing while everything fits on one page. */
export function Pagination({ onPageChange, page, pageSize = DEFAULT_PAGE_SIZE, total }: {
  onPageChange: (page: number) => void;
  page: number;
  pageSize?: number;
  total: number;
}) {
  const pageCount = Math.max(1, Math.ceil(total / pageSize));
  // Filters can shrink the list below the current page.
  useEffect(() => { if (page > pageCount - 1) onPageChange(pageCount - 1); }, [onPageChange, page, pageCount]);
  if (total <= pageSize) return null;
  const first = page * pageSize + 1;
  const last = Math.min(total, (page + 1) * pageSize);
  return <nav aria-label="Pagination" className="rm-pagination">
    <span>{first}–{last} of {total}</span>
    <div>
      <Button aria-label="Previous page" disabled={page === 0} onClick={() => onPageChange(page - 1)} type="button" variant="secondary"><ChevronLeft size={16} /></Button>
      <span aria-current="page">Page {page + 1} of {pageCount}</span>
      <Button aria-label="Next page" disabled={page >= pageCount - 1} onClick={() => onPageChange(page + 1)} type="button" variant="secondary"><ChevronRight size={16} /></Button>
    </div>
  </nav>;
}
