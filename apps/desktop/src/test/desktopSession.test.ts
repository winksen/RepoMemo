import { beforeEach, describe, expect, it, vi } from "vitest";

// The shell's credential store, in memory.
const stored = vi.hoisted(() => ({ token: null as string | null }));
vi.mock("../lib/desktop", () => ({
  isDesktopRuntime: true,
  sessionStore: {
    save: vi.fn(async (token: string) => { stored.token = token; }),
    load: vi.fn(async () => stored.token),
    clear: vi.fn(async () => { stored.token = null; }),
  },
}));

import { sessionStore } from "../lib/desktop";
import {
  clearSharedSession, configureDesktopApi, loginSharedUser, refreshSharedSession, sharedApiUrl, SHARED_SESSION_STORAGE_KEY,
} from "../lib/sharedApi";

const SERVER = "https://repomemo.example.com";

function json(body: unknown, status = 200) {
  return new Response(JSON.stringify(body), { status, headers: { "Content-Type": "application/json" } });
}

const tokens = (refresh: string) => ({ access_token: "access-1", refresh_token: refresh, token_type: "Bearer", expires_in: 3600, user: { id: "u1", email: "a@example.com", display_name: "A" } });

const desktopFetch = vi.fn<typeof fetch>();
const webFetch = vi.spyOn(globalThis, "fetch");

beforeEach(() => {
  stored.token = null;
  window.localStorage.clear();
  window.sessionStorage.clear();
  desktopFetch.mockReset();
  webFetch.mockReset();
  vi.mocked(sessionStore.save).mockClear();
  vi.mocked(sessionStore.clear).mockClear();
  configureDesktopApi(`${SERVER}/`, desktopFetch);
});

const header = (call: Parameters<typeof fetch>, name: string) => new Headers(call[1]?.headers).get(name);

describe("a session in the desktop app", () => {
  it("calls the chosen server through the shell's client, never the browser's, and says it is the desktop app", async () => {
    desktopFetch.mockResolvedValue(json(tokens("refresh-1")));
    await loginSharedUser({ email: "a@example.com", password: "correct horse battery" });
    expect(sharedApiUrl).toBe(SERVER);
    expect(String(desktopFetch.mock.calls[0][0])).toBe(`${SERVER}/v1/auth/login`);
    expect(header(desktopFetch.mock.calls[0], "X-RepoMemo-Client")).toBe("desktop");
    expect(webFetch).not.toHaveBeenCalled();
  });

  it("keeps the refresh token in the credential store, and the page's storage holds only the access token", async () => {
    desktopFetch.mockResolvedValue(json(tokens("refresh-1")));
    await loginSharedUser({ email: "a@example.com", password: "correct horse battery" });
    expect(sessionStore.save).toHaveBeenCalledWith("refresh-1");
    expect(stored.token).toBe("refresh-1");
    expect(window.sessionStorage.getItem(SHARED_SESSION_STORAGE_KEY)).toBe("access-1");
    const everything = [...Object.keys(window.localStorage), ...Object.keys(window.sessionStorage)].map((key) => `${key}=${window.localStorage.getItem(key) ?? window.sessionStorage.getItem(key)}`).join(";");
    expect(everything).not.toContain("refresh-1");
  });

  it("renews from the credential store with the token in the body, and keeps the rotated one", async () => {
    stored.token = "refresh-1";
    desktopFetch.mockResolvedValue(json(tokens("refresh-2")));
    expect(await refreshSharedSession()).toBe("access-1");
    const call = desktopFetch.mock.calls[0];
    expect(String(call[0])).toBe(`${SERVER}/v1/auth/refresh`);
    expect(JSON.parse(String(call[1]?.body))).toEqual({ refresh_token: "refresh-1" });
    expect(stored.token).toBe("refresh-2");
  });

  it("does not call the server when nothing is stored", async () => {
    expect(await refreshSharedSession()).toBeNull();
    expect(desktopFetch).not.toHaveBeenCalled();
  });

  it("forgets the stored token when the server refuses it, but not when the network fails", async () => {
    stored.token = "refresh-1";
    desktopFetch.mockRejectedValueOnce(new Error("offline"));
    expect(await refreshSharedSession()).toBeNull();
    expect(stored.token).toBe("refresh-1");

    desktopFetch.mockResolvedValueOnce(json({ error: { message: "no" } }, 401));
    expect(await refreshSharedSession()).toBeNull();
    await vi.waitFor(() => expect(stored.token).toBeNull());
  });

  it("signs out by telling the server which session to end, then forgetting it", async () => {
    stored.token = "refresh-1";
    window.localStorage.setItem("repomemo.shared.session", "1");
    window.sessionStorage.setItem(SHARED_SESSION_STORAGE_KEY, "access-1");
    desktopFetch.mockResolvedValue(json({}));
    clearSharedSession();
    expect(window.sessionStorage.getItem(SHARED_SESSION_STORAGE_KEY)).toBeNull();
    await vi.waitFor(() => expect(desktopFetch).toHaveBeenCalled());
    const call = desktopFetch.mock.calls[0];
    expect(String(call[0])).toBe(`${SERVER}/v1/auth/logout`);
    expect(JSON.parse(String(call[1]?.body))).toEqual({ refresh_token: "refresh-1" });
    await vi.waitFor(() => expect(stored.token).toBeNull());
  });
});
