import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
// The browser tsconfig intentionally omits Node types; these imports run in Vitest only.
// @ts-expect-error Node type declarations are outside the browser bundle.
import { readFileSync } from "node:fs";
// @ts-expect-error Node type declarations are outside the browser bundle.
import { resolve } from "node:path";

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

  it("keeps the person's menu in the rail, named, and out of the top bar", () => {
    screenIs("narrow");
    render(<SharedLayout apiAvailable onNavigate={() => undefined} session={session} sidebar={<p>rail</p>} signOut={() => undefined}><p>page</p></SharedLayout>);
    expect(screen.getByRole("button", { name: "Account menu for Ada" })).toBeTruthy();
    expect(document.querySelector(".shared-home-header")?.querySelector(".shared-user-trigger")).toBeNull();
    expect(screen.queryByRole("button", { name: "Sign out" })).toBeNull();
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
