import { listCairnTools } from "../../lib/sharedApi";
import { cairnAdminKeys } from "../../lib/queryClient";
import type { CairnTool } from "../../types";
import { Checkbox } from "../../components/ui/checkbox";
import { PanelHeading } from "../../components/ui/panel-heading";
import { Wrench } from "@phosphor-icons/react";
import { useSystemData } from "../system/shared";

/**
 * What CAIRN may do (V0.9.91): its jobs and tools, each with a switch. A job or tool that is turned off is not offered in the + menu,
 * is not given to the model and is refused by name, with who turned it off. A workspace can turn off more than its organization, and
 * cannot turn on what its organization turned off (`locked`).
 */
export function ToolSwitches({ accessToken, disabled, locked = [], off, onChange }: { accessToken: string; disabled?: boolean; locked?: string[]; off: string[]; onChange: (off: string[]) => void }) {
  const [tools] = useSystemData<CairnTool[]>(cairnAdminKeys.tools(), () => listCairnTools(accessToken), { failure: "The list of what CAIRN can do could not be loaded." });
  if (!tools) return null;
  const group = (kind: "job" | "tool", title: string) => <div>
    <p className="shared-eyebrow">{title}</p>
    {tools.filter((tool) => tool.kind === kind).map((tool) => {
      const isLocked = locked.includes(tool.name);
      return <Checkbox checked={!(off.includes(tool.name) || isLocked)} description={isLocked ? "Turned off by your organization" : tool.writes ? "Changes the workspace once confirmed" : undefined} disabled={disabled || isLocked} key={tool.name} label={tool.label} onCheckedChange={(on) => onChange(on ? off.filter((name) => name !== tool.name) : [...off, tool.name])} />;
    })}
  </div>;
  return <section className="shared-settings-group">
    <PanelHeading description="Untick what CAIRN may not do" icon={Wrench} title="What CAIRN may do" />
    {group("job", "Jobs")}
    {group("tool", "Tools in Agent mode")}
  </section>;
}
