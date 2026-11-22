import { CheckCircle } from "@phosphor-icons/react";
import type { TemplateSummary } from "../types";
import { Badge } from "./ui/badge";
import { ICON_SIZE } from "./ui/icon";
import { TemplateArt } from "./TemplateArt";

/**
 * Which template a new workspace starts with: a card for each, with a drawing of what it is for, its name and a line about it.
 * It shows nothing while the server offers only `general`, so a server with no other template looks as it did.
 */
export function TemplatePicker({ activeId, disabled = false, name, onChange, templates, value }: {
  /** The template the workspace uses now; its card is tagged *Active now*. */
  activeId?: string;
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
        {templates.map((template) => (
          <label className="rm-template-option" key={template.id}>
            <input checked={value === template.id} className="rm-template-radio" name={name} onChange={() => onChange(template.id)} type="radio" value={template.id} />
            <span className="rm-template-art-box"><TemplateArt id={template.id} /></span>
            <CheckCircle aria-hidden="true" className="rm-template-check" size={ICON_SIZE.lg} weight="fill" />
            <span className="rm-template-text"><strong>{template.name}{activeId === template.id ? <> <Badge className="rm-template-active">Active now</Badge></> : null}{template.custom ? <> <Badge>Custom</Badge></> : null}</strong><span>{template.summary}</span></span>
          </label>
        ))}
      </div>
    </fieldset>
  );
}
