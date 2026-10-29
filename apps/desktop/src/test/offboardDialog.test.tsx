import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
vi.mock("../lib/sharedApi", () => ({ request: vi.fn() }));
vi.mock("../components/ui/toast", () => ({ showToast: vi.fn() }));
import { request } from "../lib/sharedApi";
import { OffboardDialog } from "../components/OffboardDialog";
import type { SystemUser } from "../types";
afterEach(() => { cleanup(); vi.resetAllMocks(); });
const person = (id: string, name: string) => ({ id, display_name: name, email: `${id}@example.com`, deactivated_at: null }) as unknown as SystemUser;
const preview = { owned: [{ kind: "workspace", id: "w1", name: "Payments" }], open_tasks: 2, tasks: [{ id: "t1", workspace: "Payments", title: "Review runbook", review: true }], sessions: 1, personal_tokens: 1, invitations: 0, last_system_administrator: false };
it("requires a recipient when there is work and offboards only after the preview", async () => {
  vi.mocked(request).mockImplementation(async (path: string, options?: RequestInit) => path.endsWith("/offboard") && options?.method === "POST"
    ? { ownership_transferred: 1, tasks_reassigned: 2, personal_tokens_revoked: 1, invitations_withdrawn: 0, verified: true } : preview);
  const onDone = vi.fn();
  render(<OffboardDialog accessToken="session" candidates={[person("a", "Ada"), person("b", "Grace")]} onClose={vi.fn()} onDone={onDone} user={person("a", "Ada")} />);
  expect(await screen.findByText(/Owns workspace “Payments”/)).toBeTruthy();
  expect(screen.getByText(/2 open tasks or reviews assigned/)).toBeTruthy();
  const submit = screen.getByRole("button", { name: "Offboard account" }) as HTMLButtonElement;
  expect(submit.disabled).toBe(true);
  fireEvent.click(screen.getByRole("combobox"));
  fireEvent.click(await screen.findByRole("option", { name: /Grace/ }));
  await waitFor(() => expect((screen.getByRole("button", { name: "Offboard account" }) as HTMLButtonElement).disabled).toBe(false));
  fireEvent.click(screen.getByRole("button", { name: "Offboard account" }));
  await waitFor(() => expect(request).toHaveBeenCalledWith("/v1/system/users/a/offboard", expect.objectContaining({ body: JSON.stringify({ transfer_to: "b" }) }), "session"));
  await waitFor(() => expect(onDone).toHaveBeenCalled());
});
it("refuses the last system administrator", async () => {
  vi.mocked(request).mockResolvedValue({ ...preview, owned: [], open_tasks: 0, tasks: [], last_system_administrator: true });
  render(<OffboardDialog accessToken="session" candidates={[]} onClose={vi.fn()} onDone={vi.fn()} user={person("a", "Ada")} />);
  expect(await screen.findByRole("alert")).toBeTruthy();
  expect((screen.getByRole("button", { name: "Offboard account" }) as HTMLButtonElement).disabled).toBe(true);
});
