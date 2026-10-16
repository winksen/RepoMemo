import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../components/AssistantPanel", () => ({ AssistantPanel: ({ workspaceId }: { workspaceId: string }) => <div data-testid="panel">assistant for {workspaceId}<button type="button">inside</button></div> }));

import { AssistantDock } from "../components/AssistantDock";
import { SharedLayout } from "../components/SharedLayout";
import { NARROW_QUERY } from "../lib/useMediaQuery";
import css from "../styles/13-narrow-screens.css?raw";
import index from "../styles/index.ts?raw";
import type { SharedSession } from "../types";

function screenIs(width: "narrow" | "wide") {
  window.matchMedia = ((query: string) => ({
    matches: query === NARROW_QUERY ? width === "narrow" : false,
    media: query,
    addEventListener: () => undefined,
    removeEventListener: () => undefined,
  })) as unknown as typeof window.matchMedia;
}

beforeEach(() => window.localStorage.clear());
afterEach(cleanup);

const dock = () => render(<AssistantDock accessToken="t" onOpenArtifact={() => undefined} workspaceId="w1" />);

describe("the assistant on a narrow screen", () => {
  it("starts closed as a floating button, whatever the wide-screen state is", () => {
    window.localStorage.setItem("repomemo.assistant.dock", JSON.stringify({ open: true, width: 500 }));
    screenIs("narrow");
    dock();
    expect(screen.getByRole("button", { name: "Open assistant" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Close assistant" })).toBeNull();
    expect(screen.getByLabelText("Workspace assistant").classList.contains("sheet")).toBe(true);
  });

  it("opens as a sheet with a backdrop and closes with Esc, the close button or the backdrop, giving focus back to its button", () => {
    screenIs("narrow");
    dock();
    fireEvent.click(screen.getByRole("button", { name: "Open assistant" }));
    expect(screen.getByTestId("panel").textContent).toContain("assistant for w1");
    const sheet = screen.getByLabelText("Workspace assistant");
    expect(sheet.classList.contains("collapsed")).toBe(false);
    expect(document.activeElement).toBe(sheet);
    // Two buttons are named Close assistant: the backdrop (out of the tab order) and the sheet's own.
    expect(screen.getAllByRole("button", { name: "Close assistant" })).toHaveLength(2);

    fireEvent.keyDown(screen.getByText("inside"), { key: "Escape" });
    expect(sheet.classList.contains("collapsed")).toBe(true);
    expect(document.activeElement).toBe(screen.getByRole("button", { name: "Open assistant" }));

    fireEvent.click(screen.getByRole("button", { name: "Open assistant" }));
    fireEvent.click(document.querySelector(".shared-assistant-backdrop") as HTMLElement);
    expect(sheet.classList.contains("collapsed")).toBe(true);

    fireEvent.click(screen.getByRole("button", { name: "Open assistant" }));
    fireEvent.click(document.querySelector(".shared-assistant-dock-header button") as HTMLElement);
    expect(sheet.classList.contains("collapsed")).toBe(true);
  });

  it("keeps the conversation mounted while the sheet is closed", () => {
    screenIs("narrow");
    dock();
    fireEvent.click(screen.getByRole("button", { name: "Open assistant" }));
    const panel = screen.getByTestId("panel");
    fireEvent.keyDown(panel, { key: "Escape" });
    expect(document.querySelector('[data-testid="panel"]')).toBe(panel);
  });

  it("does not change the wide-screen dock or its saved state", () => {
    screenIs("wide");
    dock();
    expect(screen.queryByRole("button", { name: "Open assistant" })).toBeNull();
    expect(screen.getByRole("button", { name: "Shrink assistant" })).toBeTruthy();
    expect(screen.getByLabelText("Workspace assistant").classList.contains("sheet")).toBe(false);
    fireEvent.keyDown(screen.getByText("inside"), { key: "Escape" });
    expect(screen.getByRole("button", { name: "Shrink assistant" })).toBeTruthy();
    expect(JSON.parse(window.localStorage.getItem("repomemo.assistant.dock") ?? "{}").open).toBe(true);
  });
});

describe("the rail on a narrow screen", () => {
  const session = { user: { id: "u1", email: "a@example.com", display_name: "Ada" }, authentication: "jwt", memberships: [] } as unknown as SharedSession;

  it("is folded behind a button that says whether it is open", () => {
    screenIs("narrow");
    render(<SharedLayout apiAvailable onNavigate={() => undefined} session={session} sidebar={<p>rail</p>} signOut={() => undefined}><h1>Page</h1></SharedLayout>);
    const toggle = screen.getByRole("button", { name: "Workspaces and organizations" });
    expect(toggle.getAttribute("aria-expanded")).toBe("false");
    expect(document.querySelector(".shared-home-rail")?.getAttribute("data-open")).toBe("false");
    fireEvent.click(toggle);
    expect(toggle.getAttribute("aria-expanded")).toBe("true");
    expect(document.querySelector(".shared-home-rail")?.getAttribute("data-open")).toBe("true");
    expect(toggle.getAttribute("aria-controls")).toBe(document.querySelector(".shared-rail-body")?.id);
  });

  it("keeps the header buttons named when their words are hidden", () => {
    screenIs("narrow");
    render(<SharedLayout apiAvailable onNavigate={() => undefined} session={session} sidebar={<p>rail</p>} signOut={() => undefined}><h1>Page</h1></SharedLayout>);
    expect(screen.getByRole("button", { name: "Profile" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Sign out" })).toBeTruthy();
  });
});

describe("the narrow-screen stylesheet", () => {
  it("uses the width the code uses, and is the last file of the cascade", () => {
    expect(NARROW_QUERY).toBe("(max-width: 900px)");
    expect(css).toContain(`@media ${NARROW_QUERY}`);
    const imports = [...index.matchAll(/import "\.\/([^"]+)";/g)].map((match) => match[1]);
    expect(imports[imports.length - 1]).toBe("13-narrow-screens.css");
  });

  it("lets the page scroll and stops it from growing sideways on a tablet", () => {
    const tablet = css.slice(css.indexOf("@media (max-width: 900px)"), css.indexOf("@media (max-width: 640px)"));
    expect(tablet).toMatch(/html, body, #root \{[^}]*overflow: visible; overflow-x: clip/);
  });
});
