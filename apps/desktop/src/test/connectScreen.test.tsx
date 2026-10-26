import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

vi.mock("../lib/desktop", () => ({
  isDesktopRuntime: true,
  installExternalLinkHandler: () => () => undefined,
  followPageTitle: () => () => undefined,
  desktopServer: { current: vi.fn(), prepare: vi.fn(), remember: vi.fn(), forget: vi.fn() },
}));
vi.mock("../lib/sharedApi", () => ({ configureDesktopApi: vi.fn(), getSharedHealth: vi.fn(), sharedApiUrl: "https://repomemo.example.com" }));
vi.mock("../components/ui/toast", () => ({ showToast: vi.fn(), Toast: () => null }));
vi.mock("../components/SharedLayout", () => ({ ThemeToggle: () => <button type="button">Theme</button> }));

import { AuthFrame } from "../components/AuthFrame";
import { DesktopConnect } from "../components/DesktopConnect";

const components = import.meta.glob("../components/*.tsx", { query: "?raw", import: "default", eager: true }) as Record<string, string>;
const pages = import.meta.glob("../web/AuthCanvas.tsx", { query: "?raw", import: "default", eager: true }) as Record<string, string>;

afterEach(cleanup);

describe("the connect screen and the sign-in page are made of the same pieces", () => {
  const connect = components["../components/DesktopConnect.tsx"];
  const signIn = pages["../web/AuthCanvas.tsx"];

  it("both use the one frame and write none of its classes themselves", () => {
    for (const source of [connect, signIn]) {
      expect(source).toContain("<AuthFrame");
      for (const klass of ["shared-auth-canvas", "shared-auth-intro", "shared-auth-form-region", "shared-auth-form-shell", "shared-brand"]) expect(source, klass).not.toContain(klass);
    }
    expect(components["../components/AuthFrame.tsx"]).toContain("shared-auth-canvas");
  });

  it("both build their form from the web client's field, input, button and toast", () => {
    for (const source of [connect, signIn]) {
      expect(source).toMatch(/from "(\.\.\/components|\.)\/ui\/field"/);
      expect(source).toMatch(/from "(\.\.\/components|\.)\/ui\/input"/);
      expect(source).toMatch(/from "(\.\.\/components|\.)\/ui\/button"/);
      expect(source).toMatch(/from "(\.\.\/components|\.)\/ui\/toast"/);
      expect(source).toContain('className="shared-auth-form"');
      expect(source).toContain('className="shared-auth-heading"');
    }
  });

});

describe("the frame", () => {
  it("puts the brand and the introduction beside the form, with the trust list", () => {
    const { container } = render(
      <AuthFrame copy="Copy." eyebrow="Eyebrow" formLabel="The form" introLabel="The intro" title="Title." trust={[{ icon: <i />, title: "One", text: "First." }, { icon: <i />, title: "Two", text: "Second." }]}>
        <form aria-label="inside" />
      </AuthFrame>,
    );
    const canvas = container.querySelector("main.shared-auth-canvas");
    expect(canvas).not.toBeNull();
    expect(canvas?.querySelector('section.shared-auth-intro[aria-label="The intro"] h1')?.textContent).toBe("Title.");
    expect(canvas?.querySelectorAll(".shared-trust-list dt")).toHaveLength(2);
    expect(canvas?.querySelector('section.shared-auth-form-region[aria-label="The form"] .shared-auth-form-shell form')).not.toBeNull();
  });

  it("is what the connect screen shows", () => {
    const { container } = render(<DesktopConnect onConnected={() => undefined} />);
    expect(container.querySelector("main.shared-auth-canvas .shared-auth-intro h1")?.textContent).toBe("Your team's evidence, on your desktop.");
    expect(screen.getByLabelText("Server address")).toBeTruthy();
    expect(container.querySelector(".shared-auth-form-shell form.shared-auth-form")).not.toBeNull();
    expect(screen.getByRole("button", { name: "Connect" })).toBeTruthy();
  });
});
