import { useEffect, useState } from "react";
import { getSharedArtifactCodeOwners, requestSharedArtifactReview } from "../lib/sharedApi";
import type { CodeOwnerMatch } from "../types";
import { apiMessage } from "../web/format";
import { Button } from "./ui/button";
import { showToast } from "./ui/toast";

/**
 * The owners `CODEOWNERS` names for a repository file (V0.3.15). An owner written as an address that is a member of
 * the workspace shows as that member and can be asked to review the file (a task is assigned to them); a `@handle`,
 * a team or an address nobody has stays a plain hint, as before. Without an answer from the server every owner is a hint.
 */
export function CodeOwnersHint({ accessToken, artifactId, canWrite, hints }: { accessToken: string; artifactId: string; canWrite: boolean; hints: string[] }) {
  const [matches, setMatches] = useState<CodeOwnerMatch[] | null>(null);
  const [asking, setAsking] = useState<string | null>(null);
  const [asked, setAsked] = useState<ReadonlySet<string>>(new Set());
  const key = hints.join("\n");

  useEffect(() => {
    if (!hints.length) return;
    let cancelled = false;
    getSharedArtifactCodeOwners(accessToken, artifactId)
      .then((found) => { if (!cancelled) setMatches(found); })
      .catch(() => { if (!cancelled) setMatches(null); });
    return () => { cancelled = true; };
  }, [accessToken, artifactId, key]);

  async function ask(owner: CodeOwnerMatch) {
    if (!owner.member) return;
    setAsking(owner.member.id);
    try {
      await requestSharedArtifactReview(accessToken, artifactId, owner.member.id);
      setAsked((current) => new Set(current).add(owner.member!.id));
      showToast("success", `${owner.member.display_name} was asked to review this file.`);
    } catch (error) {
      showToast("error", apiMessage(error, "The request could not be made."));
    } finally {
      setAsking(null);
    }
  }

  if (!hints.length) return null;
  const entries: CodeOwnerMatch[] = matches ?? hints.map((owner) => ({ owner, member: null }));
  return <> · CODEOWNERS suggests {entries.map((entry, index) => <span className="rm-code-owner" key={`${entry.owner}-${index}`}>
    {index > 0 ? ", " : ""}
    {entry.member
      ? <><strong title={entry.owner}>{entry.member.display_name}</strong> <small>({entry.owner})</small>{canWrite
        ? <> <Button aria-label={`Ask ${entry.member.display_name} to review this file`} busy={asking === entry.member.id} disabled={asking !== null || asked.has(entry.member.id)} onClick={() => void ask(entry)} type="button" variant="secondary">{asked.has(entry.member.id) ? "Asked" : "Ask to review"}</Button></>
        : null}</>
      : <span>{entry.owner}</span>}
  </span>)}</>;
}
