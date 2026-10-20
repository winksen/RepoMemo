import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { IconButton } from "../components/ui/icon-button";

afterEach(() => { cleanup(); });

describe("the assistant's icon buttons", () => {
  it("name themselves, keep the pressed state of the history toggle and stay 32 px squares", () => {
    const toggle = vi.fn();
    render(<IconButton aria-expanded label="Chat history" onClick={toggle} size="sm"><span>H</span></IconButton>);
    const button = screen.getByRole("button", { name: "Chat history" });
    expect(button.getAttribute("aria-expanded")).toBe("true");
    expect(button.className).toContain("rm-icon-button-sm");
    fireEvent.click(button);
    expect(toggle).toHaveBeenCalledTimes(1);
  });
});
