import { useEffect, useMemo, useState } from "react";
import { DEFAULT_PAGE_SIZE, paginate } from "../components/ui/pagination";

/**
 * The rows of one page of a long list, and the props for the shared `<Pagination>` under it:
 * `const paging = usePagedList(rows, filter); … paging.rows.map(…) … <Pagination {...paging.props} />`.
 * The list starts again at the first page when `resetKey` changes (a filter, a status).
 */
export function usePagedList<T>(items: T[], resetKey?: unknown, pageSize = DEFAULT_PAGE_SIZE) {
  const [page, setPage] = useState(0);
  useEffect(() => { setPage(0); }, [resetKey]);
  const rows = useMemo(() => paginate(items, page, pageSize), [items, page, pageSize]);
  return { rows, props: { onPageChange: setPage, page, pageSize, total: items.length } };
}
