import { useEffect, useState } from "react";
import { downloadSharedArtifactFile } from "../lib/sharedApi";

/** Image types that can be opened on their own: they cannot run scripts. */
const RASTER_TYPE = /^image\/(png|jpeg|gif|webp|bmp)$/;

/**
 * How an image can be found (V0.6.21), from what is indexed for it: a description of the picture, or only its name and
 * path. Nothing is claimed about the picture's content unless something described it.
 */
export function imageSearchability(chunks: { heading_path: string | null }[]): { state: "pending" | "described" | "metadata"; text: string } {
  if (chunks.some((chunk) => chunk.heading_path === "Visual description")) {
    return { state: "described", text: "Searchable by an AI description of what the picture shows. The description is generated, so check it against the image." };
  }
  if (chunks.length > 0) {
    return { state: "metadata", text: "Searchable by its name and path only. Nothing in the picture itself is searchable; an administrator can add an image AI provider in Settings to describe it." };
  }
  return { state: "pending", text: "Not indexed yet, so it is not searchable." };
}

/**
 * The stored image, fetched with the user's session and shown at its natural size up to the panel width.
 * A blob URL belongs to this app's origin, so only raster images are offered in a tab of their own: an SVG
 * (or a file served with any other type) opened that way would run its scripts with the user's session.
 * Inside `<img>` an SVG never runs scripts.
 */
export function ImagePreview({ accessToken, artifactId, title }: { accessToken: string; artifactId: string; title: string }) {
  const [image, setImage] = useState<{ url: string; openable: boolean } | null>(null);
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    let cancelled = false;
    let objectUrl: string | null = null;
    setImage(null);
    setFailed(false);
    downloadSharedArtifactFile(accessToken, artifactId)
      .then((blob) => {
        if (cancelled) return;
        const openable = RASTER_TYPE.test(blob.type);
        objectUrl = URL.createObjectURL(blob);
        setImage({ url: objectUrl, openable });
      })
      .catch(() => { if (!cancelled) setFailed(true); });
    return () => { cancelled = true; if (objectUrl) URL.revokeObjectURL(objectUrl); };
  }, [accessToken, artifactId]);

  if (failed) return <p className="shared-muted-copy">The image could not be loaded.</p>;
  if (!image) return <div aria-busy="true" className="shared-image-preview loading" />;
  if (!image.openable) return <div className="shared-image-preview"><img alt={title} src={image.url} /></div>;
  return <a className="shared-image-preview" href={image.url} rel="noopener noreferrer" target="_blank" title="Open full size"><img alt={title} src={image.url} /></a>;
}
