import { useEffect, useRef, useState } from "react";
import { indexSharedArtifact, waitForSharedJob } from "../lib/sharedApi";
import type { IndexingJobStatus } from "../types";
import { apiMessage } from "../web/format";
import { Button } from "./ui/button";

/** A scoped retry keeps its accepted job and terminal result visible. */
export function RetryIndexButton({ accessToken, artifactId, onFinished }: { accessToken: string; artifactId: string; onFinished: () => void }) {
  const [job, setJob] = useState<IndexingJobStatus | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const mounted = useRef(true);
  useEffect(() => () => { mounted.current = false; }, []);

  async function retry() {
    setBusy(true); setError(null);
    try {
      const accepted = await indexSharedArtifact(accessToken, artifactId);
      if (!mounted.current) return;
      setJob(accepted);
      const finished = await waitForSharedJob(accessToken, accepted, () => !mounted.current);
      if (!mounted.current) return;
      setJob(finished);
      if (finished.status !== "completed") setError(finished.error_message ?? "Indexing did not complete.");
      onFinished();
    } catch (requestError) {
      if (mounted.current) setError(apiMessage(requestError, "This item could not be queued for indexing."));
    } finally {
      if (mounted.current) setBusy(false);
    }
  }

  return <div className="shared-index-retry">
    <Button busy={busy} onClick={() => void retry()} type="button" variant="secondary">Retry indexing</Button>
    {job ? <span role="status">Index job {job.id.slice(0, 8)}: {job.status}</span> : null}
    {error ? <span role="alert">{error}</span> : null}
  </div>;
}
