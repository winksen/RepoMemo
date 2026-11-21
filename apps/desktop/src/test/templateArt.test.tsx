import { cleanup, render } from "@testing-library/react";
import { afterEach, expect, it } from "vitest";
import { TemplateArt } from "../components/TemplateArt";

afterEach(cleanup);

it("draws something different for each of the server's templates, and a neutral grid for any other", () => {
  const drawing = (id: string) => render(<TemplateArt id={id} />).container.querySelector("svg")!.innerHTML;
  const ids = ["general", "software-engineering", "knowledge-base", "operations", "product-decisions"];
  const drawings = ids.map(drawing);
  expect(new Set(drawings).size).toBe(ids.length);
  expect(drawing("a-custom-template")).not.toBe("");
  expect(ids.map((id) => drawing(id) === drawing("a-custom-template"))).not.toContain(true);
});

it("is decoration: hidden from assistive technology, since the card has the name", () => {
  const svg = render(<TemplateArt id="operations" />).container.querySelector("svg")!;
  expect(svg.getAttribute("aria-hidden")).toBe("true");
});
