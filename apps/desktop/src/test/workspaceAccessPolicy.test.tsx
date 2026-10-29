import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
vi.mock("../lib/sharedApi", () => ({ request: vi.fn() }));
vi.mock("../components/ui/toast", () => ({ showToast: vi.fn() }));
import { request } from "../lib/sharedApi";
import { WorkspaceAccessPolicy } from "../components/WorkspaceAccessPolicy";
afterEach(() => { cleanup(); vi.resetAllMocks(); });
it("says nobody is added or removed when the policy changes", async () => {
  vi.mocked(request).mockResolvedValueOnce({ policy: "shared" }).mockResolvedValueOnce({ policy: "restricted" });
  render(<WorkspaceAccessPolicy accessToken="session" canChange workspaceId="w1" />);
  const toggle = await screen.findByRole("switch", { name: "Restrict this workspace" });
  fireEvent.click(toggle);
  await waitFor(() => expect(request).toHaveBeenCalledWith("/v1/workspaces/w1/access-policy", expect.objectContaining({ method: "PUT", body: JSON.stringify({ policy: "restricted" }) }), "session"));
  expect(await screen.findByText(/Restricted: people who join the organization are not added/)).toBeTruthy();
});
it("shows the policy but no control to a member who cannot change it", async () => {
  vi.mocked(request).mockResolvedValueOnce({ policy: "restricted" });
  render(<WorkspaceAccessPolicy accessToken="session" canChange={false} workspaceId="w1" />);
  expect(await screen.findByText(/Restricted:/)).toBeTruthy();
  expect(screen.queryByRole("switch")).toBeNull();
});
