import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, expect, it } from "vitest";
import { ServerUnreachable } from "../components/ServerUnreachable";

afterEach(cleanup);

it("says the server cannot be reached and offers to try again, with no sign-in form", () => {
  render(<ServerUnreachable />);
  expect(screen.getByRole("heading", { name: "We can't connect to the server right now" })).toBeTruthy();
  expect(screen.getByRole("button", { name: "Try again" })).toBeTruthy();
  expect(screen.queryByLabelText(/password/i)).toBeNull();
});
