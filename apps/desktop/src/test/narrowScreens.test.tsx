import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
// The browser tsconfig intentionally omits Node types; these imports run in Vitest only.
// @ts-expect-error Node type declarations are outside the browser bundle.
import { readFileSync } from "node:fs";
// @ts-expect-error Node type declarations are outside the browser bundle.
import { resolve } from "node:path";

vi.mock("../components/AssistantPanel", () => ({ AssistantPanel: ({ workspaceId }: { workspaceId: string }) => <div data-testid="panel">assistant for {workspaceId}<button type="button">inside</button></div> }));

import { AssistantDock } from "../components/AssistantDock";
import { SharedLayout } from "../components/SharedLayout";
import { NARROW_QUERY } from "../lib/useMediaQuery";
const css = readFileSync(resolve("src/styles/13-narrow-screens.css"), "utf8");
const index = readFileSync(resolve("src/styles/index.ts"), "utf8");
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
afterEach(() => { cleanup(); delete document.documentElement.dataset.theme; });

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

  it.each(["light", "dark"])("keeps Tab inside the assistant sheet in the %s theme", (theme) => {
    document.documentElement.dataset.theme = theme;
    screenIs("narrow");
    dock();
    fireEvent.click(screen.getByRole("button", { name: "Open assistant" }));
    const sheet = screen.getByRole("dialog", { name: "Workspace assistant" });
    expect(sheet.getAttribute("aria-modal")).toBe("true");
    const first = sheet.querySelector(".shared-assistant-dock-header button") as HTMLButtonElement;
    const last = screen.getByRole("button", { name: "inside" });
    fireEvent.keyDown(sheet, { key: "Tab" });
    expect(document.activeElement).toBe(first);
    fireEvent.keyDown(first, { key: "Tab", shiftKey: true });
    expect(document.activeElement).toBe(last);
    fireEvent.keyDown(last, { key: "Tab" });
    expect(document.activeElement).toBe(first);
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
  it("uses the width the code uses, before the final building-block rules", () => {
    expect(NARROW_QUERY).toBe("(max-width: 900px)");
    expect(css).toContain(`@media ${NARROW_QUERY}`);
    const imports = [...index.matchAll(/import "\.\/([^"]+)";/g)].map((match) => match[1]);
    expect(imports.slice(-2)).toEqual(["13-narrow-screens.css", "14-building-blocks.css"]);
  });

  it("lets the page scroll and stops it from growing sideways on a tablet", () => {
    const tablet = css.slice(css.indexOf("@media (max-width: 900px)"), css.indexOf("@media (max-width: 640px)"));
    expect(tablet).toMatch(/html, body, #root \{[^}]*overflow: visible; overflow-x: clip/);
    expect(tablet).toMatch(/\.shared-home-content \{[^}]*overflow: visible/);
  });

  it("keeps navigation, result metadata and dialog actions available at 320 CSS pixels", () => {
    const small = css.slice(css.indexOf("@media (max-width: 420px)"));
    expect(small).toMatch(/\.shared-home \{ grid-template-rows: auto/);
    expect(small).toMatch(/\.shared-home-header \{ flex-wrap: wrap/);
    expect(small).toMatch(/\.shared-search-result-heading \{ flex-wrap: wrap/);
    expect(small).toMatch(/\.rm-dialog-footer \{ flex-wrap: wrap/);
    expect(small).toMatch(/\.rm-feedback-notice \{ flex-wrap: wrap/);
  });
});
