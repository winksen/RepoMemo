import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../lib/sharedApi", () => ({
  checkSharedRepository: vi.fn(),
  clearSharedRepositoryToken: vi.fn(),
  connectSharedRepository: vi.fn(),
  createSharedRepositoryWebhook: vi.fn(),
  deleteSharedRepository: vi.fn(),
  deleteSharedRepositoryWebhook: vi.fn(),
  setSharedRepositoryDeployKey: vi.fn(),
  setSharedRepositoryToken: vi.fn(),
  updateSharedRepository: vi.fn(),
}));
vi.mock("../web/repositories/useRepositories", () => ({ useRepositories: () => ({ repositories: [], load: vi.fn(), track: vi.fn() }) }));
vi.mock("../web/workspace/data", () => ({ useTemplate: vi.fn() }));
vi.mock("../components/ui/toast", () => ({ showToast: vi.fn() }));

import { checkSharedRepository, connectSharedRepository } from "../lib/sharedApi";
import { useTemplate } from "../web/workspace/data";
import { RepositoryFields } from "../web/repositories/LinkForm";
import { RepositorySettings } from "../web/repositories/RepositorySettings";
import { EMPTY_FORM, NO_OFFER, formWithOffer, repoOffer } from "../web/repositories/shared";
import type { WorkspaceTemplateView } from "../types";

const template = (name: string, include: string[], exclude: string[]) => ({ template: { name }, repositories: { include, exclude } }) as unknown as WorkspaceTemplateView;
const software = template("Software engineering", [], ["*.snap", "**/testdata/**"]);
const general = template("General", [], []);

const NOTE = "Filled in from the Software engineering template; change or clear them as you like.";
const exclude = () => screen.getByLabelText("Exclude") as HTMLTextAreaElement;
const include = () => screen.getByLabelText("Include only") as HTMLTextAreaElement;

function show(view: WorkspaceTemplateView | null) {
  vi.mocked(useTemplate).mockReturnValue([view, vi.fn(), {} as never]);
}
const settings = () => <RepositorySettings accessToken="t" onChanged={vi.fn()} workspaceId="w1" />;

beforeEach(() => { vi.clearAllMocks(); });
afterEach(cleanup);

describe("the rules a template offers the link form", () => {
  it("are the template's patterns as the form's own lines, and nothing when it offers none", () => {
    expect(repoOffer(null)).toBe(NO_OFFER);
    expect(repoOffer(general)).toBe(NO_OFFER);
    expect(repoOffer(software)).toEqual({ include: "", exclude: "*.snap\n**/testdata/**", templateName: "Software engineering" });
    expect(repoOffer(template("Knowledge base", ["**/*.pdf", "**/*.docx"], []))).toEqual({ include: "**/*.pdf\n**/*.docx", exclude: "", templateName: "Knowledge base" });
    expect(formWithOffer(repoOffer(software))).toEqual({ ...EMPTY_FORM, exclude: "*.snap\n**/testdata/**" });
    expect(formWithOffer(NO_OFFER)).toEqual(EMPTY_FORM);
  });

  it("are said to come from the template under both fields, and only while the fields hold them", () => {
    const { rerender } = render(<RepositoryFields form={EMPTY_FORM} offeredBy="Software engineering" onChange={vi.fn()} showLink />);
    expect(screen.getAllByText(NOTE, { exact: false })).toHaveLength(2);
    rerender(<RepositoryFields form={EMPTY_FORM} onChange={vi.fn()} showLink />);
    expect(screen.queryByText(NOTE, { exact: false })).toBeNull();
  });
});

describe("the link form of a workspace", () => {
  it("is filled in with the template's rules, which stay editable", () => {
    show(software);
    render(settings());
    expect(exclude().value).toBe("*.snap\n**/testdata/**");
    expect(include().value).toBe("");
    expect(screen.getAllByText(NOTE, { exact: false })).toHaveLength(2);
    fireEvent.change(exclude(), { target: { value: "*.snap" } });
    expect(exclude().value).toBe("*.snap");
    expect(screen.queryByText(NOTE, { exact: false })).toBeNull();
  });

  it("is empty as ever for General, or while the template has not arrived", () => {
    show(general);
    render(settings());
    expect(exclude().value).toBe("");
    expect(screen.queryByText(NOTE, { exact: false })).toBeNull();
    cleanup();
    show(null);
    render(settings());
    expect(exclude().value).toBe("");
  });

  it("never overwrites what someone typed before the template arrived", () => {
    show(null);
    const { rerender } = render(settings());
    fireEvent.change(exclude(), { target: { value: "mine/**" } });
    show(software);
    rerender(settings());
    expect(exclude().value).toBe("mine/**");
  });

  it("links with what the person kept, then starts the next link from the template's rules again", async () => {
    show(software);
    vi.mocked(checkSharedRepository).mockResolvedValue({
      name: "app", root_path: "/srv/code/app", remote: false, already_connected: false, indexable_files: 3, tracked_files: 4,
      commit: { branch: "main", sha: "abcdef1234", summary: "start", author_name: "Ada", committed_at: "2026-01-01T00:00:00Z" },
    } as never);
    vi.mocked(connectSharedRepository).mockResolvedValue({ repository: { id: "r1", name: "app" }, job: null } as never);
    render(settings());
    fireEvent.change(screen.getByLabelText("Repository link"), { target: { value: "/srv/code/app" } });
    fireEvent.change(exclude(), { target: { value: "*.snap\nvendor-extra/**" } });
    fireEvent.click(screen.getByRole("button", { name: /Check access/ }));
    await screen.findByText(/The server can read this repository/);
    fireEvent.click(screen.getByRole("button", { name: /Link and sync/ }));
    await waitFor(() => expect(connectSharedRepository).toHaveBeenCalledTimes(1));
    expect(vi.mocked(connectSharedRepository).mock.calls[0][2]).toMatchObject({ link: "/srv/code/app", include: [], exclude: ["*.snap", "vendor-extra/**"] });
    await waitFor(() => expect(exclude().value).toBe("*.snap\n**/testdata/**"));
  });
});
