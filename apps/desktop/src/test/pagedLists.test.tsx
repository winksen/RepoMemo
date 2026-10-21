import { act, cleanup, fireEvent, render, renderHook, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { Pagination } from "../components/ui/pagination";
import { usePagedList } from "../lib/usePagedList";

afterEach(() => { cleanup(); });

const rows = Array.from({ length: 60 }, (_, index) => index + 1);

describe("usePagedList", () => {
  it("shows 24 rows a page", () => {
    const { result } = renderHook(() => usePagedList(rows));
    expect(result.current.rows).toHaveLength(24);
    expect(result.current.rows[0]).toBe(1);
    act(() => result.current.props.onPageChange(2));
    expect(result.current.rows).toEqual(rows.slice(48));
  });

  it("starts again at the first page when the filter changes", () => {
    const { result, rerender } = renderHook(({ key }) => usePagedList(rows, key), { initialProps: { key: "a" } });
    act(() => result.current.props.onPageChange(1));
    expect(result.current.props.page).toBe(1);
    rerender({ key: "b" });
    expect(result.current.props.page).toBe(0);
  });

  it("is hidden while everything fits on one page", () => {
    render(<Pagination onPageChange={() => undefined} page={0} total={24} />);
    expect(screen.queryByRole("navigation", { name: "Pagination" })).toBeNull();
  });

  it("moves between pages with its buttons", () => {
    let current = 0;
    const { rerender } = render(<Pagination onPageChange={(page) => { current = page; }} page={0} total={60} />);
    expect(screen.getByText("1–24 of 60")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Next page" }));
    expect(current).toBe(1);
    rerender(<Pagination onPageChange={(page) => { current = page; }} page={2} total={60} />);
    expect(screen.getByText("49–60 of 60")).toBeTruthy();
    expect((screen.getByRole("button", { name: "Next page" }) as HTMLButtonElement).disabled).toBe(true);
  });
});
