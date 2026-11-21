import { cleanup, render, waitFor } from "@testing-library/react";
import { afterEach, expect, it } from "vitest";
import { useTabIndicator } from "../web/tabIndicator";

afterEach(cleanup);

function Tabs({ active }: { active: string | null }) {
  const { ref, indicator } = useTabIndicator();
  return <div className="shared-workspace-tabs" ref={ref}>{["a", "b"].map((key) => <button aria-current={key === active ? "page" : undefined} key={key} type="button">{key}</button>)}{indicator}</div>;
}

it("draws one indicator under the active tab, and none without one", async () => {
  const { container, rerender } = render(<Tabs active="a" />);
  expect(container.querySelectorAll(".shared-tab-indicator")).toHaveLength(1);
  expect(container.querySelector(".shared-tab-indicator")?.getAttribute("aria-hidden")).toBe("true");
  rerender(<Tabs active="b" />);
  await waitFor(() => expect(container.querySelectorAll(".shared-tab-indicator")).toHaveLength(1));
  rerender(<Tabs active={null} />);
  await waitFor(() => expect(container.querySelector(".shared-tab-indicator")).toBeNull());
});

it("does not animate its first appearance, and animates the moves after it", async () => {
  const { container } = render(<Tabs active="a" />);
  const indicator = () => container.querySelector(".shared-tab-indicator");
  expect(indicator()?.getAttribute("data-animated")).toBe("false");
  await waitFor(() => expect(indicator()?.getAttribute("data-animated")).toBe("true"));
});
