import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
vi.mock("../lib/sharedApi", () => ({ request: vi.fn() }));
vi.mock("../components/ui/toast", () => ({ showToast: vi.fn() }));
import { request } from "../lib/sharedApi";
import { TwoStepPanel } from "../components/TwoStepPanel";
afterEach(() => { cleanup(); vi.resetAllMocks(); });
it("turns on only after a right code and shows the recovery codes until they are kept", async () => {
  let enabled = false;
  vi.mocked(request).mockImplementation(async (path: string, options?: RequestInit) => {
    if (path === "/v1/profile/mfa/start") return { secret: "JBSWY3DPEHPK3PXP", otpauth_url: "otpauth://totp/RepoMemo:a?secret=JBSWY3DPEHPK3PXP" };
    if (path === "/v1/profile/mfa/confirm") { enabled = true; expect(JSON.parse(String(options?.body))).toEqual({ code: "123456" }); return { recovery_codes: ["AAAAAAAA-BBBBBBBB", "CCCCCCCC-DDDDDDDD"] }; }
    return { enabled, recovery_codes_remaining: enabled ? 2 : 0 };
  });
  render(<TwoStepPanel accessToken="session" />);
  fireEvent.click(await screen.findByRole("button", { name: "Set up two-step sign-in" }));
  expect(await screen.findByText("JBSWY3DPEHPK3PXP")).toBeTruthy();
  const turnOn = screen.getByRole("button", { name: "Turn on" }) as HTMLButtonElement;
  expect(turnOn.disabled).toBe(true);
  fireEvent.change(screen.getByLabelText(/First code from the app/), { target: { value: "123456" } });
  fireEvent.click(screen.getByRole("button", { name: "Turn on" }));
  expect(await screen.findByLabelText("Recovery codes")).toBeTruthy();
  expect(screen.getByText(/AAAAAAAA-BBBBBBBB/)).toBeTruthy();
  expect(screen.queryByRole("button", { name: "Done" })).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "I have saved them" }));
  fireEvent.click(await screen.findByRole("button", { name: "Done" }));
  await waitFor(() => expect(screen.queryByLabelText("Recovery codes")).toBeNull());
  expect(screen.getByText(/2 recovery codes left\. Replace them soon\./)).toBeTruthy();
});
it("asks for the password and a code before turning it off", async () => {
  vi.mocked(request).mockResolvedValue({ enabled: true, recovery_codes_remaining: 9 });
  render(<TwoStepPanel accessToken="session" />);
  fireEvent.click(await screen.findByRole("button", { name: "Turn off" }));
  const confirm = await screen.findAllByRole("button", { name: "Turn off" });
  const submit = confirm[confirm.length - 1] as HTMLButtonElement;
  expect(submit.disabled).toBe(true);
  fireEvent.change(screen.getByLabelText(/^Password/), { target: { value: "password-1234" } });
  fireEvent.change(screen.getByLabelText(/^Code/), { target: { value: "123456" } });
  await waitFor(() => expect((submit as HTMLButtonElement).disabled).toBe(false));
  fireEvent.click(submit);
  await waitFor(() => expect(request).toHaveBeenCalledWith("/v1/profile/mfa/disable", expect.objectContaining({ body: JSON.stringify({ password: "password-1234", code: "123456" }) }), "session"));
});
