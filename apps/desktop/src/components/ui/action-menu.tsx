import * as MenuPrimitive from "@radix-ui/react-dropdown-menu";
import type { ReactNode } from "react";
import { IconDots } from "@tabler/icons-react";
import { ICON_SIZE } from "./icon";
import { IconButton } from "./icon-button";

export interface ActionMenuItem {
  label: string;
  icon?: ReactNode;
  onSelect: () => void;
  destructive?: boolean;
}

/** A small "more actions" menu used on cards and rows. */
export function ActionMenu({ items, label, className = "" }: { items: ActionMenuItem[]; label: string; className?: string }) {
  return (
    <MenuPrimitive.Root>
      <MenuPrimitive.Trigger asChild>
        <IconButton appearance="plain" className={className} label={label} size="sm"><IconDots size={ICON_SIZE.md} /></IconButton>
      </MenuPrimitive.Trigger>
      <MenuPrimitive.Portal>
        <MenuPrimitive.Content align="start" className="rm-action-menu" sideOffset={4}>
          {items.map((item) => (
            <MenuPrimitive.Item className={item.destructive ? "rm-action-item destructive" : "rm-action-item"} key={item.label} onSelect={item.onSelect}>
              {item.icon}<span>{item.label}</span>
            </MenuPrimitive.Item>
          ))}
        </MenuPrimitive.Content>
      </MenuPrimitive.Portal>
    </MenuPrimitive.Root>
  );
}
