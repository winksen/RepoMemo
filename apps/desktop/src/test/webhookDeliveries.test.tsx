import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../lib/sharedApi", () => ({ listSharedWebhookDeliveries: vi.fn(), resendSharedWebhookDelivery: vi.fn() }));
vi.mock("../components/ui/toast", () => ({ showToast: vi.fn() }));

import { listSharedWebhookDeliveries, resendSharedWebhookDelivery } from "../lib/sharedApi";
import { showToast } from "../components/ui/toast";
import { canSendAgain, describeDelivery, WebhookDeliveries } from "../components/WebhookDeliveries";
import type { WebhookDelivery } from "../types";

const delivery = (changes: Partial<WebhookDelivery>): WebhookDelivery => ({
  id: "d1", received_at: "2026-10-24T10:00:00.000Z", sender: "github", event: "issues", delivery_id: "", outcome: "queued", reason: "", resent_at: null, ...changes,
});

beforeEach(() => {
  vi.mocked(listSharedWebhookDeliveries).mockReset();
  vi.mocked(resendSharedWebhookDelivery).mockReset();
  vi.mocked(showToast).mockReset();
});
afterEach(() => cleanup());

describe("a call a webhook received", () => {
  it("says the event, the sender, what became of it and why not", () => {
    expect(describeDelivery(delivery({}))).toBe("issues from github · sync queued");
    expect(describeDelivery(delivery({ event: "push", sender: "other", outcome: "ignored", reason: "another branch" }))).toBe("push from another sender · ignored: another branch");
    expect(describeDelivery(delivery({ sender: "gitlab", event: "Issue Hook", outcome: "failed", reason: "Acme is paused until tomorrow." }))).toBe("Issue Hook from gitlab · could not be queued: Acme is paused until tomorrow.");
    expect(describeDelivery(delivery({ sender: "resend", outcome: "absorbed" }))).toBe("issues sent again from this page · a sync was already running");
  });

  it("can be sent again only when it asked for a sync", () => {
    for (const outcome of ["queued", "absorbed", "failed"]) expect(canSendAgain(delivery({ outcome }))).toBe(true);
    for (const outcome of ["ignored", "pong"]) expect(canSendAgain(delivery({ outcome }))).toBe(false);
  });
});

describe("WebhookDeliveries", () => {
  it("lists the calls newest first with a button on those that asked for a sync", async () => {
    vi.mocked(listSharedWebhookDeliveries).mockResolvedValue([
      delivery({ id: "d3", outcome: "failed", reason: "Acme is paused until tomorrow." }),
      delivery({ id: "d2", event: "push", outcome: "ignored", reason: "not an issue or pull request event" }),
      delivery({ id: "d1", event: "ping", outcome: "pong", resent_at: "2026-10-24T11:00:00.000Z" }),
    ]);
    render(<WebhookDeliveries accessToken="t" kind="connectors" sourceId="c1" />);
    await screen.findByText(/could not be queued: Acme is paused until tomorrow\./);
    expect(listSharedWebhookDeliveries).toHaveBeenCalledWith("t", "connectors", "c1");
    expect(screen.getAllByRole("button", { name: /send again/i })).toHaveLength(1);
    expect(screen.getByText(/ignored: not an issue or pull request event/)).toBeTruthy();
    expect(screen.getByText(/sent again/)).toBeTruthy();
  });

  it("says so when nothing has been received, and shows a failed load as a toast", async () => {
    vi.mocked(listSharedWebhookDeliveries).mockResolvedValueOnce([]);
    const { unmount } = render(<WebhookDeliveries accessToken="t" kind="repositories" sourceId="r1" />);
    await screen.findByText("No call has been received yet.");
    unmount();
    vi.mocked(listSharedWebhookDeliveries).mockRejectedValueOnce(new Error("Nope."));
    render(<WebhookDeliveries accessToken="t" kind="repositories" sourceId="r1" />);
    await waitFor(() => expect(showToast).toHaveBeenCalledWith("error", "Nope."));
  });

  it("sends a call again, tells what came of it and reads the list again", async () => {
    const failed = delivery({ id: "d3", outcome: "failed", reason: "paused" });
    vi.mocked(listSharedWebhookDeliveries).mockResolvedValueOnce([failed]).mockResolvedValue([delivery({ id: "d4", sender: "resend" }), { ...failed, resent_at: "2026-10-24T12:00:00.000Z" }]);
    vi.mocked(resendSharedWebhookDelivery).mockResolvedValue({ queued: true, note: "sync queued", delivery: delivery({ id: "d4", sender: "resend" }) });
    const onResent = vi.fn();
    render(<WebhookDeliveries accessToken="t" kind="connectors" onResent={onResent} sourceId="c1" />);
    fireEvent.click(await screen.findByRole("button", { name: /send again/i }));
    await waitFor(() => expect(showToast).toHaveBeenCalledWith("success", "A sync was queued."));
    expect(resendSharedWebhookDelivery).toHaveBeenCalledWith("t", "connectors", "c1", "d3");
    expect(onResent).toHaveBeenCalled();
    await screen.findByText(/issues sent again from this page · sync queued/);
  });

  it("reports a resend that could not be queued, or that the server refused, as an error", async () => {
    vi.mocked(listSharedWebhookDeliveries).mockResolvedValue([delivery({ id: "d3", outcome: "failed", reason: "paused" })]);
    vi.mocked(resendSharedWebhookDelivery)
      .mockResolvedValueOnce({ queued: false, note: "Acme is paused until tomorrow.", delivery: delivery({ id: "d5", sender: "resend", outcome: "failed", reason: "Acme is paused until tomorrow." }) })
      .mockRejectedValueOnce(new Error("Deliveries of this source are being sent again too fast."));
    render(<WebhookDeliveries accessToken="t" kind="connectors" sourceId="c1" />);
    fireEvent.click(await screen.findByRole("button", { name: /send again/i }));
    await waitFor(() => expect(showToast).toHaveBeenCalledWith("error", "It could not be queued: Acme is paused until tomorrow."));
    fireEvent.click(await screen.findByRole("button", { name: /send again/i }));
    await waitFor(() => expect(showToast).toHaveBeenCalledWith("error", "Deliveries of this source are being sent again too fast."));
  });
});
