import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { Dialog, DialogCancel } from "../components/ui/dialog";
import { ORGANIZATION_ICONS, OrganizationIcon, organizationIconKey } from "../web/organizationIcons";

afterEach(cleanup);

it("has a close button in every dialog that closes it", () => {
  const onClose = vi.fn();
  render(<Dialog footer={<DialogCancel onClick={onClose} />} onClose={onClose} open title="Anything"><p>Body</p></Dialog>);
  fireEvent.click(screen.getByRole("button", { name: "Close dialog" }));
  expect(onClose).toHaveBeenCalled();
});

it("has the close button in a dialog that only asks a question, too", () => {
  render(<Dialog footer={<DialogCancel onClick={vi.fn()} />} onClose={vi.fn()} open title="Sure?" />);
  expect(screen.getByRole("button", { name: "Close dialog" })).toBeTruthy();
});

it("shows the organization's icon, the company by default and for a key it does not know", () => {
  expect(ORGANIZATION_ICONS.length).toBeGreaterThanOrEqual(10);
  expect(organizationIconKey({ icon: "house" })).toBe("house");
  expect(organizationIconKey({ icon: "nonsense" })).toBe("building");
  expect(organizationIconKey({ icon: null })).toBe("building");
  const { container } = render(<OrganizationIcon organization={{ icon: "factory" }} size={20} weight="fill" />);
  expect(container.querySelector("svg")).toBeTruthy();
});
