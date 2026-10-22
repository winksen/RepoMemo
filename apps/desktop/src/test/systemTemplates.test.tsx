import { cleanup, render, screen, within } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { TemplatesPanel } from "../web/system/Overview";
import type { SystemTemplateFiles } from "../types";

afterEach(cleanup);

const templates: SystemTemplateFiles["templates"] = [
  { id: "general", name: "General", version: 1, source: "code", file: null },
  { id: "operations", name: "Operations and incidents", version: 3, source: "built-in", file: null },
  { id: "support", name: "Customer support", version: 1, source: "custom", file: "support.json" },
];

describe("System › Overview › Templates", () => {
  it("lists every template with its version and where it came from, and tags the custom one", () => {
    render(<TemplatesPanel files={{ folder: "/srv/repomemo/templates", templates, loaded: [{ id: "support", name: "Customer support", file: "support.json" }], problems: [] }} />);
    const rows = screen.getAllByRole("row").slice(1);
    expect(rows).toHaveLength(3);
    expect(within(rows[0]).getByText("Built into the server (code)")).toBeTruthy();
    expect(within(rows[1]).getByText("Built into the server")).toBeTruthy();
    expect(within(rows[1]).getByText("3")).toBeTruthy();
    expect(within(rows[2]).getByText("Custom · support.json")).toBeTruthy();
    expect(screen.getAllByText("Custom")).toHaveLength(1);
    expect(screen.getByText("3 available · 1 custom")).toBeTruthy();
    expect(screen.getByText("/srv/repomemo/templates")).toBeTruthy();
    expect(screen.getByText("Every file in the folder was loaded. A change needs a restart.")).toBeTruthy();
  });

  it("names each file that was left out with the place and the reason, and says it never stops the server", () => {
    render(<TemplatesPanel files={{
      folder: "/srv/repomemo/templates", templates: templates.slice(0, 2), loaded: [],
      problems: [
        { file: "typo.json", issues: [{ path: "sections.order[0]", message: "`evidnce` is not a workspace section", line: null, column: null }] },
        { file: "broken.json", issues: [{ path: "", message: "expected `,` or `}`", line: 4, column: 12 }] },
        { file: "operations.json", issues: [{ path: "meta.id", message: "operations is a template built into this server; a file cannot replace it", line: null, column: null }] },
      ],
    }} />);
    expect(screen.getByText("2 available · 0 custom · 3 files left out")).toBeTruthy();
    expect(screen.getByText("typo.json")).toBeTruthy();
    expect(screen.getByText("sections.order[0]")).toBeTruthy();
    expect(screen.getByText("`evidnce` is not a workspace section")).toBeTruthy();
    expect(screen.getByText("line 4, column 12")).toBeTruthy();
    expect(screen.getByText(/cannot replace it/)).toBeTruthy();
    expect(screen.getByText(/never stops the server/)).toBeTruthy();
    expect(screen.queryByText("Every file in the folder was loaded. A change needs a restart.")).toBeNull();
  });

  it("says where the custom templates would go when no folder is set, and shows only the built-in ones", () => {
    render(<TemplatesPanel files={{ folder: null, templates: templates.slice(0, 2), loaded: [], problems: [] }} />);
    expect(screen.getByText("not set (REPOMEMO_TEMPLATES_DIR)")).toBeTruthy();
    expect(screen.queryByText("Custom")).toBeNull();
    expect(screen.queryByText(/Every file in the folder/)).toBeNull();
  });
});
