import { WORKSPACE_ICONS } from "../web/workspaceIcons";
import { ICON_SIZE } from "./ui/icon";
import type { TemplateSummary } from "../types";
import { Badge } from "./ui/badge";

/**
 * Which template a new workspace starts with: one card for each, with its icon, name and what it is for.
 * It shows nothing while the server offers only `general`, so a server with no other template looks as it did.
 */
export function TemplatePicker({ disabled = false, name, onChange, templates, value }: {
  disabled?: boolean;
  /** Distinguishes the radio group when two pickers are on a page. */
  name: string;
  onChange: (templateId: string) => void;
  templates: TemplateSummary[];
  value: string;
}) {
  if (templates.length < 2) return null;
  return (
    <fieldset className="rm-template-picker" disabled={disabled}>
      <legend>Template</legend>
      <div className="rm-template-options">
        {templates.map((template) => {
          const Icon = (WORKSPACE_ICONS.find((entry) => entry.key === template.icon) ?? WORKSPACE_ICONS[0]).icon;
          return (
            <label className="rm-template-option" key={template.id}>
              <input checked={value === template.id} className="rm-template-radio" name={name} onChange={() => onChange(template.id)} type="radio" value={template.id} />
              <Icon size={ICON_SIZE.md} />
              <span className="rm-template-text"><strong>{template.name}{template.custom ? <> <Badge>Custom</Badge></> : null}</strong><span>{template.summary}</span></span>
            </label>
          );
        })}
      </div>
    </fieldset>
  );
}
