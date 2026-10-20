import { QueryClientProvider } from "@tanstack/react-query";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../lib/sharedApi", () => ({
  checkSharedConnector: vi.fn(),
  connectSharedConnector: vi.fn(),
  createSharedConnectorWebhook: vi.fn(),
  deleteSharedConnector: vi.fn(),
  deleteSharedConnectorWebhook: vi.fn(),
  listSharedConnectors: vi.fn(),
  setSharedConnectorToken: vi.fn(),
  syncSharedConnector: vi.fn(),
}));
vi.mock("../components/ui/toast", () => ({ Toast: () => null, showToast: vi.fn() }));

import { listSharedConnectors } from "../lib/sharedApi";
import { queryClient } from "../lib/queryClient";
import { ConnectorSettings } from "../components/ConnectorsPanel";

beforeEach(() => { queryClient.clear(); });
afterEach(() => { cleanup(); });

function renderConnectors() {
  return render(<QueryClientProvider client={queryClient}><ConnectorSettings accessToken="t" onChanged={vi.fn()} workspaceId="w1" /></QueryClientProvider>);
}

describe("the connector form on the shared pieces", () => {
  it("shows the shared loading state until the connectors are read", () => {
    vi.mocked(listSharedConnectors).mockReturnValue(new Promise(() => undefined));
    renderConnectors();
    expect(screen.getByRole("status").textContent).toBe("Loading connectors…");
  });

  it("labels the address, tracker and token as fields with hints and keeps Check access off without an address", async () => {
    vi.mocked(listSharedConnectors).mockResolvedValue({ connectors: [] } as never);
    renderConnectors();
    const address = await screen.findByLabelText("Repository address") as HTMLInputElement;
    expect(address.closest(".rm-field")).not.toBeNull();
    expect(address.getAttribute("aria-describedby")).toBeTruthy();
    expect(screen.getByLabelText("Tracker").closest(".rm-field")).not.toBeNull();
    expect((screen.getByLabelText("Access token") as HTMLInputElement).type).toBe("password");
    expect((screen.getByRole("button", { name: /Check access/ }) as HTMLButtonElement).disabled).toBe(true);
    fireEvent.change(address, { target: { value: "https://github.com/acme/app" } });
    expect((screen.getByRole("button", { name: /Check access/ }) as HTMLButtonElement).disabled).toBe(false);
  });
});
