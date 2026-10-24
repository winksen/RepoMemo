const names = (metadata: unknown, key: string): string[] => {
  const value = (metadata as Record<string, unknown> | null | undefined)?.[key];
  return Array.isArray(value) ? value.map(String).filter(Boolean) : [];
};

/** What the reviewers of a pull request said, from what its connector kept (V0.3.21); null while nobody has decided. */
export function describeReview(metadata: unknown): string | null {
  const approved = names(metadata, "approved_by");
  const changes = names(metadata, "changes_requested_by");
  const parts = [];
  if (approved.length) parts.push(`Approved by ${approved.join(", ")}`);
  if (changes.length) parts.push(`Changes requested by ${changes.join(", ")}`);
  return parts.length ? parts.join(" · ") : null;
}

export function PullReviewSummary({ metadata }: { metadata: unknown }) {
  const text = describeReview(metadata);
  return text ? <p className="shared-muted-copy" role="status">{text}</p> : null;
}
