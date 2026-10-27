import { beforeEach, describe, expect, it, vi } from "vitest";
import { clearSharedSession, loginSharedUser, refreshSharedSession, SHARED_SESSION_STORAGE_KEY } from "../lib/sharedApi";

const MARKER = "repomemo.shared.session";
const LEGACY = "repomemo.shared.refresh-token";

function json(body: unknown, status = 200) {
  return new Response(JSON.stringify(body), { status, headers: { "Content-Type": "application/json" } });
}

const tokens = { access_token: "access-1", refresh_token: "", token_type: "Bearer", expires_in: 3600, user: { id: "u1", email: "a@example.com", display_name: "A" } };

beforeEach(() => {
  window.localStorage.clear();
  window.sessionStorage.clear();
});

describe("web session", () => {
  it("does not call the server without a session to renew", async () => {
    const fetchMock = vi.spyOn(globalThis, "fetch");
    expect(await refreshSharedSession()).toBeNull();
    expect(fetchMock).not.toHaveBeenCalled();
  });

  it("renews with the cookie: credentials are sent and no token is in the body", async () => {
    window.localStorage.setItem(MARKER, "1");
    const fetchMock = vi.spyOn(globalThis, "fetch").mockResolvedValue(json(tokens));
    expect(await refreshSharedSession()).toBe("access-1");
    const [url, init] = fetchMock.mock.calls[0];
    expect(String(url)).toContain("/v1/auth/refresh");
    expect(init?.credentials).toBe("include");
    expect(init?.body).toBe("{}");
    expect(new Headers(init?.headers).get("X-RepoMemo-Client")).toBe("web");
    expect(window.sessionStorage.getItem(SHARED_SESSION_STORAGE_KEY)).toBe("access-1");
  });

  it("exchanges a refresh token kept by an earlier version once, then forgets it", async () => {
    window.localStorage.setItem(LEGACY, "old-token");
    const fetchMock = vi.spyOn(globalThis, "fetch").mockResolvedValue(json(tokens));
    await refreshSharedSession();
    expect(JSON.parse(String(fetchMock.mock.calls[0][1]?.body))).toEqual({ refresh_token: "old-token" });
    expect(window.localStorage.getItem(LEGACY)).toBeNull();
    expect(window.localStorage.getItem(MARKER)).toBe("1");
  });

  it("ends the local session when the server refuses the renewal", async () => {
    window.localStorage.setItem(MARKER, "1");
    window.sessionStorage.setItem(SHARED_SESSION_STORAGE_KEY, "stale");
    vi.spyOn(globalThis, "fetch").mockResolvedValue(json({ error: { message: "no" } }, 401));
    expect(await refreshSharedSession()).toBeNull();
    expect(window.localStorage.getItem(MARKER)).toBeNull();
    expect(window.sessionStorage.getItem(SHARED_SESSION_STORAGE_KEY)).toBeNull();
  });

  it("renews inside the lock the tabs of a browser share, so two tabs never present one cookie", async () => {
    window.localStorage.setItem(MARKER, "1");
    const request = vi.fn(async (_name: string, work: () => Promise<unknown>) => work());
    Object.defineProperty(navigator, "locks", { value: { request }, configurable: true });
    try {
      vi.spyOn(globalThis, "fetch").mockResolvedValue(json(tokens));
      expect(await refreshSharedSession()).toBe("access-1");
      expect(request).toHaveBeenCalledTimes(1);
      expect(request.mock.calls[0][0]).toBe("repomemo-refresh");
    } finally {
      Reflect.deleteProperty(navigator, "locks");
    }
  });

  it("asks again a moment later when the server says another tab renewed, and keeps the session", async () => {
    window.localStorage.setItem(MARKER, "1");
    const fetchMock = vi.spyOn(globalThis, "fetch")
      .mockResolvedValueOnce(json({ error: { message: "renewed elsewhere" } }, 409))
      .mockResolvedValueOnce(json(tokens));
    expect(await refreshSharedSession()).toBe("access-1");
    expect(fetchMock).toHaveBeenCalledTimes(2);
    expect(window.localStorage.getItem(MARKER)).toBe("1");
  });

  it("gives up after two retries without ending the session", async () => {
    window.localStorage.setItem(MARKER, "1");
    const fetchMock = vi.spyOn(globalThis, "fetch").mockImplementation(async () => json({ error: { message: "renewed elsewhere" } }, 409));
    expect(await refreshSharedSession()).toBeNull();
    expect(fetchMock).toHaveBeenCalledTimes(3);
    expect(window.localStorage.getItem(MARKER)).toBe("1");
  });

  it("keeps the session when the network fails", async () => {
    window.localStorage.setItem(MARKER, "1");
    vi.spyOn(globalThis, "fetch").mockRejectedValue(new TypeError("offline"));
    expect(await refreshSharedSession()).toBeNull();
    expect(window.localStorage.getItem(MARKER)).toBe("1");
  });

  it("stores only the access token and a marker after sign-in", async () => {
    vi.spyOn(globalThis, "fetch").mockResolvedValue(json(tokens));
    await loginSharedUser({ email: "a@example.com", password: "a-long-enough-password" });
    expect(window.sessionStorage.getItem(SHARED_SESSION_STORAGE_KEY)).toBe("access-1");
    expect(window.localStorage.getItem(MARKER)).toBe("1");
    expect(window.localStorage.getItem(LEGACY)).toBeNull();
  });

  it("signs out on the server and forgets everything locally", async () => {
    window.localStorage.setItem(MARKER, "1");
    window.sessionStorage.setItem(SHARED_SESSION_STORAGE_KEY, "access-1");
    const fetchMock = vi.spyOn(globalThis, "fetch").mockResolvedValue(new Response(null, { status: 204 }));
    clearSharedSession();
    await vi.waitFor(() => expect(fetchMock).toHaveBeenCalled());
    expect(String(fetchMock.mock.calls[0][0])).toContain("/v1/auth/logout");
    expect(window.localStorage.getItem(MARKER)).toBeNull();
    expect(window.sessionStorage.getItem(SHARED_SESSION_STORAGE_KEY)).toBeNull();
  });
});
