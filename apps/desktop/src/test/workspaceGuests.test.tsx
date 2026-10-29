import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
vi.mock("../lib/sharedApi", () => ({ request: vi.fn() }));
vi.mock("../components/ui/toast", () => ({ showToast: vi.fn() }));
import { request } from "../lib/sharedApi";
import { WorkspaceGuests } from "../components/WorkspaceGuests";
afterEach(() => { cleanup(); vi.resetAllMocks(); });
const guest = (expires_at: string | null, expired: boolean) => ({ user_id: "g1", email: "guest@example.com", display_name: "Guest", role: "member", expires_at, expired });
it("renews an ended guest with a later day", async () => {
  vi.mocked(request).mockImplementation(async (_path: string, options?: RequestInit) => options?.method === "PUT" ? {} : { guests: [guest("2020-01-01T23:59:59+00:00", true)] });
  render(<WorkspaceGuests accessToken="session" workspaceId="w1" />);
  expect(await screen.findByText(/^Ended /)).toBeTruthy();
  fireEvent.change(screen.getByLabelText("End date for Guest"), { target: { value: "2026-12-31" } });
  fireEvent.click(screen.getByRole("button", { name: "Renew" }));
  await waitFor(() => expect(request).toHaveBeenCalledWith("/v1/workspaces/w1/guests/g1/expiry", expect.objectContaining({ method: "PUT", body: JSON.stringify({ expires_at: "2026-12-31" }) }), "session"));
});
it("shows nothing when the workspace has no guests", async () => {
  vi.mocked(request).mockResolvedValue({ guests: [] });
  const { container } = render(<WorkspaceGuests accessToken="session" workspaceId="w1" />);
  await waitFor(() => expect(request).toHaveBeenCalled());
  expect(container.textContent).toBe("");
});
