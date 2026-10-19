import { useEffect, useState } from "react";

/** A browser address for a file held in memory, made when the file arrives and released when it goes or the page is left. */
export function useObjectUrl(file: Blob | null | undefined, type = "application/pdf"): string | null {
  const [url, setUrl] = useState<string | null>(null);
  useEffect(() => {
    if (!file) { setUrl(null); return undefined; }
    const next = URL.createObjectURL(new Blob([file], { type }));
    setUrl(next);
    return () => { URL.revokeObjectURL(next); };
  }, [file, type]);
  return url;
}
