/**
 * Offers a file to the person from data the page holds (V0.4.52). Every download in the client goes through here, so how a file is
 * saved is decided in one place: a browser downloads it through an anchor, and the desktop app, whose webview may save nothing or
 * save without asking, installs a saver that opens a native save dialog (`setFileSaver`).
 *
 * Answers whether the file was saved: `false` when the person cancelled the dialog.
 */
export type FileSaver = (blob: Blob, filename: string) => Promise<boolean>;

/** The browser's own way: an anchor with `download`, which the browser answers with its dialog or its downloads folder. */
const browserSaver: FileSaver = async (blob, filename) => {
  const url = URL.createObjectURL(blob);
  const link = document.createElement("a");
  link.href = url;
  link.download = filename;
  document.body.appendChild(link);
  link.click();
  link.remove();
  window.setTimeout(() => URL.revokeObjectURL(url), 10_000);
  return true;
};

let saver: FileSaver = browserSaver;

/** Replaces the way files are saved; `null` brings back the browser's. */
export function setFileSaver(next: FileSaver | null) {
  saver = next ?? browserSaver;
}

export function saveBlob(blob: Blob, filename: string): Promise<boolean> {
  return saver(blob, filename);
}
