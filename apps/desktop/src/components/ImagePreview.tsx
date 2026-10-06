import { useEffect, useState } from "react";
import { downloadSharedArtifactFile } from "../lib/sharedApi";

/** The stored image, fetched with the user's session and shown at its natural size up to the panel width. */
export function ImagePreview({ accessToken, artifactId, title }: { accessToken: string; artifactId: string; title: string }) {
  const [url, setUrl] = useState<string | null>(null);
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    let cancelled = false;
    let objectUrl: string | null = null;
    setUrl(null);
    setFailed(false);
    downloadSharedArtifactFile(accessToken, artifactId)
      .then((blob) => {
        if (cancelled) return;
        objectUrl = URL.createObjectURL(blob);
        setUrl(objectUrl);
      })
      .catch(() => { if (!cancelled) setFailed(true); });
    return () => { cancelled = true; if (objectUrl) URL.revokeObjectURL(objectUrl); };
  }, [accessToken, artifactId]);

  if (failed) return <p className="shared-muted-copy">The image could not be loaded.</p>;
  if (!url) return <div aria-busy="true" className="shared-image-preview loading" />;
  return <a className="shared-image-preview" href={url} rel="noreferrer" target="_blank" title="Open full size"><img alt={title} src={url} /></a>;
}
