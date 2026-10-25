import { configureWebApi } from "./sharedApi";

/**
 * What the host that serves the web app tells it when it starts (V0.4.14), in `/config.json` beside the
 * page. One built app then serves any server: the address of the API and the product's name are read here,
 * not compiled in. A host that serves no such file, or a development server, leaves the build's own values.
 */
export type RuntimeConfig = {
  /** The API's address; empty means the page's own host. Absent keeps the address the app was built with. */
  apiUrl?: string;
  /** The name shown in the browser's tab. */
  productName?: string;
};

const MAX_NAME_CHARS = 60;

/** Keeps only what is valid; anything else in the file is ignored. */
export function parseRuntimeConfig(value: unknown): RuntimeConfig {
  if (typeof value !== "object" || value === null) return {};
  const input = value as Record<string, unknown>;
  const config: RuntimeConfig = {};
  if (typeof input.apiUrl === "string") {
    const apiUrl = normalizeApiUrl(input.apiUrl);
    if (apiUrl !== null) config.apiUrl = apiUrl;
  }
  if (typeof input.productName === "string") {
    // A name for a tab: one line, no control characters, not empty.
    const name = input.productName.replace(/\s+/g, " ").trim();
    // eslint-disable-next-line no-control-regex
    if (name.length > 0 && name.length <= MAX_NAME_CHARS && !/[\u0000-\u001f\u007f]/.test(name)) config.productName = name;
  }
  return config;
}

/** An `http(s)` address without a login, a query or a fragment, or an empty one; otherwise `null`. */
function normalizeApiUrl(text: string): string | null {
  const trimmed = text.trim();
  if (trimmed === "") return "";
  let url: URL;
  try {
    url = new URL(trimmed);
  } catch {
    return null;
  }
  if (url.protocol !== "https:" && url.protocol !== "http:") return null;
  if (url.username || url.password || url.search || url.hash) return null;
  return `${url.origin}${url.pathname === "/" ? "" : url.pathname.replace(/\/$/, "")}`;
}

/** Reads `/config.json`; a missing, unreadable or invalid file is an empty configuration. */
export async function loadRuntimeConfig(fetcher: typeof fetch = fetch): Promise<RuntimeConfig> {
  try {
    const response = await fetcher("/config.json", { cache: "no-store", credentials: "omit", headers: { Accept: "application/json" } });
    if (!response.ok) return {};
    return parseRuntimeConfig(await response.json());
  } catch {
    // Not served (a development server answers with the page), or not JSON.
    return {};
  }
}

/** Points the client at the configured server and names the page. */
export function applyRuntimeConfig(config: RuntimeConfig, page: Document = document) {
  if (config.apiUrl !== undefined) configureWebApi(config.apiUrl);
  if (config.productName) page.title = config.productName;
}
