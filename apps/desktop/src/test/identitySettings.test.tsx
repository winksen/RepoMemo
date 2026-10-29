import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
vi.mock("../lib/sharedApi", () => ({ request: vi.fn() }));
vi.mock("../lib/unsavedChanges", () => ({ useUnsavedChanges: vi.fn() }));
vi.mock("../components/ui/toast", () => ({ showToast: vi.fn() }));
import { request } from "../lib/sharedApi";
import { IdentitySettings } from "../components/IdentitySettings";
afterEach(() => { cleanup(); vi.resetAllMocks(); });
const overview = { provider: { name: "Company", issuer: "https://id.example.com", client_id: "memo", enabled: false, allowed_domains: [] }, has_secret: true, callback_url: "https://memo.example.com/v1/auth/identity/callback" };
it("checks without saving, keeps stored secrets hidden and clears a replacement after save", async () => {
  vi.mocked(request).mockResolvedValue(overview);
  render(<IdentitySettings accessToken="session" />);
  const secret = await screen.findByLabelText("Client secret") as HTMLInputElement;
  expect(secret.value).toBe("");
  fireEvent.change(secret, { target: { value: "replacement" } });
  fireEvent.click(screen.getByRole("button", { name: "Check discovery" }));
  await waitFor(() => expect(request).toHaveBeenCalledWith("/v1/system/identity/check", expect.objectContaining({ method: "POST" }), "session"));
  await waitFor(() => expect((screen.getByRole("button", { name: "Save provider" }) as HTMLButtonElement).disabled).toBe(false));
  expect(secret.value).toBe("replacement");
  fireEvent.click(screen.getByRole("button", { name: "Save provider" }));
  await waitFor(() => expect(secret.value).toBe(""));
});
it("offers retry after a settings load failure", async () => {
  vi.mocked(request).mockRejectedValueOnce(new Error("Unavailable")).mockResolvedValue(overview);
  render(<IdentitySettings accessToken="session" />);
  fireEvent.click(await screen.findByRole("button", { name: "Retry provider settings" }));
  expect(await screen.findByLabelText("Issuer URL")).toBeTruthy();
});
