import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

vi.mock("../lib/sharedApi", () => ({ sharedApiUrl: "https://memo.example.com" }));

import { OpenInAppLink } from "../components/OpenInAppLink";
import { APP_AREAS, appLinkFor, isAppPath } from "../lib/appLink";
import { parseRoute } from "../web/router";

const rust = import.meta.glob("../../src-tauri/src/deeplink.rs", { query: "?raw", import: "default", eager: true }) as Record<string, string>;
const deeplinkRs = Object.values(rust)[0];

afterEach(cleanup);

describe("the paths a link may carry (the same cases as the shell's tests)", () => {
  it("takes the client's own paths", () => {
    for (const good of ["/dashboard", "/profile", "/notifications", "/workspaces", "/workspaces/w1", "/workspaces/w1/evidence?q=a%20b#top", "/workspaces/w1/artifacts/a1", "/system/jobs"]) {
      expect(isAppPath(good), good).toBe(true);
    }
  });

  it("refuses everything that leaves the client", () => {
    for (const bad of ["", "/", "dashboard", "//evil.example.com/workspaces", "/\\evil.example.com", "https://evil.example.com/dashboard", "javascript:alert(1)", "/login", "/v1/system/backups", "/workspaces/../../etc/passwd", "/workspaces/%2e%2e/system", "/workspaces/w1%2Fx", "/workspaces/w1\u0000", "/workspaces/%zz", "/dashboard\n/x"]) {
      expect(isAppPath(bad), JSON.stringify(bad)).toBe(false);
    }
    expect(isAppPath(`/workspaces/${"a".repeat(2100)}`)).toBe(false);
  });

  it("names the same areas as the shell and the client's router", () => {
    const listed = /const APP_AREAS: \[&str; \d+\] = \[([^\]]+)\]/.exec(deeplinkRs)?.[1].match(/"([a-z]+)"/g)?.map((name) => name.replace(/"/g, ""));
    expect(listed).toEqual([...APP_AREAS]);
    for (const area of APP_AREAS) expect(parseRoute(`/${area}`).name, area).not.toBe("not-found");
    expect(parseRoute("/login").name).toBe("not-found");
  });
});

describe("a link from a page", () => {
  it("carries the server and the page, both encoded", () => {
    expect(appLinkFor("https://memo.example.com/some/path", "/workspaces/w1/evidence?q=a b")).toBe(
      "repomemo://open?server=https%3A%2F%2Fmemo.example.com&path=%2Fworkspaces%2Fw1%2Fevidence%3Fq%3Da%20b",
    );
  });

  it("is made for nothing else: no foreign path, no foreign scheme, no non-address", () => {
    expect(appLinkFor("https://memo.example.com", "https://evil.example.com")).toBeNull();
    expect(appLinkFor("https://memo.example.com", "/login")).toBeNull();
    expect(appLinkFor("ftp://memo.example.com", "/dashboard")).toBeNull();
    expect(appLinkFor("not an address", "/dashboard")).toBeNull();
  });
});

describe("Open in the app", () => {
  it("is a link in a browser, for the server the page talks to", () => {
    render(<OpenInAppLink path="/notifications" />);
    const link = screen.getByRole("link", { name: "Open in the app" });
    expect(link.getAttribute("href")).toBe("repomemo://open?server=https%3A%2F%2Fmemo.example.com&path=%2Fnotifications");
    expect(link.getAttribute("title")).toMatch(/desktop app/);
  });

  it("is nothing for a path that is not a page of the client", () => {
    render(<OpenInAppLink path="/login" />);
    expect(screen.queryByRole("link")).toBeNull();
  });
});
