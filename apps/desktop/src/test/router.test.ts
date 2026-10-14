import { describe, expect, it } from "vitest";
import { parseRoute, routePath } from "../web/router";

describe("parseRoute", () => {
  it("names the pages", () => {
    expect(parseRoute("/")).toEqual({ name: "workspaces" });
    expect(parseRoute("/workspaces")).toEqual({ name: "workspaces" });
    expect(parseRoute("/dashboard")).toEqual({ name: "dashboard" });
    expect(parseRoute("/profile")).toEqual({ name: "profile" });
    expect(parseRoute("/notifications")).toEqual({ name: "notifications" });
  });

  it("reads workspace sections, artifacts and memory cards", () => {
    expect(parseRoute("/workspaces/w1")).toEqual({ name: "workspace", workspaceId: "w1", section: "overview" });
    expect(parseRoute("/workspaces/w1/memory")).toEqual({ name: "workspace", workspaceId: "w1", section: "memory" });
    expect(parseRoute("/workspaces/w%201/artifacts/a%2F1")).toEqual({ name: "artifact", workspaceId: "w 1", artifactId: "a/1" });
    expect(parseRoute("/workspaces/w1/memory-cards/c1")).toEqual({ name: "memory-card", workspaceId: "w1", cardId: "c1" });
  });

  it("reads the System area, with overview as its front page", () => {
    expect(parseRoute("/system")).toEqual({ name: "system", section: "overview" });
    expect(parseRoute("/system/logs")).toEqual({ name: "system", section: "logs" });
    expect(parseRoute("/system/nothing")).toEqual({ name: "not-found", workspaceId: null });
    expect(parseRoute("/system/logs/extra")).toEqual({ name: "not-found", workspaceId: null });
  });

  it("keeps the workspace of an address that names nothing", () => {
    expect(parseRoute("/workspaces/w1/unknown")).toEqual({ name: "not-found", workspaceId: "w1" });
    expect(parseRoute("/workspaces/w1/artifacts")).toEqual({ name: "not-found", workspaceId: "w1" });
    expect(parseRoute("/elsewhere")).toEqual({ name: "not-found", workspaceId: null });
  });

  it("is the inverse of routePath", () => {
    for (const path of ["/workspaces", "/dashboard", "/system/audit", "/workspaces/w%201/tasks", "/workspaces/w1/artifacts/a%2F1", "/workspaces/w1/memory-cards/c1"]) {
      const route = parseRoute(path);
      if (route.name === "not-found") throw new Error(path);
      expect(routePath(route)).toBe(path);
    }
  });
});
