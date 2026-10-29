import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
vi.mock("../lib/sharedApi", () => ({
  pollProviderFlow: vi.fn(),
  SharedApiError: class extends Error { status: number; constructor(status: number, message: string) { super(message); this.status = status; } },
}));
vi.mock("../lib/desktop", () => ({ isDesktopRuntime: false, openExternal: vi.fn() }));
vi.mock("../components/ui/toast", () => ({ Toast: ({ message }: { message: string | null }) => <p>{message}</p>, showToast: vi.fn() }));
import { pollProviderFlow, SharedApiError } from "../lib/sharedApi";
import { showToast } from "../components/ui/toast";
import { ProviderSignIn } from "../components/ProviderSignIn";
const flow = { authorization_url: "https://id.example.com/auth", state: "s", proof: "p", expires_in: 300 };
beforeEach(() => { vi.useFakeTimers(); vi.stubGlobal("open", vi.fn()); });
afterEach(() => { cleanup(); vi.useRealTimers(); vi.resetAllMocks(); vi.unstubAllGlobals(); });
async function begin() {
  await act(async () => { fireEvent.click(screen.getByRole("button", { name: "Continue with Company" })); });
}
it("hands over the session only after the confirmation code is typed", async () => {
  vi.mocked(pollProviderFlow).mockResolvedValueOnce(null).mockResolvedValueOnce({ access_token: "token" } as never);
  const done = vi.fn();
  render(<ProviderSignIn label="Continue with Company" onDone={done} start={async () => flow} />);
  await begin();
  expect(window.open).toHaveBeenCalledWith(flow.authorization_url, "_blank", "noopener");
  await act(async () => { await vi.advanceTimersByTimeAsync(2000); });
  expect(done).not.toHaveBeenCalled();
  expect((screen.getByRole("button", { name: "Finish signing in" }) as HTMLButtonElement).disabled).toBe(true);
  fireEvent.change(screen.getByLabelText("Confirmation code"), { target: { value: "1234 5678" } });
  await act(async () => { fireEvent.click(screen.getByRole("button", { name: "Finish signing in" })); });
  expect(pollProviderFlow).toHaveBeenLastCalledWith(flow, true, "1234 5678");
  expect(done).toHaveBeenCalledWith({ access_token: "token" });
});
it("keeps waiting after a wrong code but returns to the form on cancellation, provider failure and expiry", async () => {
  vi.mocked(pollProviderFlow).mockResolvedValue(null);
  render(<ProviderSignIn label="Continue with Company" onDone={vi.fn()} start={async () => flow} />);
  await begin();
  vi.mocked(pollProviderFlow).mockRejectedValueOnce(new SharedApiError(409, "That confirmation code is not right."));
  fireEvent.change(screen.getByLabelText("Confirmation code"), { target: { value: "00000000" } });
  await act(async () => { fireEvent.click(screen.getByRole("button", { name: "Finish signing in" })); });
  expect(showToast).toHaveBeenCalledWith("error", "That confirmation code is not right.");
  expect(screen.getByLabelText("Confirmation code")).toBeTruthy();
  vi.mocked(pollProviderFlow).mockRejectedValueOnce(new Error("Provider sign-in was cancelled or refused."));
  await act(async () => { await vi.advanceTimersByTimeAsync(2000); });
  expect(screen.getByRole("button", { name: "Continue with Company" })).toBeTruthy();
  await begin();
  fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
  expect(screen.getByRole("button", { name: "Continue with Company" })).toBeTruthy();
  await begin();
  await act(async () => { await vi.advanceTimersByTimeAsync(303000); });
  expect(screen.getByText(/expired/)).toBeTruthy();
});
