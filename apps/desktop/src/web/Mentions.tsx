import { useEffect, useState } from "react";
import { getSharedArtifactMentions } from "../lib/sharedApi";
import type { ArtifactMention, ArtifactMentions } from "../types";
import { artifactTypeLabel } from "./format";

const KIND_LABEL: Record<string, string> = {
  file: "names the file",
  symbol: "names the symbol",
  commit: "names the commit",
  item: "refers to",
};

function describe(mention: ArtifactMention) {
  const what = KIND_LABEL[mention.kind] ?? mention.kind;
  const label = mention.kind === "commit" ? mention.label.slice(0, 7) : mention.label;
  return `${what} ${label}`;
}

/** Issues and pull requests that name this file, and the files, symbols and
 *  commits that this issue or pull request names (roadmap F17d). Shown only
 *  when there is something to show. */
export function ArtifactMentionsPanel({
  accessToken,
  artifactId,
  onOpenArtifact,
}: {
  accessToken: string;
  artifactId: string;
  onOpenArtifact: (artifactId: string) => void;
}) {
  const [mentions, setMentions] = useState<ArtifactMentions | null>(null);

  useEffect(() => {
    let current = true;
    setMentions(null);
    getSharedArtifactMentions(accessToken, artifactId)
      .then((result) => { if (current) setMentions(result); })
      .catch(() => { if (current) setMentions(null); });
    return () => { current = false; };
  }, [accessToken, artifactId]);

  if (!mentions || (!mentions.mentioned_in.length && !mentions.mentions.length)) return null;

  const list = (items: ArtifactMention[]) => <ul className="rm-mention-links">
    {items.map((mention) => <li key={`${mention.artifact_id}-${mention.kind}-${mention.label}`}>
      <button className="rm-map-link" onClick={() => onOpenArtifact(mention.artifact_id)} type="button">{mention.title}</button>
      <span className="rm-map-muted"> · {artifactTypeLabel(mention.artifact_type)} · {describe(mention)}</span>
    </li>)}
  </ul>;

  return <section className="shared-record-panel">
    {mentions.mentioned_in.length ? <>
      <h2>Mentioned in</h2>
      <p className="shared-muted-copy">Issues and pull requests whose text names this file, its symbols or the commit that last changed it, or that refer to this item by its number or address.</p>
      {list(mentions.mentioned_in)}
    </> : null}
    {mentions.mentions.length ? <>
      <h2>Mentions</h2>
      <p className="shared-muted-copy">Files this item names, in its text or through its commits, and the issues and pull requests it refers to (such as #12).</p>
      {list(mentions.mentions)}
    </> : null}
  </section>;
}
