import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
vi.mock("../lib/sharedApi", () => ({ request: vi.fn() }));
vi.mock("../components/ui/toast", () => ({ showToast: vi.fn() }));
import { request } from "../lib/sharedApi";
import { ConfirmProvider } from "../components/ui/confirm-dialog";
import { WorkspaceAccessPolicy } from "../components/WorkspaceAccessPolicy";
afterEach(() => { cleanup(); vi.resetAllMocks(); });
const member = (origin: string, via_group: string | null = null) => ({ user_id: origin, email: `${origin}@example.com`, display_name: origin, role: "member", origin, via_group });
const shared = { policy: "shared", members: [member("direct"), member("organization"), member("group", "eng")], system_administrators: [{ display_name: "Root", email: null }] };
it("explains each person's access and previews a change before making it", async () => {
  vi.mocked(request).mockImplementation(async (path: string, options?: RequestInit) => {
    if (path.endsWith("/effective-access")) return shared;
    if (path.endsWith("/access-policy/preview")) return { become_direct: 1, people: ["organization@example.com"] };
    if (options?.method === "PUT") return { policy: "restricted" };
    throw new Error(path);
  });
  render(<ConfirmProvider><WorkspaceAccessPolicy accessToken="session" canChange workspaceId="w1" /></ConfirmProvider>);
  expect(await screen.findByText("Follows the organization role")).toBeTruthy();
  expect(screen.getByText(/Granted by a provider group \(eng\)/)).toBeTruthy();
  expect(screen.getByText(/System administrators of this server can open every workspace/)).toBeTruthy();
  fireEvent.click(screen.getByRole("switch", { name: "Restrict this workspace" }));
  expect(await screen.findByText(/Nobody is removed\. 1 person's access stops following the organization role/)).toBeTruthy();
  expect(request).not.toHaveBeenCalledWith("/v1/workspaces/w1/access-policy", expect.objectContaining({ method: "PUT" }), "session");
  fireEvent.click(screen.getByRole("button", { name: "Restrict" }));
  await waitFor(() => expect(request).toHaveBeenCalledWith("/v1/workspaces/w1/access-policy", expect.objectContaining({ method: "PUT", body: JSON.stringify({ policy: "restricted" }) }), "session"));
});
it("shows access but no control to someone who cannot change the policy", async () => {
  vi.mocked(request).mockResolvedValue({ ...shared, policy: "restricted" });
  render(<ConfirmProvider><WorkspaceAccessPolicy accessToken="session" canChange={false} workspaceId="w1" /></ConfirmProvider>);
  expect(await screen.findByText(/Restricted:/)).toBeTruthy();
  expect(screen.queryByRole("switch")).toBeNull();
});
