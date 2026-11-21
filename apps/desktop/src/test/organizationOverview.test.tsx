import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";

vi.mock("../lib/useServerState", () => ({ useServerState: vi.fn() }));

import { OrganizationOverviewTab, OrganizationTopbar, barsBy, organizationSections } from "../web/OrganizationPage";
import { useServerState } from "../lib/useServerState";
import type { Organization, OrganizationOverview } from "../types";

const organization = (role: Organization["role"]) => ({ id: "o1", name: "Bend", role }) as Organization;
const overview: OrganizationOverview = {
  organization_id: "o1", member_count: 4, workspace_count: 2, artifact_count: 30, memory_card_count: 5, active_task_count: 3,
  activity_by_day: Array.from({ length: 30 }, (_, index) => ({ label: `2026-11-${String(index + 1).padStart(2, "0")}`, value: index % 3 })),
  workspaces: [
    { workspace_id: "w1", name: "Code", artifact_count: 10, memory_card_count: 0, active_task_count: 3, activity_count: 12 },
    { workspace_id: "w2", name: "Pay", artifact_count: 20, memory_card_count: 5, active_task_count: 0, activity_count: null },
  ],
};
const loaded = (data: OrganizationOverview | null, isLoading = false) => vi.mocked(useServerState).mockReturnValue([data, vi.fn(), { isLoading, isError: false }] as never);

beforeEach(() => vi.clearAllMocks());
afterEach(cleanup);

it("offers Administration only to owners and administrators", () => {
  expect(organizationSections(organization("owner"))).toEqual(["overview", "workspaces", "administration"]);
  expect(organizationSections(organization("admin"))).toContain("administration");
  expect(organizationSections(organization("member"))).toEqual(["overview", "workspaces"]);
});

it("marks the open tab and moves to another", () => {
  const onNavigate = vi.fn();
  render(<OrganizationTopbar activeSection="overview" onNavigate={onNavigate} organization={organization("member")} />);
  expect(screen.getByRole("button", { name: "Overview" }).getAttribute("aria-current")).toBe("page");
  expect(screen.queryByRole("button", { name: "Administration" })).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "Workspaces" }));
  expect(onNavigate).toHaveBeenCalledWith("workspaces");
});

it("orders workspaces by a figure and leaves out those with none", () => {
  expect(barsBy(overview, "artifact_count")).toEqual([{ label: "Pay", value: 20 }, { label: "Code", value: 10 }]);
  expect(barsBy(overview, "memory_card_count")).toEqual([{ label: "Pay", value: 5 }]);
  expect(barsBy(overview, "activity_count")).toEqual([{ label: "Code", value: 12 }]);
});

it("shows the totals, the charts and a row for each workspace, and opens one", () => {
  loaded(overview);
  const onOpenWorkspace = vi.fn();
  render(<OrganizationOverviewTab accessToken="t" onOpenWorkspace={onOpenWorkspace} organization={organization("owner")} />);
  expect(screen.getByText("Evidence by workspace")).toBeTruthy();
  expect(screen.getByText("Activity, last 30 days")).toBeTruthy();
  expect(screen.getByText("30")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Pay" }));
  expect(onOpenWorkspace).toHaveBeenCalledWith("w2");
  expect(screen.getAllByText("—").length).toBeGreaterThan(0);
});

it("leaves the activity out when the person administers no workspace, and shows placeholders while loading", () => {
  loaded({ ...overview, activity_by_day: [] });
  const { unmount } = render(<OrganizationOverviewTab accessToken="t" onOpenWorkspace={vi.fn()} organization={organization("member")} />);
  expect(screen.queryByText("Activity, last 30 days")).toBeNull();
  unmount();
  loaded(null, true);
  render(<OrganizationOverviewTab accessToken="t" onOpenWorkspace={vi.fn()} organization={organization("member")} />);
  expect(document.querySelectorAll(".rm-skeleton").length).toBeGreaterThan(0);
});
