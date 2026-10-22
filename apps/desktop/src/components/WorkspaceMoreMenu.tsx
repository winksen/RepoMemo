import * as MenuPrimitive from "@radix-ui/react-dropdown-menu";
import { IconChevronDown as ChevronDown } from "@tabler/icons-react";
import type { ReactNode } from "react";
import { Button } from "./ui/button";
import { ICON_SIZE } from "./ui/icon";

export interface MoreMenuItem {
  icon?: ReactNode;
  label: string;
  onSelect: () => void;
  section: string;
}

/** The *More* tab of a workspace: the sections its template keeps out of the row, each a menu item. */
export function WorkspaceMoreMenu({ items }: { items: MoreMenuItem[] }) {
  if (!items.length) return null;
  return (
    <MenuPrimitive.Root>
      <MenuPrimitive.Trigger asChild>
        <Button type="button" variant="secondary"><span className="shared-workspace-tab-label">More<ChevronDown size={ICON_SIZE.sm} /></span></Button>
      </MenuPrimitive.Trigger>
      <MenuPrimitive.Portal>
        <MenuPrimitive.Content align="start" aria-label="More sections" className="rm-action-menu" sideOffset={4}>
          {items.map((item) => (
            <MenuPrimitive.Item className="rm-action-item" key={item.section} onSelect={item.onSelect}>
              {item.icon}<span>{item.label}</span>
            </MenuPrimitive.Item>
          ))}
        </MenuPrimitive.Content>
      </MenuPrimitive.Portal>
    </MenuPrimitive.Root>
  );
}
