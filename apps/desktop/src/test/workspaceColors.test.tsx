import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";

vi.mock("../lib/sharedApi", async (original) => ({ ...(await original<typeof import("../lib/sharedApi")>()), saveSharedWorkspaceColor: vi.fn(), saveSharedWorkspaceIcon: vi.fn() }));

import { WorkspaceCustomization } from "../components/WorkspaceCustomization";
import { saveSharedWorkspaceColor } from "../lib/sharedApi";
import { WORKSPACE_COLORS, applyWorkspaceColor, workspaceColorKey } from "../web/workspaceColors";
import type { Workspace } from "../types";

// WCAG contrast ratio of two #rrggbb colours.
function luminance(hex: string) {
  const channel = (offset: number) => {
    const value = parseInt(hex.slice(offset, offset + 2), 16) / 255;
    return value <= 0.03928 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4;
  };
  return 0.2126 * channel(1) + 0.7152 * channel(3) + 0.0722 * channel(5);
}
const contrast = (a: string, b: string) => {
  const [high, low] = [luminance(a), luminance(b)].sort((x, y) => y - x);
  return (high + 0.05) / (low + 0.05);
};
const LIGHT_SURFACE = "#ffffff";
const DARK_SURFACE = "#141414"; // hsl(0 0% 8%), the dark theme's surface

const workspace = (settings: Record<string, unknown> = {}) => ({ id: "w1", name: "Code", created_at: "", updated_at: "", settings }) as unknown as Workspace;

beforeEach(() => vi.clearAllMocks());
afterEach(cleanup);

it("keeps every colour readable in both themes", () => {
  expect(WORKSPACE_COLORS.length).toBeGreaterThanOrEqual(6);
  for (const { key, light, dark } of WORKSPACE_COLORS) {
    // The light theme takes the darker variant: its label and the text on the page read on white and on its own soft background.
    expect(contrast(light.on, light.accent), `${key} label on accent (light)`).toBeGreaterThanOrEqual(4.5);
    expect(contrast(light.on, light.hover), `${key} label on hover (light)`).toBeGreaterThanOrEqual(4.5);
    expect(contrast(light.text, LIGHT_SURFACE), `${key} text (light)`).toBeGreaterThanOrEqual(4.5);
    expect(contrast(light.text, light.soft), `${key} text on soft (light)`).toBeGreaterThanOrEqual(4.5);
    // The dark theme takes the lighter variant, which must stand out from the dark surface.
    expect(contrast(dark.accent, DARK_SURFACE), `${key} accent on surface (dark)`).toBeGreaterThanOrEqual(4.5);
    expect(contrast(dark.on, dark.accent), `${key} label on accent (dark)`).toBeGreaterThanOrEqual(4.5);
    expect(contrast(dark.on, dark.hover), `${key} label on hover (dark)`).toBeGreaterThanOrEqual(4.5);
    expect(contrast(dark.text, DARK_SURFACE), `${key} text (dark)`).toBeGreaterThanOrEqual(4.5);
    // The variants differ: that is the point of having two.
    expect(luminance(dark.accent)).toBeGreaterThan(luminance(light.accent));
  }
});

it("reads the colour a workspace chose and ignores one it does not know", () => {
  expect(workspaceColorKey(workspace({ color: "teal" }))).toBe("teal");
  expect(workspaceColorKey(workspace({ color: "chartreuse" }))).toBeNull();
  expect(workspaceColorKey(workspace())).toBeNull();
});

it("sets both variants on an element and takes them away again", () => {
  const element = document.createElement("div");
  applyWorkspaceColor(element, "teal");
  const teal = WORKSPACE_COLORS.find((color) => color.key === "teal")!;
  expect(element.getAttribute("data-workspace-color")).toBe("teal");
  expect(element.style.getPropertyValue("--ws-light-accent")).toBe(teal.light.accent);
  expect(element.style.getPropertyValue("--ws-dark-accent")).toBe(teal.dark.accent);
  applyWorkspaceColor(element, null);
  expect(element.hasAttribute("data-workspace-color")).toBe(false);
  expect(element.style.getPropertyValue("--ws-light-accent")).toBe("");
});

it("saves a chosen colour at once and reports it to the page", async () => {
  vi.mocked(saveSharedWorkspaceColor).mockResolvedValue({ color: "teal" });
  const onSaved = vi.fn();
  render(<WorkspaceCustomization accessToken="t" onSaved={onSaved} workspace={workspace()} />);
  expect(screen.getByRole("heading", { name: "Customization" })).toBeTruthy();
  expect(screen.getByRole("button", { name: "Default" }).getAttribute("aria-pressed")).toBe("true");
  fireEvent.click(screen.getByRole("button", { name: "Teal" }));
  await waitFor(() => expect(onSaved).toHaveBeenCalledTimes(1));
  expect(saveSharedWorkspaceColor).toHaveBeenCalledWith("t", "w1", "teal");
  expect(onSaved.mock.calls[0][0].settings.color).toBe("teal");
});

it("goes back to the default colour", async () => {
  vi.mocked(saveSharedWorkspaceColor).mockResolvedValue({ color: null });
  render(<WorkspaceCustomization accessToken="t" onSaved={vi.fn()} workspace={workspace({ color: "red" })} />);
  fireEvent.click(screen.getByRole("button", { name: "Default" }));
  await waitFor(() => expect(saveSharedWorkspaceColor).toHaveBeenCalledWith("t", "w1", null));
});
