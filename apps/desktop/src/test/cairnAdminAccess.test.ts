import { describe, expect, it } from "vitest";
import { cairnAdminSections } from "../lib/cairnAdminAccess";
import { paletteEntries } from "../web/paletteEntries";

const member = { is_system_admin: false, is_app_admin: false, is_cairn_admin: false };

describe("cairnAdminSections", () => {
  it("gives a member no section, and so no Administration tab", () => {
    expect(cairnAdminSections(member, [{ role: "member", guest: false }])).toEqual([]);
    expect(cairnAdminSections(member, [{ role: "admin", guest: true }])).toEqual([]);
  });

  it("gives owners and admins of an organization only the sections about their organizations", () => {
    expect(cairnAdminSections(member, [{ role: "admin", guest: false }])).toEqual(["organizations", "guardrails", "abilities", "memory", "usage"]);
    expect(cairnAdminSections(member, [{ role: "owner", guest: false }])).toHaveLength(5);
  });

  it("gives system, CAIRN and app administrators every section", () => {
    for (const flag of ["is_system_admin", "is_cairn_admin", "is_app_admin"] as const) {
      expect(cairnAdminSections({ ...member, [flag]: true }, [])).toHaveLength(8);
    }
  });
});

describe("the palette", () => {
  const base = { query: "administration", workspaces: [], assistantWorkspace: undefined, currentWorkspace: undefined, files: [], cards: [], canAdministerSystem: false };

  it("offers CAIRN administration only to people who may open it", () => {
    expect(paletteEntries(base).some((entry) => entry.id === "page:cairn-admin")).toBe(false);
    expect(paletteEntries({ ...base, canAdministerCairn: true }).some((entry) => entry.id === "page:cairn-admin")).toBe(true);
  });
});
