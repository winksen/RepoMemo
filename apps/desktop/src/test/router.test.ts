import { describe, expect, it } from "vitest";
import { cairnPath, legacyRedirect, parseRoute, routePath } from "../web/router";

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
    expect(parseRoute("/cairn")).toEqual({ name: "cairn", chatId: null });
    expect(parseRoute("/cairn/c%201")).toEqual({ name: "cairn", chatId: "c 1" });
    expect(parseRoute("/cairn/c1/x")).toEqual({ name: "not-found", workspaceId: null });
    expect(parseRoute("/assistant")).toEqual({ name: "not-found", workspaceId: null });
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

  it("reads the CAIRN administration page, with overview as its front page, and keeps a chat's address", () => {
    expect(parseRoute("/cairn/admin")).toEqual({ name: "cairn-admin", section: "overview" });
    expect(parseRoute("/cairn/admin/models")).toEqual({ name: "cairn-admin", section: "models" });
    expect(parseRoute("/cairn/admin/nothing")).toEqual({ name: "not-found", workspaceId: null });
    expect(parseRoute("/cairn/chat-1")).toEqual({ name: "cairn", chatId: "chat-1" });
    expect(routePath({ name: "cairn-admin", section: "overview" })).toBe("/cairn/admin");
    expect(routePath({ name: "cairn-admin", section: "usage" })).toBe("/cairn/admin/usage");
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

  it("names Cairn's chats in the address", () => {
    expect(cairnPath(null)).toBe("/cairn");
    expect(cairnPath("c 1")).toBe("/cairn/c%201");
    expect(routePath({ name: "cairn", chatId: "c1" })).toBe("/cairn/c1");
  });
});
