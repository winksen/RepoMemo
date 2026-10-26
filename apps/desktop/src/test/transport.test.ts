import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

// The shell's HTTP client, in memory.
const plugin = vi.hoisted(() => ({ fetch: vi.fn() }));
vi.mock("@tauri-apps/plugin-http", () => plugin);
vi.mock("../lib/desktop", () => ({
  isDesktopRuntime: true,
  sessionStore: { save: vi.fn(async () => undefined), load: vi.fn(async () => null), clear: vi.fn(async () => undefined) },
}));

const health = () => new Response(JSON.stringify({ service: "repomemo-server", status: "ok", authentication: "jwt" }), { status: 200, headers: { "Content-Type": "application/json" } });

beforeEach(() => {
  vi.resetModules();
  plugin.fetch.mockReset();
});
afterEach(() => vi.restoreAllMocks());

describe("the one transport", () => {
  it("is the browser's own fetch in a browser, looked up when it is called", async () => {
    const { browserTransport } = await import("../lib/transport");
    const page = vi.spyOn(globalThis, "fetch").mockResolvedValue(health());
    await browserTransport("https://api.example.com/health", { method: "GET" });
    expect(page).toHaveBeenCalledWith("https://api.example.com/health", { method: "GET" });
  });

  it("is the shell's client in the app, which follows no redirect", async () => {
    plugin.fetch.mockResolvedValue(health());
    const { shellTransport } = await import("../lib/transport");
    const transport = await shellTransport();
    await transport("https://repomemo.example.com/health", { method: "POST", headers: { A: "b" } });
    expect(plugin.fetch).toHaveBeenCalledWith("https://repomemo.example.com/health", { method: "POST", headers: { A: "b" }, maxRedirections: 0 });
  });

  it("carries every request of the client: the page's fetch for a web client, the shell's for the app, never both", async () => {
    const page = vi.spyOn(globalThis, "fetch").mockImplementation(async () => health());
    const web = await import("../lib/sharedApi");
    web.configureWebApi("https://api.example.com");
    await web.getSharedHealth();
    expect(page).toHaveBeenCalledTimes(1);
    expect(String(page.mock.calls[0][0])).toBe("https://api.example.com/health");
    expect(plugin.fetch).not.toHaveBeenCalled();

    page.mockClear();
    plugin.fetch.mockImplementation(async () => health());
    const { pointAtServer } = await import("../lib/desktopConnection");
    await pointAtServer("https://repomemo.example.com");
    await web.getSharedHealth();
    expect(plugin.fetch).toHaveBeenCalledTimes(1);
    expect(String(plugin.fetch.mock.calls[0][0])).toBe("https://repomemo.example.com/health");
    expect(plugin.fetch.mock.calls[0][1]).toMatchObject({ maxRedirections: 0 });
    expect(page).not.toHaveBeenCalled();
  });
});

describe("what the window can do", () => {
  it("is asked as capabilities, which the app and a browser answer differently", async () => {
    const app = (await import("../lib/platform")).platform;
    expect(app).toEqual({ opensTabs: false, choosesServer: true, installedApp: true, nativeNotices: true, opensInApp: false });
    vi.resetModules();
    vi.doMock("../lib/desktop", () => ({ isDesktopRuntime: false }));
    const browser = (await import("../lib/platform")).platform;
    expect(browser).toEqual({ opensTabs: true, choosesServer: false, installedApp: false, nativeNotices: false, opensInApp: true });
  });
});

describe("a server with its own certificate authority (V0.4.55)", () => {
  const relay = { origin: "https://memo.example.com", proxy: "http://127.0.0.1:51234", token: "secret-1" };

  it("is called through the relay with the run's secret, the page still using the server's own address", async () => {
    const { throughRelay } = await import("../lib/transport");
    const sent = throughRelay(relay, "https://memo.example.com/v1/auth/login?x=1", { headers: { "X-RepoMemo-Client": "desktop" } });
    expect(sent.url).toBe("http://127.0.0.1:51234/v1/auth/login?x=1");
    expect(sent.headers.get("x-repomemo-route")).toBe("secret-1");
    expect(sent.headers.get("x-repomemo-client")).toBe("desktop");
    expect(throughRelay(relay, new URL("https://memo.example.com/health")).url).toBe("http://127.0.0.1:51234/health");
    expect(throughRelay(relay, "https://memo.example.com").url).toBe("http://127.0.0.1:51234");
  });

  it("sends nothing else to the relay, and gives it nothing", async () => {
    const { throughRelay } = await import("../lib/transport");
    for (const other of ["https://other.example.com/v1/x", "https://memo.example.com.evil.example.com/v1/x", "https://memo.example.comx/v1/x", "http://127.0.0.1:51234/v1/x"]) {
      const sent = throughRelay(relay, other);
      expect(sent.url, other).toBe(other);
      expect(sent.headers.get("x-repomemo-route"), other).toBeNull();
    }
  });

  it("goes through the shell's client by way of the relay, and the whole client with it", async () => {
    plugin.fetch.mockImplementation(async () => health());
    vi.resetModules();
    vi.doMock("../lib/desktop", () => ({
      isDesktopRuntime: true,
      desktopServer: { route: async () => ({ proxy: relay.proxy, token: relay.token }) },
      sessionStore: { save: async () => undefined, load: async () => null, clear: async () => undefined },
    }));
    const { pointAtServer } = await import("../lib/desktopConnection");
    const api = await import("../lib/sharedApi");
    await pointAtServer(relay.origin);
    await api.getSharedHealth();
    expect(String(plugin.fetch.mock.calls[0][0])).toBe("http://127.0.0.1:51234/health");
    expect(new Headers(plugin.fetch.mock.calls[0][1]?.headers).get("x-repomemo-route")).toBe("secret-1");
    expect(plugin.fetch.mock.calls[0][1]).toMatchObject({ maxRedirections: 0 });
  });
});
