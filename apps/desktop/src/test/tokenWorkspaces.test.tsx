import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
vi.mock("../lib/sharedApi", () => ({
  createPersonalToken: vi.fn(),
  limitPersonalTokenWorkspaces: vi.fn().mockResolvedValue(undefined),
  listPersonalTokens: vi.fn(),
  listSharedWorkspaces: vi.fn(),
  revokePersonalToken: vi.fn(),
}));
vi.mock("../components/ui/toast", () => ({ showToast: vi.fn() }));
import { limitPersonalTokenWorkspaces, listPersonalTokens, listSharedWorkspaces } from "../lib/sharedApi";
import { ApiTokensPanel } from "../components/ApiTokensPanel";
afterEach(() => { cleanup(); vi.clearAllMocks(); });
const workspace = (id: string, name: string) => ({ workspace: { id, name }, organization_id: "o", role: "owner" });
const token = (workspaces: string[] | null) => ({ id: "t1", name: "ci", prefix: "rmp_ab", scope: "write", created_at: "2026-10-01T00:00:00Z", expires_at: "2999-01-01T00:00:00Z", last_used_at: null, workspaces });
it("says what each token reaches and narrows an unlimited one", async () => {
  vi.mocked(listSharedWorkspaces).mockResolvedValue([workspace("w1", "Payments"), workspace("w2", "Docs")] as never);
  vi.mocked(listPersonalTokens).mockResolvedValue([token(null)] as never);
  render(<ApiTokensPanel accessToken="t" isAdministrator={false} />);
  expect(await screen.findByText(/reaches every workspace you can open/)).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Limit ci to workspaces" }));
  const submit = await screen.findByRole("button", { name: "Limit token" });
  expect((submit as HTMLButtonElement).disabled).toBe(true);
  fireEvent.click(await screen.findByRole("checkbox", { name: "Payments" }));
  await waitFor(() => expect((screen.getByRole("button", { name: "Limit token" }) as HTMLButtonElement).disabled).toBe(false));
  fireEvent.click(screen.getByRole("button", { name: "Limit token" }));
  await waitFor(() => expect(limitPersonalTokenWorkspaces).toHaveBeenCalledWith("t", "t1", ["w1"]));
  expect(await screen.findByText(/limited to Payments/)).toBeTruthy();
});
