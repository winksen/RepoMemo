import { FILE_CATEGORIES } from "../constants";

/** The coverage rows with the template's preferred kinds of file first, in its order; the rest keep their order (V0.2.184). */
export function orderCoverage<T extends { label: string }>(rows: T[], first: string[] | null | undefined): T[] {
  const wanted = (first ?? [])
    .map((type) => FILE_CATEGORIES.find((category) => category.value === type)?.label.toLowerCase())
    .filter((label): label is string => Boolean(label));
  const rank = (row: T) => {
    const at = wanted.indexOf(row.label.toLowerCase());
    return at === -1 ? wanted.length : at;
  };
  return rows.map((row, index) => ({ row, index })).sort((a, b) => rank(a.row) - rank(b.row) || a.index - b.index).map(({ row }) => row);
}

/** How early a kind of link comes: the template's preferred kinds first, in its order; `similar` before `cites` otherwise. */
export function relationRank(kind: string, first: string[] | null | undefined): number {
  const wanted = first ?? [];
  const at = wanted.indexOf(kind);
  if (at !== -1) return at;
  return wanted.length + (kind === "similar" ? 0 : 1);
}

/** The links ordered so the preferred kinds come first (a table) or last (drawn on top of the others). */
export function orderLinks<T extends { kind: string }>(links: T[], first: string[] | null | undefined, onTop = false): T[] {
  const sign = onTop ? -1 : 1;
  return links.map((link, index) => ({ link, index })).sort((a, b) => sign * (relationRank(a.link.kind, first) - relationRank(b.link.kind, first)) || a.index - b.index).map(({ link }) => link);
}
