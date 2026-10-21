import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { Field } from "../components/ui/field";
import { Input } from "../components/ui/input";

afterEach(() => { cleanup(); });

function Form({ onSubmit }: { onSubmit: () => void }) {
  return <form onSubmit={(event) => { event.preventDefault(); onSubmit(); }}>
    <Field label="Title" required><Input /></Field>
    <Field label="Email" required><Input type="email" /></Field>
    <button type="submit">Save</button>
  </form>;
}

describe("a field the browser refuses", () => {
  it("shows its own message, marks the control and does not submit", () => {
    const onSubmit = vi.fn();
    render(<Form onSubmit={onSubmit} />);
    fireEvent.click(screen.getByRole("button", { name: "Save" }));
    expect(onSubmit).not.toHaveBeenCalled();
    const title = screen.getByLabelText("Title");
    expect(title.getAttribute("aria-invalid")).toBe("true");
    expect(screen.getByText("Title is required.")).toBeTruthy();
    expect(title.getAttribute("aria-describedby")).toContain("message");
    expect(screen.getByText("Email is required.")).toBeTruthy();
  });

  it("asks for a well-formed email address and clears once the value is valid", () => {
    render(<Form onSubmit={vi.fn()} />);
    const email = screen.getByLabelText("Email");
    fireEvent.change(screen.getByLabelText("Title"), { target: { value: "A title" } });
    fireEvent.change(email, { target: { value: "not-an-address" } });
    fireEvent.click(screen.getByRole("button", { name: "Save" }));
    expect(screen.getByText("Enter an email address such as name@example.com.")).toBeTruthy();
    fireEvent.change(email, { target: { value: "me@example.com" } });
    expect(screen.queryByText("Enter an email address such as name@example.com.")).toBeNull();
    expect(email.getAttribute("aria-invalid")).toBeNull();
  });

  it("submits once every field is valid", () => {
    const onSubmit = vi.fn();
    render(<Form onSubmit={onSubmit} />);
    fireEvent.change(screen.getByLabelText("Title"), { target: { value: "A title" } });
    fireEvent.change(screen.getByLabelText("Email"), { target: { value: "me@example.com" } });
    fireEvent.click(screen.getByRole("button", { name: "Save" }));
    expect(onSubmit).toHaveBeenCalledTimes(1);
  });

  it("keeps a message the page sets itself", () => {
    render(<Field invalid label="Port" message="Enter a value from 1 to 65535."><Input /></Field>);
    expect(screen.getByText("Enter a value from 1 to 65535.")).toBeTruthy();
  });
});
