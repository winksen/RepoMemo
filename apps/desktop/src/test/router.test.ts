import { describe, expect, it } from "vitest";
import { assistantEntryPath, assistantPath, legacyRedirect, parseRoute, routePath } from "../web/router";

describe("parseRoute", () => {
  it("names the pages", () => {
    expect(parseRoute("/")).toEqual({ name: "dashboard" });
    expect(parseRoute("/spaces")).toEqual({ name: "dashboard" });
    expect(parseRoute("/orgs/o1")).toEqual({ name: "organization", organizationId: "o1", section: "overview" });
    expect(parseRoute("/orgs/o1/workspaces")).toEqual({ name: "organization", organizationId: "o1", section: "workspaces" });
    expect(parseRoute("/orgs/o1/administration")).toEqual({ name: "organization", organizationId: "o1", section: "administration" });
    expect(parseRoute("/orgs/new")).toEqual({ name: "new-organization" });
    expect(parseRoute("/orgs/new/workspaces")).toEqual({ name: "not-found", workspaceId: null });
    expect(parseRoute("/orgs/o1/x")).toEqual({ name: "not-found", workspaceId: null });
    expect(parseRoute("/dashboard")).toEqual({ name: "dashboard" });
    expect(parseRoute("/orgs")).toEqual({ name: "orgs" });
    expect(parseRoute("/assistant")).toEqual({ name: "assistant", workspaceId: null });
    expect(parseRoute("/assistant/w%201")).toEqual({ name: "assistant", workspaceId: "w 1" });
    expect(parseRoute("/assistant/w1/x")).toEqual({ name: "not-found", workspaceId: null });
    expect(parseRoute("/profile")).toEqual({ name: "profile" });
    expect(parseRoute("/notifications")).toEqual({ name: "notifications" });
  });

  it("reads workspace sections, artifacts and memory cards", () => {
    expect(parseRoute("/spaces/w1")).toEqual({ name: "workspace", workspaceId: "w1", section: "overview", landing: true });
    expect(parseRoute("/spaces/w1/overview")).toEqual({ name: "workspace", workspaceId: "w1", section: "overview" });
    expect(parseRoute("/spaces/w1/memory")).toEqual({ name: "workspace", workspaceId: "w1", section: "memory" });
    expect(parseRoute("/spaces/w%201/artifacts/a%2F1")).toEqual({ name: "artifact", workspaceId: "w 1", artifactId: "a/1" });
    expect(parseRoute("/spaces/w1/memory-cards/c1")).toEqual({ name: "memory-card", workspaceId: "w1", cardId: "c1" });
  });

  it("reads the System area, with overview as its front page", () => {
    expect(parseRoute("/system")).toEqual({ name: "system", section: "overview" });
    expect(parseRoute("/system/logs")).toEqual({ name: "system", section: "logs" });
    expect(parseRoute("/system/nothing")).toEqual({ name: "not-found", workspaceId: null });
    expect(parseRoute("/system/logs/extra")).toEqual({ name: "not-found", workspaceId: null });
  });

  it("keeps the workspace of an address that names nothing", () => {
    expect(parseRoute("/spaces/w1/unknown")).toEqual({ name: "not-found", workspaceId: "w1" });
    expect(parseRoute("/spaces/w1/artifacts")).toEqual({ name: "not-found", workspaceId: "w1" });
    expect(parseRoute("/elsewhere")).toEqual({ name: "not-found", workspaceId: null });
  });

  it("is the inverse of routePath", () => {
    for (const path of ["/orgs", "/orgs/o1", "/orgs/o1/workspaces", "/orgs/o1/administration", "/orgs/new", "/dashboard", "/system/audit", "/spaces/w%201/tasks", "/spaces/w1/artifacts/a%2F1", "/spaces/w1/memory-cards/c1"]) {
      const route = parseRoute(path);
      if (route.name === "not-found") throw new Error(path);
      expect(routePath(route)).toBe(path);
    }
  });

  it("moves the old workspace addresses to the current ones", () => {
    expect(legacyRedirect("/workspaces/w1/memory", "?q=a")).toBe("/spaces/w1/memory?q=a");
    expect(legacyRedirect("/workspaces", "?organization=o%201")).toBe("/orgs/o%201");
    expect(legacyRedirect("/workspaces", "?organization=o1&createWorkspace=1")).toBe("/orgs/o1");
    expect(legacyRedirect("/workspaces", "?createOrganization=1")).toBe("/orgs/new");
    expect(legacyRedirect("/workspaces", "")).toBe("/spaces");
    expect(legacyRedirect("/spaces/w1", "")).toBeNull();
    expect(parseRoute("/workspaces/w1/memory")).toEqual({ name: "workspace", workspaceId: "w1", section: "memory" });
  });

  it("takes the assistant button to the workspace being viewed, else to the page's own choice", () => {
    expect(assistantPath("w 1")).toBe("/assistant/w%201");
    expect(assistantEntryPath("/spaces/w1/memory")).toBe("/assistant/w1");
    expect(assistantEntryPath("/spaces/w1/artifacts/a1")).toBe("/assistant/w1");
    expect(assistantEntryPath("/assistant/w2")).toBe("/assistant/w2");
    expect(assistantEntryPath("/dashboard")).toBe("/assistant");
    expect(routePath({ name: "assistant", workspaceId: "w1" })).toBe("/assistant/w1");
  });
});
