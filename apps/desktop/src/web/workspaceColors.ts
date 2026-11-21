import type { Workspace } from "../types";

/** The colours of one theme: the accent, its hover, the soft background, the text that reads on the page, and the text on the accent. */
export interface ColorTones { accent: string; hover: string; soft: string; text: string; on: string }

/**
 * The colours a workspace can take as its main colour, which shows on its tabs and highlights. Each has two variants,
 * so that it reads well on both themes: a darker one for the light theme (white text on it, 5:1 or better) and a
 * lighter one for the dark theme (dark text on it, and 5:1 or better against the dark surface). The default (no
 * colour) is the app's own indigo.
 */
export const WORKSPACE_COLORS: { key: string; label: string; light: ColorTones; dark: ColorTones }[] = [
  { key: "blue", label: "Blue", light: { accent: "#0e6cd8", hover: "#0b59b1", soft: "#f0f7ff", text: "#0b59b1", on: "#ffffff" }, dark: { accent: "#2085f8", hover: "#3993f9", soft: "#14273d", text: "#72acee", on: "#0f0f0f" } },
  { key: "teal", label: "Teal", light: { accent: "#0e7c71", hover: "#0a574f", soft: "#f0fffd", text: "#0a574f", on: "#ffffff" }, dark: { accent: "#0ebeac", hover: "#10d5c2", soft: "#143d39", text: "#34dfce", on: "#0f0f0f" } },
  { key: "green", label: "Green", light: { accent: "#167e41", hover: "#105b2f", soft: "#f0fff6", text: "#105b2f", on: "#ffffff" }, dark: { accent: "#18b459", hover: "#1cca64", soft: "#143d25", text: "#3fd57d", on: "#0f0f0f" } },
  { key: "orange", label: "Orange", light: { accent: "#b84f0a", hover: "#913f08", soft: "#fff6f0", text: "#913f08", on: "#ffffff" }, dark: { accent: "#e15d05", hover: "#fa6705", soft: "#3d2514", text: "#eb8642", on: "#0f0f0f" } },
  { key: "red", label: "Red", light: { accent: "#da1b2b", hover: "#b61624", soft: "#fff0f1", text: "#b61624", on: "#ffffff" }, dark: { accent: "#ef4856", hover: "#f15f6c", soft: "#3d1418", text: "#ed929a", on: "#0f0f0f" } },
  { key: "pink", label: "Pink", light: { accent: "#ce1c75", hover: "#ab1761", soft: "#fff0f7", text: "#ab1761", on: "#ffffff" }, dark: { accent: "#ec4196", hover: "#ee58a3", soft: "#3d1429", text: "#ea8bba", on: "#0f0f0f" } },
  { key: "purple", label: "Purple", light: { accent: "#852fda", hover: "#7022bf", soft: "#f7f0ff", text: "#7022bf", on: "#ffffff" }, dark: { accent: "#a866ea", hover: "#b57ded", soft: "#29143d", text: "#ccabed", on: "#0f0f0f" } },
  { key: "slate", label: "Slate", light: { accent: "#637083", hover: "#515c6c", soft: "#f6f7f9", text: "#515c6c", on: "#ffffff" }, dark: { accent: "#7588a3", hover: "#8596ad", soft: "#24282e", text: "#a3aebd", on: "#0f0f0f" } },
];

/** The key of the colour a workspace uses, or `null` for the app's default when none was chosen or the key is unknown. */
export function workspaceColorKey(workspace: Pick<Workspace, "settings">): string | null {
  const chosen = workspace.settings?.color;
  return typeof chosen === "string" && WORKSPACE_COLORS.some((entry) => entry.key === chosen) ? chosen : null;
}

const TOKENS: (keyof ColorTones)[] = ["accent", "hover", "soft", "text", "on"];

/**
 * Gives `element` (and what is inside it) the colour: both variants are set as custom properties and the stylesheet
 * picks the one of the current theme (`[data-workspace-color]` in `01-tokens-and-shared-web.css`), so changing the theme needs no work here.
 * `null` takes the colour away. Returns nothing; call it again, or with `null`, to change it.
 */
export function applyWorkspaceColor(element: HTMLElement | null, key: string | null) {
  if (!element) return;
  const color = key ? WORKSPACE_COLORS.find((entry) => entry.key === key) : undefined;
  if (!color) {
    element.removeAttribute("data-workspace-color");
    for (const token of TOKENS) { element.style.removeProperty(`--ws-light-${token}`); element.style.removeProperty(`--ws-dark-${token}`); }
    return;
  }
  element.setAttribute("data-workspace-color", color.key);
  for (const token of TOKENS) {
    element.style.setProperty(`--ws-light-${token}`, color.light[token]);
    element.style.setProperty(`--ws-dark-${token}`, color.dark[token]);
  }
}
