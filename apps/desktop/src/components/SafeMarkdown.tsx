import type { ReactNode } from "react";
import ReactMarkdown, { type Components } from "react-markdown";
import remarkGfm from "remark-gfm";

/**
 * Markdown from outside the page's own code: AI answers, notes, memory cards,
 * repository READMEs. Two rules keep it from phoning home or misleading
 * (review M5, L9):
 *
 * - **No images.** A remote image is a request the page makes for whoever
 *   wrote the Markdown, and its address can carry what the model was shown.
 *   Images become a note with their text and address.
 * - **Links say where they go.** The target host is shown next to the text and
 *   the link opens in a new tab without an opener or a referrer.
 */

/**
 * The host of an address that leaves this page's own origin, or null for anything else. The address is read
 * the way a browser reads it from this page, so `//host/path` and `\\host\path` (both go to another host
 * with this page's scheme) are external, while `/v1/files`, `#section` and `mailto:` are not (V0.2.147).
 */
export function externalHost(address: string | undefined): string | null {
  if (!address) return null;
  try {
    const base = typeof window === "undefined" ? undefined : window.location.href;
    const url = new URL(address, base);
    if (url.protocol !== "http:" && url.protocol !== "https:") return null;
    return base && url.origin === new URL(base).origin ? null : url.host;
  } catch {
    return null;
  }
}

function plainText(children: ReactNode): string {
  if (typeof children === "string") return children;
  if (typeof children === "number") return String(children);
  if (Array.isArray(children)) return children.map(plainText).join("");
  return "";
}

const components: Components = {
  img({ alt, src }) {
    const target = typeof src === "string" ? src : "";
    return (
      <span className="shared-markdown-blocked-image" title={target || undefined}>
        Image not loaded{alt ? `: ${alt}` : ""}{target ? ` (${externalHost(target) ?? target})` : ""}
      </span>
    );
  },
  a({ href, children }) {
    const host = externalHost(href);
    if (!host) {
      // Anchors inside the page, mail links and anything else that is not a web address.
      return <a href={href} rel="noopener noreferrer">{children}</a>;
    }
    // Only name the host again when the text does not already show it.
    const showsTarget = plainText(children).includes(host);
    return (
      <>
        <a href={href} rel="noopener noreferrer nofollow" target="_blank" title={href}>{children}</a>
        {showsTarget ? null : <span className="shared-markdown-link-target"> ({host})</span>}
      </>
    );
  },
};

export function SafeMarkdown({ children }: { children: string }) {
  return <ReactMarkdown components={components} remarkPlugins={[remarkGfm]}>{children}</ReactMarkdown>;
}
